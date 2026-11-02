//! The role slots (docs/role-slots.md): a role's runs are let in under its cap and no further, a slot given
//! back lets the next in, a run whose provider has no quota waits — never refused — and goes once quota comes
//! back, an unknown role is an error, and a delegation naming no role is the supervisor's double: a slot of the
//! delegator role on the supervisor's own provider and model.

use agency_server::order_run::dispatch::{self, Asked, DELEGATE, Wanted, acquire, enqueue, finish, used};
use agency_server::places::Ground;

use crate::support::grounds::{deepseek_balance, seeded};
use crate::support::scratch::Scratch;

fn asked<'a>(role: &'a str, key: &str) -> Asked<'a> {
  Asked {
    request_key: format!("test:{key}"),
    owner: format!("owner:{key}"),
    wanted: Wanted {
      role,
      path: Some("/checkouts/one"),
      fixed: None,
    },
    place_key: None,
    ..Asked::default()
  }
}

async fn ask(ground: &Ground, role: &str, key: &str) -> dispatch::Request {
  enqueue(&ground.db, &asked(role, key), "job", async |_, _| Ok(())).await.unwrap()
}

async fn admitted(ground: &Ground, held: &dispatch::Request) -> bool {
  let now = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();

  acquire(ground, &now).await.unwrap().is_some()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_roles_runs_are_let_in_up_to_its_cap_and_the_next_waits_for_a_slot_given_back() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let mut held = vec![];

  for key in ["one", "two", "three", "four"] {
    held.push(ask(&ground, "worker", key).await);
  }

  for request in &held[..3] {
    assert!(admitted(&ground, request).await, "within the worker role's cap of 3");
  }

  assert!(!admitted(&ground, &held[3]).await, "the fourth waits for a slot");
  assert_eq!(used(&ground.db, "worker").await.unwrap(), 3);

  let let_in = dispatch::request(&ground.db, &held[0].token).await.unwrap().unwrap();

  assert_eq!(let_in.provider, "deepseek", "let in on the role's active provider");
  assert_eq!(let_in.model, "deepseek-flash");
  assert_eq!(let_in.capacity, 3, "the cap is the lease's snapshot");

  assert!(finish(&ground.db, &held[0].token, "done", false).await.unwrap());
  assert!(admitted(&ground, &held[3]).await, "a slot given back lets the next in");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_raised_cap_lets_in_the_runs_waiting_at_their_next_ask() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 1 WHERE "name" = 'worker'"#, vec![])
    .await;

  let first = ask(&ground, "worker", "first").await;
  let second = ask(&ground, "worker", "second").await;

  assert!(admitted(&ground, &first).await);
  assert!(!admitted(&ground, &second).await);

  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 2 WHERE "name" = 'worker'"#, vec![])
    .await;

  assert!(admitted(&ground, &second).await, "the cap is read as every run asks to be let in");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_whose_providers_have_no_quota_waits_with_the_reason_and_goes_once_quota_comes_back() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  // DeepSeek's balance spent and every other provider of the worker's order switched off: none has quota.
  deepseek_balance(&ground, 0.0).await;

  for provider in ["claude-code", "codex", "openrouter", "registered"] {
    agency_server::control::harness::set_provider_enabled(&ground.db, provider, false)
      .await
      .unwrap();
  }

  let waiting = ask(&ground, "worker", "waits").await;

  let let_in = admitted(&ground, &waiting).await;
  let seen = dispatch::request(&ground.db, &waiting.token).await.unwrap().unwrap();

  assert!(!let_in, "no provider of the order has quota: it waits: {seen:?}");

  let held = dispatch::request(&ground.db, &waiting.token).await.unwrap().unwrap();

  assert_eq!(held.state, "waiting", "a run waiting for quota is never refused");
  assert!(held.held_for.as_deref().is_some_and(|why| why.contains("deepseek")), "{held:?}");
  assert_eq!(used(&ground.db, "worker").await.unwrap(), 0, "a run waiting for quota holds no slot");

  deepseek_balance(&ground, 5.0).await;

  assert!(admitted(&ground, &waiting).await, "quota back: the run goes out");

  let gone = dispatch::request(&ground.db, &waiting.token).await.unwrap().unwrap();

  assert_eq!(gone.held_for, None);
  assert_eq!(gone.provider, "deepseek");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slot_asked_under_a_role_the_store_does_not_hold_is_refused() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let refused =
    enqueue(&ground.db, &asked("general-purpose", "unknown"), "native", async |_, _| Ok(())).await;

  assert!(refused.is_err_and(|why| why.contains("No role named general-purpose")));
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "role_slot_requests""#, vec![]).await,
    0,
    "nothing starts"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delegation_naming_no_role_is_the_supervisors_double_and_holds_a_delegator_slot() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "provider_order", "last_mode",
           "last_model", "last_effort", "created_at", "updated_at")
         VALUES ('chat', 'agent', 'window', '[{"provider":"claude-code","model":"claude-opus-5"}]', 'deepseek',
           'deepseek-flash', 'high', 0, 0)"#,
      vec![],
    )
    .await;

  let fixed = dispatch::fixed_road(&ground.db, "delegator", Some("chat"))
    .await
    .unwrap()
    .expect("the supervisor's own road");

  assert_eq!((fixed.provider.as_str(), fixed.model.as_str()), ("deepseek", "deepseek-flash"));
  assert_eq!(fixed.effort.as_deref(), Some("high"), "at the effort the supervisor stands on");

  let doubles: Vec<_> = {
    let mut held = vec![];

    for key in ["first", "second", "third"] {
      let delegation = Asked {
        request_key: format!("delegate:{key}"),
        owner: format!("delegate:{key}"),
        wanted: Wanted {
          role: "delegator",
          path: Some("/scratch/chat"),
          fixed: Some(fixed.clone()),
        },
        asked_by: Some("chat"),
        ..Asked::default()
      };

      held.push(enqueue(&ground.db, &delegation, DELEGATE, async |_, _| Ok(())).await.unwrap());
    }

    held
  };

  assert!(admitted(&ground, &doubles[0]).await);
  assert!(admitted(&ground, &doubles[1]).await);
  assert!(
    !admitted(&ground, &doubles[2]).await,
    "the delegator role's cap of 2 counts every double"
  );
  assert_eq!(used(&ground.db, "delegator").await.unwrap(), 2);
  assert_eq!(used(&ground.db, "helper").await.unwrap(), 0, "a double holds no helper slot");

  let running = dispatch::request(&ground.db, &doubles[0].token).await.unwrap().unwrap();

  assert_eq!(running.role, "delegator");
  assert_eq!((running.provider.as_str(), running.model.as_str()), ("deepseek", "deepseek-flash"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_double_waits_on_its_supervisors_provider_and_never_moves_to_the_roles_order() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 0.0).await;

  let delegation = Asked {
    request_key: "delegate:waits".into(),
    owner: "delegate:waits".into(),
    wanted: Wanted {
      role: "delegator",
      path: Some("/scratch/chat"),
      fixed: Some(agency_server::order::Entry {
        provider: "deepseek".into(),
        model: "deepseek-flash".into(),
        effort: None,
        thinking: None,
        answer_tokens: None,
      }),
    },
    ..Asked::default()
  };
  let held = enqueue(&ground.db, &delegation, DELEGATE, async |_, _| Ok(())).await.unwrap();

  assert!(!admitted(&ground, &held).await, "its supervisor's provider has no quota: it waits");

  deepseek_balance(&ground, 5.0).await;

  assert!(admitted(&ground, &held).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_key_answers_its_standing_request_and_is_spent_once_the_request_is_given_back() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let first = ask(&ground, "worker", "again").await;
  let again = ask(&ground, "worker", "again").await;

  assert_eq!(again.token, first.token, "asked again while it stands, the key answers its request");

  assert!(finish(&ground.db, &first.token, "done", false).await.unwrap());

  let spent = enqueue(&ground.db, &asked("worker", "again"), "job", async |_, _| Ok(())).await;

  assert!(spent.is_err(), "a key whose request was given back opens nothing: {spent:?}");
  assert_ne!(
    ask(&ground, "worker", "afresh").await.token,
    first.token,
    "a new key opens a new request"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bound_lease_is_given_back_only_once_its_child_was_seen_standing_and_then_is_not() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let held = ask(&ground, "worker", "bound").await;

  assert!(admitted(&ground, &held).await);
  dispatch::bind(
    &ground.db,
    &dispatch::request(&ground.db, &held.token).await.unwrap().unwrap(),
    "unit-one",
  )
  .await
  .unwrap();

  let nothing = std::collections::HashSet::new();
  let standing: std::collections::HashSet<String> = ["unit-one".to_owned()].into();

  // A child never seen standing is not read as ended, however long it takes to come up.
  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert_eq!(leases, vec![(held.token.clone(), "unit-one".to_owned(), false)]);
  assert_eq!(dispatch::release_ended(&ground.db, &leases, &nothing).await.unwrap(), 0);
  assert_eq!(dispatch::request(&ground.db, &held.token).await.unwrap().unwrap().state, "active");

  // Seen standing, then gone: given back, as the line's on evidence its run ended.
  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert_eq!(dispatch::release_ended(&ground.db, &leases, &standing).await.unwrap(), 0);

  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert!(leases[0].2, "marked seen");
  assert_eq!(dispatch::release_ended(&ground.db, &leases, &nothing).await.unwrap(), 1);

  let ended = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();

  assert_eq!(
    (ended.state.as_str(), ended.ended_by.as_deref()),
    ("released", Some(dispatch::BY_RUN_ENDED))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lease_bound_to_a_role_session_ends_with_that_session() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let held = ask(&ground, "worker", "session").await;

  assert!(admitted(&ground, &held).await);
  dispatch::bind_session(&ground.db, &held.token, "worker@run:7").await.unwrap();

  // A later binding naming the run keeps the lease on its session and the name as the run's.
  let now = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();
  let bound = dispatch::bind(&ground.db, &now, "research-7-review").await.unwrap();

  assert_eq!(bound.child_id.as_deref(), Some("worker@run:7"));
  assert_eq!(bound.session.as_deref(), Some("worker@run:7"));
  assert_eq!(bound.agent_name.as_deref(), Some("research-7-review"));

  // The pass reads no session-bound lease against units.
  assert!(dispatch::bound_leases(&ground.db).await.unwrap().is_empty());

  assert_eq!(
    dispatch::release_sessions(&ground.db, &["worker@run:7".to_owned()], "ended")
      .await
      .unwrap(),
    1
  );

  let ended = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();

  assert_eq!(
    (ended.state.as_str(), ended.ended_by.as_deref()),
    ("released", Some(dispatch::BY_RUN_ENDED))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_owner_s_release_gives_back_everything_it_holds_and_nothing_of_another_owner() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 1 WHERE "name" = 'worker'"#, vec![])
    .await;

  let mut first = asked("worker", "first");
  let mut second = asked("worker", "second");

  first.owner = "restarting".to_owned();
  second.owner = "restarting".to_owned();

  let first = enqueue(&ground.db, &first, "job", async |_, _| Ok(())).await.unwrap();
  let second = enqueue(&ground.db, &second, "job", async |_, _| Ok(())).await.unwrap();
  let other = ask(&ground, "worker", "other").await;

  assert!(admitted(&ground, &first).await);
  dispatch::bind_session(&ground.db, &first.token, "worker@run:9").await.unwrap();

  let done = dispatch::release_owner(&ground.db, "restarting").await.unwrap();

  assert_eq!((done.released, done.cancelled), (1, 1));
  assert_eq!(done.sessions, vec!["worker@run:9".to_owned()]);

  for token in [&first.token, &second.token] {
    let ended = dispatch::request(&ground.db, token).await.unwrap().unwrap();

    assert_eq!(ended.ended_by.as_deref(), Some(dispatch::BY_OWNER));
  }

  assert_eq!(dispatch::request(&ground.db, &other.token).await.unwrap().unwrap().state, "waiting");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slot_asked_for_a_model_alone_goes_out_on_the_provider_its_role_s_order_names_it_under() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let role = dispatch::role_of(&ground.db, "worker").await.unwrap();
  let entry = role.order.first().unwrap().clone();

  assert_eq!(
    dispatch::provider_for_model(&ground, "worker", &entry.model).await.unwrap(),
    entry.provider,
    "the entry naming that model"
  );

  // A model no entry names goes out on the role's active provider, else its order's first.
  let provider = dispatch::provider_for_model(&ground, "worker", "no-such-model").await.unwrap();

  assert!(role.order.entries.iter().any(|one| one.provider == provider), "{provider}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slot_carries_three_sessions_in_turn_and_is_given_back_with_the_last() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let held = ask(&ground, "worker", "rounds").await;
  let read = async |token: &str| dispatch::request(&ground.db, token).await.unwrap().unwrap();

  assert!(admitted(&ground, &held).await);

  // The first answer's session, a run named on it by a binding.
  dispatch::bind_session(&ground.db, &held.token, "worker@run:1").await.unwrap();
  dispatch::bind(&ground.db, &read(&held.token).await, "research-1-answer").await.unwrap();
  assert_eq!(read(&held.token).await.agent_name.as_deref(), Some("research-1-answer"));

  // The round's session starts on the slot before the first ends: the lease moves onto it, and the first
  // session's end no longer gives it back.
  dispatch::bind_session(&ground.db, &held.token, "worker@run:2").await.unwrap();

  let moved = read(&held.token).await;

  assert_eq!(
    (moved.child_id.as_deref(), moved.session.as_deref(), moved.agent_name.as_deref()),
    (Some("worker@run:2"), Some("worker@run:2"), None),
    "the run named on the session before stays with it"
  );
  assert_eq!(
    dispatch::release_sessions(&ground.db, &["worker@run:1".to_owned()], "ended")
      .await
      .unwrap(),
    0
  );

  // The review's session, the same way.
  dispatch::bind_session(&ground.db, &held.token, "worker@run:3").await.unwrap();
  assert_eq!(
    dispatch::release_sessions(&ground.db, &["worker@run:2".to_owned()], "ended")
      .await
      .unwrap(),
    0
  );
  assert_eq!(read(&held.token).await.state, "active", "still held across three sessions");

  // Starting the same session again keeps what is named on it.
  dispatch::bind(&ground.db, &read(&held.token).await, "research-1-review").await.unwrap();
  dispatch::bind_session(&ground.db, &held.token, "worker@run:3").await.unwrap();
  assert_eq!(read(&held.token).await.agent_name.as_deref(), Some("research-1-review"));

  // The last session's end gives the lease back.
  assert_eq!(
    dispatch::release_sessions(&ground.db, &["worker@run:3".to_owned()], "ended")
      .await
      .unwrap(),
    1
  );

  let ended = read(&held.token).await;

  assert_eq!(
    (ended.state.as_str(), ended.ended_by.as_deref()),
    ("released", Some(dispatch::BY_RUN_ENDED))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lease_let_in_whose_owner_never_starts_a_session_stands_until_its_owner_releases_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let mut lone = asked("worker", "never-started");

  lone.owner = "research:185".to_owned();

  let held = enqueue(&ground.db, &lone, "search", async |_, _| Ok(())).await.unwrap();

  assert!(admitted(&ground, &held).await);

  // Bound to nothing, the line's pass reads no end of it: it stands.
  assert!(dispatch::bound_leases(&ground.db).await.unwrap().is_empty());
  assert_eq!(dispatch::request(&ground.db, &held.token).await.unwrap().unwrap().state, "active");

  let done = dispatch::release_owner(&ground.db, "research:185").await.unwrap();

  assert_eq!((done.released, done.cancelled), (1, 0));
  assert!(done.sessions.is_empty(), "no session was bound to it");

  let ended = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();

  assert_eq!(
    (ended.state.as_str(), ended.ended_by.as_deref()),
    ("released", Some(dispatch::BY_OWNER))
  );
  assert_eq!(used(&ground.db, "worker").await.unwrap(), 0, "its slot is free again");
}

// The delegator role is seeded with an empty order, so a delegation follows its supervisor; an order the owner
// sets on the role stands over the supervisor's road, and a slot of a role with an empty order that no
// supervisor asks has no road and is refused (docs/role-slots.md, 「A delegated task」).
#[tokio::test(flavor = "multi_thread")]
async fn a_delegation_follows_its_supervisor_until_the_owner_sets_the_role_an_order() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "provider_order", "last_mode",
           "last_model", "last_effort", "created_at", "updated_at")
         VALUES ('chat', 'agent', 'window', '[{"provider":"deepseek","model":"deepseek-flash","effort":"max"}]',
           NULL, NULL, NULL, 0, 0)"#,
      vec![],
    )
    .await;

  let seeded_role = agency_server::agent_roles::named(&ground.db, "delegator").await.unwrap().unwrap();

  assert!(seeded_role.order.follows_supervisor(), "the double's order is seeded empty");

  let followed = dispatch::fixed_road(&ground.db, "delegator", Some("chat"))
    .await
    .unwrap()
    .expect("the supervisor's road");

  assert_eq!(
    (followed.provider.as_str(), followed.model.as_str(), followed.effort.as_deref()),
    ("deepseek", "deepseek-flash", Some("max")),
    "a supervisor with no turn yet goes by its order's first"
  );

  let nobody = dispatch::fixed_road(&ground.db, "delegator", None).await;

  assert!(
    nobody.is_err_and(|why| why.contains("no supervisor asked")),
    "an empty order needs its asker"
  );

  // The entry a turn of the double goes out on, on any road, is the asking supervisor's for the provider; a run
  // of a following role that names no supervisor is refused.
  let entry = agency_server::order::row_entry(&ground.db, "delegator", "deepseek", Some("chat"))
    .await
    .unwrap();

  assert_eq!((entry.model.as_str(), entry.effort.as_deref()), ("deepseek-flash", Some("max")));
  assert!(
    agency_server::order::row_entry(&ground.db, "delegator", "deepseek", None)
      .await
      .is_err_and(|why| why.contains("follows the supervisor")),
    "a following role's run names its supervisor"
  );

  scratch
    .run(
      r#"UPDATE "roles" SET "provider_order" = '[{"provider":"codex","model":"gpt-5.6-terra","effort":"high"}]'
         WHERE "name" = 'delegator'"#,
      vec![],
    )
    .await;

  assert_eq!(
    dispatch::fixed_road(&ground.db, "delegator", Some("chat")).await.unwrap(),
    None,
    "the owner's order stands over the supervisor's road: the slot walks the role's order"
  );
}

// Two doubles that each delegate again under the delegator role's cap of 2 would wait for each other with no
// end: once delegated runs hold the whole cap, a double's ask is refused with why, while a supervisor's own ask,
// an ask under a key already standing and an ask of a role whose slots a run of another source holds are not
// (docs/role-slots.md, 「A delegated task」).
#[tokio::test(flavor = "multi_thread")]
async fn a_double_s_ask_is_refused_where_delegated_runs_hold_the_whole_cap() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let road = agency_server::order::Entry {
    provider: "deepseek".into(),
    model: "deepseek-flash".into(),
    effort: None,
    thinking: None,
    answer_tokens: None,
  };
  let delegation = |key: &str, role: &'static str| Asked {
    request_key: format!("delegate:{key}"),
    owner: format!("delegate:{key}"),
    wanted: Wanted {
      role,
      path: Some("/scratch/chat"),
      fixed: Some(road.clone()),
    },
    asked_by: Some("chat"),
    ..Asked::default()
  };
  let first = enqueue(&ground.db, &delegation("first", "delegator"), DELEGATE, async |_, _| Ok(()))
    .await
    .unwrap();

  assert!(admitted(&ground, &first).await);
  assert!(
    dispatch::double_would_wait(&ground.db, "delegator", "delegate:again")
      .await
      .unwrap()
      .is_none(),
    "one double holds one of two slots: a double may ask"
  );

  let second = enqueue(&ground.db, &delegation("second", "delegator"), DELEGATE, async |_, _| Ok(()))
    .await
    .unwrap();

  assert!(admitted(&ground, &second).await);

  let why = dispatch::double_would_wait(&ground.db, "delegator", "delegate:again")
    .await
    .unwrap()
    .expect("both slots are doubles': a double's ask could wait on itself");

  assert!(why.contains("delegator") && why.contains("2 of the 2"), "{why}");
  assert!(
    dispatch::double_would_wait(&ground.db, "delegator", "delegate:second")
      .await
      .unwrap()
      .is_none(),
    "an ask under a key already standing is answered with its request"
  );

  // A role whose slots a job holds is waited on as the line says: that run ends of its own.
  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 1 WHERE "name" = 'helper'"#, vec![])
    .await;

  let job = enqueue(
    &ground.db,
    &Asked {
      wanted: Wanted {
        role: "helper",
        path: Some("/checkouts/one"),
        fixed: Some(road.clone()),
      },
      ..asked("helper", "job")
    },
    "job",
    async |_, _| Ok(()),
  )
  .await
  .unwrap();

  assert!(admitted(&ground, &job).await);
  assert!(
    dispatch::double_would_wait(&ground.db, "helper", "delegate:helper")
      .await
      .unwrap()
      .is_none()
  );

  // A closed role is waited on until the owner raises its cap.
  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 0 WHERE "name" = 'delegator'"#, vec![])
    .await;

  assert!(
    dispatch::double_would_wait(&ground.db, "delegator", "delegate:closed")
      .await
      .unwrap()
      .is_none()
  );
}
