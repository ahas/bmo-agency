//! A job's native `Agent` calls held to the common co-worker pool (docs/coworker-pool.md, 「Native co-worker
//! places」): the hook's input and answers, the admission and the place given back over a scratch store, the
//! reservation verb, and the hook binary run as the worker's settings run it.

use crate::binary;
use crate::common;

use std::io::Write;
use std::time::Duration;

use agency_server::order_run::dispatch::reclaim_native;
use agency_server::verbs::coworker_pool::{
  self, Admission, HookInput, NESTED_REFUSAL, ProcessMark, admit, allowance, give_back, hook_input,
  process_mark,
};
use common::Scratch;
use serde_json::{Value, json};

fn own() -> ProcessMark {
  process_mark(i64::from(std::process::id())).unwrap()
}

// A process that has ended, marked as it stood: its pid no longer names it.
fn gone() -> ProcessMark {
  let child = std::process::Command::new("true").spawn().unwrap();
  let pid = i64::from(child.id());
  let mark = process_mark(pid).unwrap_or(ProcessMark {
    pid,
    start: "1".into(),
  });

  child.wait_with_output().unwrap();

  mark
}

fn call(tool_use_id: &str, role: Option<&str>, caller: Option<&str>) -> HookInput {
  HookInput::Pre {
    session: "session-1".into(),
    tool_use_id: tool_use_id.into(),
    coworker_type: role.map(str::to_owned),
    caller_agent_id: caller.map(str::to_owned),
    tool_input: json!({ "subagent_type": role, "prompt": "look" }),
  }
}

fn returned(tool_use_id: &str, status: &str, agent: Option<&str>) -> HookInput {
  HookInput::Post {
    session: "session-1".into(),
    tool_use_id: tool_use_id.into(),
    status: Some(status.into()),
    agent_id: agent.map(str::to_owned),
  }
}

/// The provider, tier and model the scout role runs on.
struct Mapping {
  provider: String,
  tier: String,
  model: String,
}

// The scout role pooled, assigned the Light tier, Light mapped to a model on Claude Code and its bucket
// held to a capacity of the test's own; the mapping the role resolves to.
async fn fixture(scratch: &Scratch, capacity: i64) -> Mapping {
  for sql in [
    r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
       VALUES ('scout', 'locates', 'locate', '', '', 1, 1, 1)
       ON CONFLICT ("name") DO UPDATE SET "enabled" = 1, "pooled" = 1"#,
    r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:scout', 'Light')
       ON CONFLICT ("key") DO UPDATE SET "tier" = 'Light'"#,
    r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'Light', 'claude-sonnet-5', 'low')
       ON CONFLICT ("provider", "tier") DO NOTHING"#,
  ] {
    scratch.run(sql, vec![]).await;
  }

  let (provider, tier, model) = coworker_pool::role_mapping(scratch.db(), "scout").await.unwrap();

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at") VALUES ($2, $3, $1, 1)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "capacity" = $1"#,
      vec![capacity.into(), provider.clone().into(), tier.clone().into()],
    )
    .await;

  Mapping {
    provider,
    tier,
    model,
  }
}

// The pool emptied between the steps of one store: no request, reservation, native row or stop mark
// stands. A migration of the whole store takes seconds, so the steps share two stores rather than
// paying one each.
async fn reset(scratch: &Scratch) {
  for table in [
    "coworker_pool_requests",
    "coworker_pool_reservations",
    "coworker_pool_native",
    "coworker_pool_native_stops",
    "coworker_pool_native_reservations",
  ] {
    scratch.run(&format!(r#"DELETE FROM "{table}""#), vec![]).await;
  }
}

// What the pool does with native calls, one step after another on one store.
#[tokio::test]
async fn the_pool_admits_waits_gives_back_and_reclaims_native_calls() {
  let archive = Scratch::new().await;
  let scratch = &archive;

  a_pooled_native_call_gets_a_place_on_the_roles_own_model_and_gives_it_back_when_it_returns(scratch).await;
  a_full_bucket_makes_the_call_wait_then_refuses_it_and_leaves_no_waiting_request_at_the_head(scratch)
    .await;
  a_waiting_call_gets_the_place_the_moment_the_holder_gives_it_back(scratch).await;
  a_tiers_new_model_and_a_roles_new_tier_still_let_a_native_call_in_counted_under_that_tier(scratch).await;
  a_background_child_holds_its_place_until_it_stops_whichever_of_launch_and_stop_is_seen_first(scratch)
    .await;
  a_type_outside_the_pool_and_a_call_from_inside_a_coworker_are_refused_without_a_request(scratch).await;
  reclaim_ends_a_place_only_on_evidence_that_its_process_is_gone_never_for_its_age(scratch).await;
  a_clis_reserved_seats_let_its_native_calls_in_without_waiting_and_go_back_when_it_releases(scratch).await;
}

// The hook and the reservation verb as the worker's settings and a job's shell run them, on one store.
#[tokio::test]
async fn the_hook_and_the_reservation_verb_run_as_the_worker_runs_them() {
  let archive = Scratch::new().await;
  let scratch = &archive;

  the_hook_lets_a_pooled_call_out_refuses_another_type_and_gives_the_place_back(scratch).await;
  reserve_holds_seats_for_the_cli_that_ran_it_and_release_gives_them_back(scratch).await;
}

async fn state(scratch: &Scratch, token: &str) -> String {
  agency_server::order_run::dispatch::request(scratch.db(), token)
    .await
    .unwrap()
    .unwrap()
    .state
}

async fn waiting_requests(scratch: &Scratch) -> i64 {
  scratch
    .count(r#"SELECT COUNT(*) FROM "coworker_pool_requests" WHERE "state" = 'waiting'"#, vec![])
    .await
}

// Until a call stands waiting in the line, looked at every few milliseconds for at most ten seconds.
async fn until_one_waits(scratch: &Scratch) {
  let deadline = std::time::Instant::now() + Duration::from_secs(10);

  while waiting_requests(scratch).await == 0 {
    assert!(std::time::Instant::now() < deadline, "no call came to wait in the line");
    tokio::time::sleep(Duration::from_millis(25)).await;
  }
}

async fn admitted(
  scratch: &Scratch,
  input: &HookInput,
  waiter: &ProcessMark,
  owner: &ProcessMark,
  wait: Duration,
) -> Admission {
  admit(scratch.db(), input, waiter, owner, wait, &mut |_| {}).await.unwrap()
}

fn token(admission: &Admission) -> String {
  match admission {
    Admission::Allowed { token } => token.clone(),
    Admission::Refused(why) => panic!("refused: {why}"),
  }
}

// "a pooled native call gets a place on the role's own model and gives it back when it returns"
async fn a_pooled_native_call_gets_a_place_on_the_roles_own_model_and_gives_it_back_when_it_returns(
  scratch: &Scratch,
) {
  reset(scratch).await;
  let mapping = fixture(scratch, 2).await;
  let held = token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );
  let request = agency_server::order_run::dispatch::request(scratch.db(), &held).await.unwrap().unwrap();

  assert_eq!(
    (
      request.state.as_str(),
      request.source.as_str(),
      request.role.as_str(),
      request.model.as_str(),
      request.child_id.as_deref()
    ),
    ("active", "native", "scout", mapping.model.as_str(), Some("toolu_a"))
  );

  give_back(scratch.db(), &returned("toolu_a", "completed", None)).await.unwrap();

  assert_eq!(state(scratch, &held).await, "released");
}

// "a full bucket makes the call wait, then refuses it and leaves no waiting request at the head"
async fn a_full_bucket_makes_the_call_wait_then_refuses_it_and_leaves_no_waiting_request_at_the_head(
  scratch: &Scratch,
) {
  reset(scratch).await;

  fixture(scratch, 1).await;
  token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );

  let refused = admitted(
    scratch,
    &call("toolu_b", Some("scout"), None),
    &own(),
    &own(),
    Duration::from_millis(100),
  )
  .await;
  let Admission::Refused(why) = refused else { panic!("let in on a full bucket") };

  assert!(why.contains("no free"));
  // The refusal names the way to hold places ahead of such calls.
  assert!(why.contains("bmo-agency coworker pool reserve --role <role> --count <n>"));
  assert_eq!(waiting_requests(scratch).await, 0);

  give_back(
    scratch.db(),
    &HookInput::Failure {
      session: "session-1".into(),
      tool_use_id: "toolu_a".into(),
    },
  )
  .await
  .unwrap();
  token(
    &admitted(scratch, &call("toolu_c", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );
}

// "a waiting call gets the place the moment the holder gives it back"
async fn a_waiting_call_gets_the_place_the_moment_the_holder_gives_it_back(scratch: &Scratch) {
  reset(scratch).await;

  fixture(scratch, 1).await;
  token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );

  let (asked, me) = (call("toolu_b", Some("scout"), None), own());
  let (second, ()) = tokio::join!(admitted(scratch, &asked, &me, &me, Duration::from_secs(20)), async {
    until_one_waits(scratch).await;
    give_back(scratch.db(), &returned("toolu_a", "completed", None)).await.unwrap();
  });

  token(&second);
}

// "a tier's new model and a role's new tier still let a native call in, counted under that tier"
async fn a_tiers_new_model_and_a_roles_new_tier_still_let_a_native_call_in_counted_under_that_tier(
  scratch: &Scratch,
) {
  reset(scratch).await;
  fixture(scratch, 4).await;
  scratch
    .run(
      r#"UPDATE "model_tier_mappings" SET "model" = 'claude-sonnet-5', "effort" = 'low'
         WHERE "provider" = 'claude-code' AND "tier" = 'Light'"#,
      vec![],
    )
    .await;

  let light = token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );
  let request = agency_server::order_run::dispatch::request(scratch.db(), &light)
    .await
    .unwrap()
    .unwrap();

  assert_eq!(
    (
      request.state.as_str(),
      request.tier.as_str(),
      request.model.as_str(),
      request.effort.as_deref(),
      request.capacity
    ),
    ("active", "Light", "claude-sonnet-5", Some("low"), 4)
  );

  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'Reliable', 'claude-opus-5', 'high')
         ON CONFLICT ("provider", "tier") DO NOTHING"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at") VALUES ('claude-code', 'Reliable', 6, 1)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "capacity" = 6"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"UPDATE "model_tier_assignments" SET "tier" = 'Reliable' WHERE "key" = 'role:scout'"#,
      vec![],
    )
    .await;

  let (_, _, reliable_model) = coworker_pool::role_mapping(scratch.db(), "scout").await.unwrap();
  let reliable = token(
    &admitted(scratch, &call("toolu_b", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );
  let request = agency_server::order_run::dispatch::request(scratch.db(), &reliable)
    .await
    .unwrap()
    .unwrap();

  assert_eq!(
    (request.state.as_str(), request.tier.as_str(), request.model.as_str(), request.capacity),
    ("active", "Reliable", reliable_model.as_str(), 6)
  );

  let mut active: Vec<(String, i64)> = agency_server::control::pool::status(scratch.db())
    .await
    .unwrap()
    .into_iter()
    .filter(|place| place.active > 0)
    .map(|place| (place.tier, place.active))
    .collect();

  active.sort();
  assert_eq!(active, [("Light".to_owned(), 1), ("Reliable".to_owned(), 1)]);

  scratch
    .run(r#"DELETE FROM "model_tier_assignments" WHERE "key" = 'role:scout'"#, vec![])
    .await;

  let Admission::Refused(why) =
    admitted(scratch, &call("toolu_c", Some("scout"), None), &own(), &own(), Duration::from_secs(1)).await
  else {
    panic!("let in with no tier assignment");
  };

  assert!(why.contains("Missing tier assignment: role:scout"), "{why}");
  assert!(!why.contains("capacity"), "{why}");
}

// "a background child holds its place until it stops, whichever of launch and stop is seen first"
async fn a_background_child_holds_its_place_until_it_stops_whichever_of_launch_and_stop_is_seen_first(
  scratch: &Scratch,
) {
  reset(scratch).await;

  fixture(scratch, 4).await;

  let launched = token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );

  give_back(scratch.db(), &returned("toolu_a", "async_launched", Some("agent-a")))
    .await
    .unwrap();
  assert_eq!(state(scratch, &launched).await, "active");
  give_back(
    scratch.db(),
    &HookInput::Stop {
      agent_id: "agent-a".into(),
    },
  )
  .await
  .unwrap();
  assert_eq!(state(scratch, &launched).await, "released");

  let quick = token(
    &admitted(scratch, &call("toolu_b", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );

  give_back(
    scratch.db(),
    &HookInput::Stop {
      agent_id: "agent-b".into(),
    },
  )
  .await
  .unwrap();
  give_back(scratch.db(), &returned("toolu_b", "async_launched", Some("agent-b")))
    .await
    .unwrap();
  assert_eq!(state(scratch, &quick).await, "released");
}

// "a type outside the pool and a call from inside a co-worker are refused without a request"
async fn a_type_outside_the_pool_and_a_call_from_inside_a_coworker_are_refused_without_a_request(
  scratch: &Scratch,
) {
  reset(scratch).await;

  fixture(scratch, 4).await;

  for role in [Some("general-purpose"), Some("builder"), None] {
    let Admission::Refused(why) =
      admitted(scratch, &call("toolu_a", role, None), &own(), &own(), Duration::from_secs(1)).await
    else {
      panic!("{role:?} let in");
    };

    assert!(why.contains("scout"), "{why}");
  }

  assert_eq!(
    admitted(
      scratch,
      &call("toolu_c", Some("scout"), Some("agent-x")),
      &own(),
      &own(),
      Duration::from_secs(1)
    )
    .await,
    Admission::Refused(NESTED_REFUSAL.into())
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "coworker_pool_requests""#, vec![]).await, 0);
}

// "reclaim ends a place only on evidence that its process is gone, never for its age"
async fn reclaim_ends_a_place_only_on_evidence_that_its_process_is_gone_never_for_its_age(
  scratch: &Scratch,
) {
  reset(scratch).await;

  fixture(scratch, 2).await;

  let alive = token(
    &admitted(scratch, &call("toolu_a", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );
  let orphaned = token(
    &admitted(scratch, &call("toolu_b", Some("scout"), None), &own(), &gone(), Duration::from_secs(5))
      .await,
  );

  assert_eq!(reclaim_native(scratch.db()).await.unwrap(), 1);
  assert_eq!(state(scratch, &alive).await, "active");
  assert_eq!(state(scratch, &orphaned).await, "released");

  // The bucket full again, a call waiting in the line whose own waiting hook is gone is ended while it
  // waits, and the place its holder keeps is left alone.
  token(
    &admitted(scratch, &call("toolu_c", Some("scout"), None), &own(), &own(), Duration::from_secs(5)).await,
  );

  let (asked, waiter, owner) = (call("toolu_d", Some("scout"), None), gone(), own());
  let (refused, reclaimed) =
    tokio::join!(admitted(scratch, &asked, &waiter, &owner, Duration::from_secs(30)), async {
      until_one_waits(scratch).await;
      reclaim_native(scratch.db()).await.unwrap()
    });

  assert_eq!(reclaimed, 1);
  assert!(matches!(refused, Admission::Refused(why) if why.contains("was ended before it got a place")));
  assert_eq!(state(scratch, &alive).await, "active");
}

// "a CLI's reserved seats let its native calls in without waiting, warm nothing, and go back when the CLI ends"
// as far as the verb holds them: the seats held, the CLI's own call let in on one while another's finds none,
// and every seat given back by `release`.
async fn a_clis_reserved_seats_let_its_native_calls_in_without_waiting_and_go_back_when_it_releases(
  scratch: &Scratch,
) {
  reset(scratch).await;
  let mapping = fixture(scratch, 1).await;
  let cli = own();
  let other = gone();
  let held = coworker_pool::reserve(scratch.db(), &cli, "scout", None, 1).await.unwrap();

  assert_eq!(
    held,
    [format!(
      "{}/{} {}: 1 of 1 seats held, 0 waiting for a place",
      mapping.provider, mapping.tier, mapping.model
    )]
  );

  // Another CLI's call finds the only place held and gets no place.
  let outsider = admitted(
    scratch,
    &call("toolu_out", Some("scout"), None),
    &own(),
    &other,
    Duration::from_millis(100),
  )
  .await;

  assert!(matches!(outsider, Admission::Refused(_)));

  let started = std::time::Instant::now();
  let mine = token(
    &admitted(scratch, &call("toolu_own", Some("scout"), None), &own(), &cli, Duration::from_secs(5)).await,
  );

  assert!(started.elapsed() < Duration::from_millis(400));
  assert_eq!(state(scratch, &mine).await, "active");

  give_back(scratch.db(), &returned("toolu_own", "completed", None)).await.unwrap();

  assert_eq!(
    coworker_pool::release(scratch.db(), &cli).await.unwrap(),
    ["1 reservation(s) given back"]
  );
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "coworker_pool_reservations" WHERE "state" = 'held'"#, vec![])
      .await,
    0
  );
}

// "hook input is read for the four events the worker settings send"
#[test]
fn hook_input_is_read_for_the_four_events_the_worker_settings_send() {
  let read = |value: Value| hook_input(&value.to_string());

  assert_eq!(
    read(
      json!({ "hook_event_name": "PreToolUse", "session_id": "s", "tool_use_id": "t", "tool_input": { "subagent_type": "scout" } })
    ),
    Ok(HookInput::Pre {
      session: "s".into(),
      tool_use_id: "t".into(),
      coworker_type: Some("scout".into()),
      caller_agent_id: None,
      tool_input: json!({ "subagent_type": "scout" }),
    })
  );
  assert_eq!(
    read(
      json!({ "hook_event_name": "PostToolUse", "session_id": "s", "tool_use_id": "t", "tool_response": { "status": "async_launched", "agentId": "a" } })
    ),
    Ok(HookInput::Post {
      session: "s".into(),
      tool_use_id: "t".into(),
      status: Some("async_launched".into()),
      agent_id: Some("a".into())
    })
  );
  assert_eq!(
    read(
      json!({ "hook_event_name": "PostToolUseFailure", "session_id": "s", "tool_use_id": "t", "error": "x" })
    ),
    Ok(HookInput::Failure {
      session: "s".into(),
      tool_use_id: "t".into()
    })
  );
  assert_eq!(
    read(json!({ "hook_event_name": "SubagentStop", "session_id": "s", "agent_id": "a" })),
    Ok(HookInput::Stop {
      agent_id: "a".into()
    })
  );
  assert!(
    read(json!({ "hook_event_name": "PreToolUse", "session_id": "s" }))
      .unwrap_err()
      .contains("tool_use_id")
  );
}

// "a call let out carries the guide after its own prompt and nothing else of it moves"
#[test]
fn a_call_let_out_carries_the_guide_after_its_own_prompt_and_nothing_else_of_it_moves() {
  let input = json!({ "subagent_type": "verifier", "description": "check", "prompt": "Is it real?", "run_in_background": true });
  let out: Value = serde_json::from_str(&allowance(&input, "GUIDE").unwrap()).unwrap();
  let mut expected = input.clone();

  expected["prompt"] = json!("Is it real?\n\nGUIDE");

  assert_eq!(out["hookSpecificOutput"]["permissionDecision"], "allow");
  assert_eq!(out["hookSpecificOutput"]["updatedInput"], expected);
  assert_eq!(allowance(&json!({ "subagent_type": "scout" }), "GUIDE"), None);
}

// "a process mark tells a live process from one whose pid was handed on"
#[test]
fn a_process_mark_tells_a_live_process_from_one_whose_pid_was_handed_on() {
  let me = own();

  assert_eq!(process_mark(me.pid), Some(me.clone()));
  assert_ne!(process_mark(me.pid).map(|mark| mark.start), Some(format!("{}0", me.start)));
}

// The hook binary as the worker's settings run it, its input on standard input.
fn hook(scratch: &Scratch, input: &Value) -> (i32, String, String) {
  let address = scratch.address();
  let data_root = tempfile::tempdir().unwrap();
  let mut child = std::process::Command::new(binary::built("agency-server", "bmo-agency"))
    .args(["coworker", "hook"])
    .env("BMO_DATA_DIR", data_root.path())
    .env("BMO_ARCHIVE_URL", address)
    .env_remove("BMO_HOME_BRIDGE")
    .env_remove("ANTHROPIC_BASE_URL")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();

  let done = child.wait_with_output().unwrap();

  (
    done.status.code().unwrap_or(-1),
    String::from_utf8_lossy(&done.stdout).into_owned(),
    String::from_utf8_lossy(&done.stderr).into_owned(),
  )
}

// "the hook script lets a pooled call out, refuses another type and gives the place back"
async fn the_hook_lets_a_pooled_call_out_refuses_another_type_and_gives_the_place_back(scratch: &Scratch) {
  reset(scratch).await;

  fixture(scratch, 2).await;

  let tool_input =
    json!({ "subagent_type": "scout", "description": "find it", "prompt": "Where is the pool hook?" });
  let (code, out, err) = hook(
    scratch,
    &json!({ "hook_event_name": "PreToolUse", "session_id": "hook-session", "tool_use_id": "toolu_1", "tool_input": tool_input }),
  );
  let answer: Value = serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("{out}{err}"));

  assert_eq!(code, 0);
  assert_eq!(answer["hookSpecificOutput"]["permissionDecision"], "allow");
  assert!(
    answer["hookSpecificOutput"]["updatedInput"]["prompt"]
      .as_str()
      .unwrap()
      .starts_with("Where is the pool hook?\n\n")
  );
  assert_eq!(answer["hookSpecificOutput"]["updatedInput"]["description"], "find it");

  let (code, out, _) = hook(
    scratch,
    &json!({ "hook_event_name": "PreToolUse", "session_id": "hook-session", "tool_use_id": "toolu_2", "tool_input": { "subagent_type": "general-purpose" } }),
  );

  assert_eq!(code, 0);
  assert_eq!(
    serde_json::from_str::<Value>(out.trim()).unwrap()["hookSpecificOutput"]["permissionDecision"],
    "deny"
  );
  assert_eq!(
    hook(scratch, &json!({ "hook_event_name": "PreToolUse", "session_id": "hook-session" })).0,
    2
  );
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "coworker_pool_requests" WHERE "state" = 'active' AND "child_id" = 'toolu_1'"#, vec![]).await,
    1
  );

  let (code, ..) = hook(
    scratch,
    &json!({ "hook_event_name": "PostToolUse", "session_id": "hook-session", "tool_use_id": "toolu_1", "tool_response": { "status": "completed" } }),
  );

  assert_eq!(code, 0);
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "coworker_pool_requests" WHERE "state" IN ('active', 'waiting')"#,
        vec![]
      )
      .await,
    0
  );
}

// The reservation verb as a job's shell runs it: the reservation belongs to the process that ran it — here,
// this test's own — so every run below is the same holder.
fn pool(scratch: &Scratch, args: &[&str]) -> (i32, String, String) {
  let address = scratch.address();
  let data_root = tempfile::tempdir().unwrap();
  let done = std::process::Command::new(binary::built("agency-server", "bmo-agency"))
    .args(["coworker", "pool"])
    .args(args)
    .env("BMO_DATA_DIR", data_root.path())
    .env("BMO_ARCHIVE_URL", address)
    .env_remove("BMO_HOME_BRIDGE")
    .env_remove("ANTHROPIC_BASE_URL")
    .stdin(std::process::Stdio::null())
    .output()
    .unwrap();

  (
    done.status.code().unwrap_or(-1),
    String::from_utf8_lossy(&done.stdout).into_owned(),
    String::from_utf8_lossy(&done.stderr).into_owned(),
  )
}

async fn held_wanted(scratch: &Scratch) -> Vec<i64> {
  let rows = scratch
    .count(
      r#"SELECT COUNT(*) FROM "coworker_pool_reservations" WHERE "state" = 'held' AND "source" = 'native'"#,
      vec![],
    )
    .await;

  if rows == 0 {
    return vec![];
  }

  vec![
    scratch
      .count(r#"SELECT "wanted" FROM "coworker_pool_reservations" WHERE "state" = 'held'"#, vec![])
      .await,
  ]
}

// "reserve holds seats for the CLI that ran it, a second reserve moves the count, release gives them back, and what it cannot hold is refused"
async fn reserve_holds_seats_for_the_cli_that_ran_it_and_release_gives_them_back(scratch: &Scratch) {
  reset(scratch).await;
  let mapping = fixture(scratch, 12).await;
  let head = format!("{}/{} {}", mapping.provider, mapping.tier, mapping.model);
  let (code, out, err) = pool(scratch, &["reserve", "--role", "scout", "--count", "2"]);

  assert_eq!((code, out), (0, format!("{head}: 2 of 2 seats held, 0 waiting for a place\n")), "{err}");
  assert_eq!(held_wanted(scratch).await, [2]);

  let (code, out, _) = pool(scratch, &["reserve", "--role", "scout", "--count", "3"]);

  assert_eq!((code, out), (0, format!("{head}: 3 of 3 seats held, 0 waiting for a place\n")));
  assert_eq!(held_wanted(scratch).await, [3]);

  let (code, out, _) = pool(scratch, &["release"]);

  assert_eq!((code, out.as_str()), (0, "1 reservation(s) given back\n"));
  assert!(held_wanted(scratch).await.is_empty());

  for (args, said) in [
    (
      vec!["reserve", "--role", "nobody", "--count", "1"],
      "nobody is not a role of the common spawning pool",
    ),
    (
      vec!["reserve", "--role", "scout", "--tier", "Huge", "--count", "1"],
      "unknown tier: Huge",
    ),
    (
      vec!["reserve", "--role", "scout", "--count", "0"],
      "reserve needs --role <role> and --count <n>",
    ),
    (vec!["reserve", "--count", "1"], "reserve needs --role <role> and --count <n>"),
    (vec!["hold"], "usage: bmo-agency coworker pool"),
  ] {
    let (code, _, err) = pool(scratch, &args);

    assert_eq!(code, 1, "{args:?}");
    assert!(err.contains(&format!("coworker-pool: {said}")), "{args:?}: {err}");
  }

  assert!(held_wanted(scratch).await.is_empty());
}
