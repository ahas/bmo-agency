//! The role slots (docs/role-slots.md): a role's runs are let in under its cap and no further, a slot given
//! back lets the next in, a run whose provider has no quota waits — never refused — and goes once quota
//! comes back, an unknown role is an error, and a delegation naming no role is the supervisor's double: a
//! slot of the doppelganger role on the supervisor's own provider and model.

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
      agent: None,
    },
    place_key: None,
    ..Asked::default()
  }
}

async fn ask(ground: &Ground, role: &str, key: &str) -> dispatch::Request {
  enqueue(&ground.db, &ground.events, &asked(role, key), "job", async |_, _| Ok(()))
    .await
    .unwrap()
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
  assert_eq!(used(&ground.db, "worker", None).await.unwrap(), 3);

  let let_in = dispatch::request(&ground.db, &held[0].token).await.unwrap().unwrap();

  assert_eq!(let_in.provider, "deepseek", "let in on the role's active provider");
  assert_eq!(let_in.model, "deepseek-flash");
  assert_eq!(let_in.capacity, 3, "the cap is the lease's snapshot");

  assert!(finish(&ground.db, &ground.events, &held[0].token, "done", false).await.unwrap());
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
  crate::support::rows::reseated(scratch.db()).await;

  let first = ask(&ground, "worker", "first").await;
  let second = ask(&ground, "worker", "second").await;

  assert!(admitted(&ground, &first).await);
  assert!(!admitted(&ground, &second).await);

  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 2 WHERE "name" = 'worker'"#, vec![])
    .await;
  crate::support::rows::reseated(scratch.db()).await;

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
  assert_eq!(
    used(&ground.db, "worker", None).await.unwrap(),
    0,
    "a run waiting for quota holds no slot"
  );

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
  let refused = enqueue(
    &ground.db,
    &ground.events,
    &asked("general-purpose", "unknown"),
    "native",
    async |_, _| Ok(()),
  )
  .await;

  assert!(refused.is_err_and(|why| why.contains("No role named general-purpose")));
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "role_slot_requests""#, vec![]).await,
    0,
    "nothing starts"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delegation_naming_no_role_is_the_supervisors_double_and_holds_a_doppelganger_slot() {
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

  let fixed = dispatch::fixed_road(&ground.db, "doppelganger", Some("chat"))
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
          role: "doppelganger",
          path: Some("/scratch/chat"),
          fixed: Some(fixed.clone()),
          agent: None,
        },
        asked_by: Some("chat"),
        ..Asked::default()
      };

      held.push(
        enqueue(&ground.db, &ground.events, &delegation, DELEGATE, async |_, _| Ok(()))
          .await
          .unwrap(),
      );
    }

    held
  };

  assert!(admitted(&ground, &doubles[0]).await);
  assert!(admitted(&ground, &doubles[1]).await);
  assert!(
    !admitted(&ground, &doubles[2]).await,
    "the doppelganger role's cap of 2 counts every double"
  );
  assert_eq!(used(&ground.db, "doppelganger", None).await.unwrap(), 2);
  assert_eq!(used(&ground.db, "producer", None).await.unwrap(), 0, "a double holds no producer slot");

  let running = dispatch::request(&ground.db, &doubles[0].token).await.unwrap().unwrap();

  assert_eq!(running.role, "doppelganger");
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
      role: "doppelganger",
      path: Some("/scratch/chat"),
      fixed: Some(agency_server::order::Entry {
        provider: "deepseek".into(),
        model: "deepseek-flash".into(),
        effort: None,
        thinking: None,
        answer_tokens: None,
      }),
      agent: None,
    },
    ..Asked::default()
  };
  let held = enqueue(&ground.db, &ground.events, &delegation, DELEGATE, async |_, _| Ok(()))
    .await
    .unwrap();

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

  assert!(finish(&ground.db, &ground.events, &first.token, "done", false).await.unwrap());

  let spent =
    enqueue(&ground.db, &ground.events, &asked("worker", "again"), "job", async |_, _| Ok(())).await;

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
    &ground.events,
    &dispatch::request(&ground.db, &held.token).await.unwrap().unwrap(),
    "unit-one",
    None,
  )
  .await
  .unwrap();

  let nothing = std::collections::HashSet::new();
  let standing: std::collections::HashSet<String> = ["unit-one".to_owned()].into();

  // A child never seen standing is not read as ended, however long it takes to come up.
  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert_eq!(leases, vec![(held.token.clone(), "unit-one".to_owned(), false)]);
  assert_eq!(dispatch::release_ended(&ground.db, &ground.events, &leases, &nothing).await.unwrap(), 0);
  assert_eq!(dispatch::request(&ground.db, &held.token).await.unwrap().unwrap().state, "active");

  // Seen standing, then gone: given back, as the line's on evidence its run ended.
  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert_eq!(
    dispatch::release_ended(&ground.db, &ground.events, &leases, &standing).await.unwrap(),
    0
  );

  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert!(leases[0].2, "marked seen");
  assert_eq!(dispatch::release_ended(&ground.db, &ground.events, &leases, &nothing).await.unwrap(), 1);

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
  dispatch::bind_session(&ground.db, &ground.events, &held.token, "worker@run:7")
    .await
    .unwrap();

  // A later binding naming the run keeps the lease on its session and the name as the run's.
  let now = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();
  let bound = dispatch::bind(&ground.db, &ground.events, &now, "research-7-review", None)
    .await
    .unwrap();

  assert_eq!(bound.child_id.as_deref(), Some("worker@run:7"));
  assert_eq!(bound.session.as_deref(), Some("worker@run:7"));
  assert_eq!(bound.agent_name.as_deref(), Some("research-7-review"));

  // The pass reads no session-bound lease against units.
  assert!(dispatch::bound_leases(&ground.db).await.unwrap().is_empty());

  assert_eq!(
    dispatch::release_sessions(&ground.db, &ground.events, &["worker@run:7".to_owned()], "ended")
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
  crate::support::rows::reseated(scratch.db()).await;

  let mut first = asked("worker", "first");
  let mut second = asked("worker", "second");

  first.owner = "restarting".to_owned();
  second.owner = "restarting".to_owned();

  let first = enqueue(&ground.db, &ground.events, &first, "job", async |_, _| Ok(())).await.unwrap();
  let second = enqueue(&ground.db, &ground.events, &second, "job", async |_, _| Ok(())).await.unwrap();
  let other = ask(&ground, "worker", "other").await;

  assert!(admitted(&ground, &first).await);
  dispatch::bind_session(&ground.db, &ground.events, &first.token, "worker@run:9")
    .await
    .unwrap();

  let done = dispatch::release_owner(&ground.db, &ground.events, "restarting").await.unwrap();

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
  dispatch::bind_session(&ground.db, &ground.events, &held.token, "worker@run:1")
    .await
    .unwrap();
  dispatch::bind(&ground.db, &ground.events, &read(&held.token).await, "research-1-answer", None)
    .await
    .unwrap();
  assert_eq!(read(&held.token).await.agent_name.as_deref(), Some("research-1-answer"));

  // The round's session starts on the slot before the first ends: the lease moves onto it, and the first
  // session's end no longer gives it back.
  dispatch::bind_session(&ground.db, &ground.events, &held.token, "worker@run:2")
    .await
    .unwrap();

  let moved = read(&held.token).await;

  assert_eq!(
    (moved.child_id.as_deref(), moved.session.as_deref(), moved.agent_name.as_deref()),
    (Some("worker@run:2"), Some("worker@run:2"), None),
    "the run named on the session before stays with it"
  );
  assert_eq!(
    dispatch::release_sessions(&ground.db, &ground.events, &["worker@run:1".to_owned()], "ended")
      .await
      .unwrap(),
    0
  );

  // The review's session, the same way.
  dispatch::bind_session(&ground.db, &ground.events, &held.token, "worker@run:3")
    .await
    .unwrap();
  assert_eq!(
    dispatch::release_sessions(&ground.db, &ground.events, &["worker@run:2".to_owned()], "ended")
      .await
      .unwrap(),
    0
  );
  assert_eq!(read(&held.token).await.state, "active", "still held across three sessions");

  // Starting the same session again keeps what is named on it.
  dispatch::bind(&ground.db, &ground.events, &read(&held.token).await, "research-1-review", None)
    .await
    .unwrap();
  dispatch::bind_session(&ground.db, &ground.events, &held.token, "worker@run:3")
    .await
    .unwrap();
  assert_eq!(read(&held.token).await.agent_name.as_deref(), Some("research-1-review"));

  // The last session's end gives the lease back.
  assert_eq!(
    dispatch::release_sessions(&ground.db, &ground.events, &["worker@run:3".to_owned()], "ended")
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

  let held = enqueue(&ground.db, &ground.events, &lone, "search", async |_, _| Ok(())).await.unwrap();

  assert!(admitted(&ground, &held).await);

  // Bound to nothing, the line's pass reads no end of it: it stands.
  assert!(dispatch::bound_leases(&ground.db).await.unwrap().is_empty());
  assert_eq!(dispatch::request(&ground.db, &held.token).await.unwrap().unwrap().state, "active");

  let done = dispatch::release_owner(&ground.db, &ground.events, "research:185").await.unwrap();

  assert_eq!((done.released, done.cancelled), (1, 0));
  assert!(done.sessions.is_empty(), "no session was bound to it");

  let ended = dispatch::request(&ground.db, &held.token).await.unwrap().unwrap();

  assert_eq!(
    (ended.state.as_str(), ended.ended_by.as_deref()),
    ("released", Some(dispatch::BY_OWNER))
  );
  assert_eq!(used(&ground.db, "worker", None).await.unwrap(), 0, "its slot is free again");
}

// The doppelganger role is seeded with an empty order, so a delegation follows its supervisor; an order the
// owner sets on the role stands over the supervisor's road, and a slot of a role with an empty order that
// no supervisor asks has no road and is refused (docs/role-slots.md, 「A delegated task」).
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

  let seeded_role = agency_server::agent_roles::named(&ground.db, "doppelganger").await.unwrap().unwrap();

  assert!(seeded_role.order.follows_supervisor(), "the double's order is seeded empty");

  let followed = dispatch::fixed_road(&ground.db, "doppelganger", Some("chat"))
    .await
    .unwrap()
    .expect("the supervisor's road");

  assert_eq!(
    (followed.provider.as_str(), followed.model.as_str(), followed.effort.as_deref()),
    ("deepseek", "deepseek-flash", Some("max")),
    "a supervisor with no turn yet goes by its order's first"
  );

  let nobody = dispatch::fixed_road(&ground.db, "doppelganger", None).await;

  assert!(
    nobody.is_err_and(|why| why.contains("no supervisor asked")),
    "an empty order needs its asker"
  );

  // The entry a turn of the double goes out on, on any road, is the asking supervisor's for the provider; a
  // run of a following role that names no supervisor is refused.
  let entry = agency_server::order::row_entry(&ground.db, "doppelganger", "deepseek", Some("chat"))
    .await
    .unwrap();

  assert_eq!((entry.model.as_str(), entry.effort.as_deref()), ("deepseek-flash", Some("max")));
  assert!(
    agency_server::order::row_entry(&ground.db, "doppelganger", "deepseek", None)
      .await
      .is_err_and(|why| why.contains("follows the supervisor")),
    "a following role's run names its supervisor"
  );

  scratch
    .run(
      r#"UPDATE "roles" SET "provider_order" = '[{"provider":"codex","model":"gpt-5.6-terra","effort":"high"}]'
         WHERE "name" = 'doppelganger'"#,
      vec![],
    )
    .await;
  crate::support::rows::reseated(scratch.db()).await;

  assert_eq!(
    dispatch::fixed_road(&ground.db, "doppelganger", Some("chat")).await.unwrap(),
    None,
    "the owner's order stands over the supervisor's road: the slot walks the role's order"
  );
}

// Two doubles that each delegate again under the doppelganger role's cap of 2 would wait for each other
// with no end: once delegated runs hold the whole cap, a double's ask is refused with why, while a
// supervisor's own ask, an ask under a key already standing and an ask of a role whose slots a run of
// another source holds are not (docs/role-slots.md, 「A delegated task」).
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
      agent: None,
    },
    asked_by: Some("chat"),
    ..Asked::default()
  };
  let first = enqueue(
    &ground.db,
    &ground.events,
    &delegation("first", "doppelganger"),
    DELEGATE,
    async |_, _| Ok(()),
  )
  .await
  .unwrap();

  assert!(admitted(&ground, &first).await);
  assert!(
    dispatch::double_would_wait(&ground.db, "doppelganger", "delegate:again")
      .await
      .unwrap()
      .is_none(),
    "one double holds one of two slots: a double may ask"
  );

  let second = enqueue(
    &ground.db,
    &ground.events,
    &delegation("second", "doppelganger"),
    DELEGATE,
    async |_, _| Ok(()),
  )
  .await
  .unwrap();

  assert!(admitted(&ground, &second).await);

  let why = dispatch::double_would_wait(&ground.db, "doppelganger", "delegate:again")
    .await
    .unwrap()
    .expect("both slots are doubles': a double's ask could wait on itself");

  assert!(why.contains("doppelganger") && why.contains("2 of the 2"), "{why}");
  assert!(
    dispatch::double_would_wait(&ground.db, "doppelganger", "delegate:second")
      .await
      .unwrap()
      .is_none(),
    "an ask under a key already standing is answered with its request"
  );

  // A role whose slots a job holds is waited on as the line says: that run ends of its own.
  crate::support::rows::seats(scratch.db(), "producer", &[("deepseek", "deepseek-flash")]).await;

  let job = enqueue(
    &ground.db,
    &ground.events,
    &Asked {
      wanted: Wanted {
        role: "producer",
        path: Some("/checkouts/one"),
        fixed: Some(road.clone()),
        agent: None,
      },
      ..asked("producer", "job")
    },
    "job",
    async |_, _| Ok(()),
  )
  .await
  .unwrap();

  assert!(admitted(&ground, &job).await);
  assert!(
    dispatch::double_would_wait(&ground.db, "producer", "delegate:producer")
      .await
      .unwrap()
      .is_none()
  );

  // A closed role is waited on until the owner raises its cap.
  scratch
    .run(r#"UPDATE "roles" SET "max_concurrent" = 0 WHERE "name" = 'doppelganger'"#, vec![])
    .await;
  crate::support::rows::reseated(scratch.db()).await;

  assert!(
    dispatch::double_would_wait(&ground.db, "doppelganger", "delegate:closed")
      .await
      .unwrap()
      .is_none()
  );
}

// What a run reads off the agent it is let in on (docs/role-agents.md, 「What a run reads off its agent」): its
// seat is found by the slot it holds at the folder it works in, and what the agent lays over its role stands in
// the role's place for that run alone.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_reads_its_prompt_bundles_and_threshold_off_the_agent_it_is_let_in_on() {
  use agency_server::role_agents::{bundles_at, prompt_at, seated_at};

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;

  // The role's first agent lays a prompt, bundles and a threshold of its own over its role's.
  let first = agency_server::role_agents::of_role(db, "worker").await.unwrap().remove(0);

  scratch
    .run(
      r#"UPDATE "role_agents" SET "prompt" = 'Work as the first seat.', "bundles" = '["web.read"]',
           "fold_ratio" = 0.4 WHERE "name" = $1"#,
      vec![first.name.as_str().into()],
    )
    .await;

  let role_prompt = agency_server::agent_roles::prompt_of(db, "worker").await.unwrap();
  let role_bundles = agency_server::agent_roles::bundles_of(db, "worker").await.unwrap();

  // No run stands anywhere: the role's own preset.
  assert_eq!(seated_at(db, Some("worker"), Some("/checkouts/one")).await.unwrap(), None);
  assert_eq!(prompt_at(db, "worker", Some("/checkouts/one")).await.unwrap(), role_prompt);
  assert_eq!(bundles_at(db, "worker", Some("/checkouts/one")).await.unwrap(), role_bundles);

  // A run let in at a folder sits on the role's first free agent, and reads that agent.
  let held = ask(&ground, "worker", "one").await;

  assert!(admitted(&ground, &held).await);

  let seat = seated_at(db, Some("worker"), Some("/checkouts/one")).await.unwrap().unwrap();

  assert_eq!(seat.name, first.name);
  assert_eq!(
    prompt_at(db, "worker", Some("/checkouts/one")).await.unwrap().as_deref(),
    Some("Work as the first seat.")
  );
  assert_eq!(bundles_at(db, "worker", Some("/checkouts/one/")).await.unwrap(), ["web.read"]);
  // The folder alone names the seat too, whatever its role, and a run that names no folder takes its role's
  // only active request.
  assert_eq!(seated_at(db, None, Some("/checkouts/one")).await.unwrap().unwrap().name, first.name);
  assert_eq!(seated_at(db, Some("worker"), None).await.unwrap().unwrap().name, first.name);
  // Another folder, and another role at the same folder, name no seat.
  assert_eq!(seated_at(db, Some("worker"), Some("/checkouts/two")).await.unwrap(), None);
  assert_eq!(seated_at(db, Some("coder"), Some("/checkouts/one")).await.unwrap(), None);
  assert_eq!(prompt_at(db, "worker", Some("/checkouts/two")).await.unwrap(), role_prompt);

  // The job's threshold is its agent's where the agent sets one, and its role's elsewhere.
  let role_ratio = agency_server::agent_roles::named(db, "worker").await.unwrap().unwrap().fold_ratio;

  assert_eq!(
    agency_server::worker_session::fold::worker_fold_ratio(
      &ground,
      "deepseek",
      None,
      Some("/checkouts/one")
    )
    .await,
    0.4
  );
  assert_eq!(
    agency_server::worker_session::fold::worker_fold_ratio(
      &ground,
      "deepseek",
      None,
      Some("/checkouts/two")
    )
    .await,
    role_ratio
  );

  // The slot given back, nothing stands on the agent, and the role's own preset is read again.
  assert!(finish(db, &ground.events, &held.token, "done", false).await.unwrap());
  assert_eq!(seated_at(db, Some("worker"), Some("/checkouts/one")).await.unwrap(), None);
  assert_eq!(bundles_at(db, "worker", Some("/checkouts/one")).await.unwrap(), role_bundles);
}

// A role that holds no agent is given its agents from what its row names (docs/role-agents.md, 「The roles the
// policy seeds」): as many as its count, each on the first road of its order whose program carries the role's
// line, or on the provider forced on it.
#[tokio::test(flavor = "multi_thread")]
async fn a_role_is_given_its_agents_on_the_first_road_of_its_order_that_carries_its_line() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;
  let order = r#"[{"provider":"deepseek","model":"deepseek-flash","effort":"high"},
    {"provider":"registered","model":"flash-next-coder"},
    {"provider":"claude-code","model":"claude-opus-5-5","effort":"high"}]"#;

  for role in ["worker", "voyager", "seeker", "translator"] {
    scratch
      .run(
        r#"UPDATE "roles" SET "provider_order" = $1, "max_concurrent" = 2, "active_provider" = NULL,
             "active_model" = NULL WHERE "name" = $2"#,
        vec![order.into(), role.into()],
      )
      .await;
  }

  scratch
    .run(
      r#"UPDATE "roles" SET "provider_order" = $1, "max_concurrent" = 1, "active_provider" = 'claude-code',
           "active_model" = 'claude-sonnet-5' WHERE "name" = 'producer'"#,
      vec![order.into()],
    )
    .await;
  crate::support::rows::reseated(db).await;

  let mut roads = vec![];

  for role in ["worker", "voyager", "seeker", "translator", "producer"] {
    let agents = agency_server::role_agents::of_role(db, role).await.unwrap();
    let first = agents.first().unwrap();

    assert!(
      agents
        .iter()
        .all(|agent| agent.provider == first.provider && agent.model == first.model)
    );
    roads.push((
      role,
      agents.len(),
      first.provider.clone().unwrap(),
      first.model.clone().unwrap(),
      first.effort.clone(),
    ));
  }

  assert_eq!(
    roads,
    [
      // The worker line and the search line stand on the order's first road.
      ("worker", 2, "deepseek".to_owned(), "deepseek-flash".to_owned(), Some("high".to_owned())),
      (
        "voyager",
        2,
        "deepseek".to_owned(),
        "deepseek-flash".to_owned(),
        Some("high".to_owned())
      ),
      // The browse line is carried by Claude Code alone, and the translate line by no road before the registry.
      (
        "seeker",
        2,
        "claude-code".to_owned(),
        "claude-opus-5-5".to_owned(),
        Some("high".to_owned())
      ),
      ("translator", 2, "registered".to_owned(), "flash-next-coder".to_owned(), None),
      // A forced provider stands on its forced model, at the effort the order names for it.
      (
        "producer",
        1,
        "claude-code".to_owned(),
        "claude-sonnet-5".to_owned(),
        Some("high".to_owned())
      ),
    ]
  );
}

// A role that holds an agent on a road of its own beside agents that name none (docs/role-agents.md, 「Which
// agent a run takes」): a run a supervisor asks takes the first free agent in the one order, an agent that
// names no road on that supervisor's road; a run no supervisor asks passes those agents over; and a run that
// names a road no agent stands on is let in on an agent that names none.
#[tokio::test(flavor = "multi_thread")]
async fn a_role_of_both_kinds_of_agent_seats_a_supervisors_run_in_one_order() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "provider_order", "last_mode",
           "last_model", "last_effort", "created_at", "updated_at")
         VALUES ('chat', 'agent', 'window', '[{"provider":"deepseek","model":"deepseek-flash"}]', 'deepseek',
           'deepseek-flash', 'max', 0, 0)"#,
      vec![],
    )
    .await;
  // The coder's second agent stands on a road of its own; its first and third name none.
  scratch
    .run(
      r#"UPDATE "role_agents" SET "provider" = 'deepseek', "model" = 'deepseek-flash', "effort" = 'low'
         WHERE "name" = 'coder-02'"#,
      vec![],
    )
    .await;

  let ask =
    async |key: &str, asked_by: Option<&'static str>, fixed: Option<agency_server::order::Entry>| {
      let asked = Asked {
        request_key: format!("mixed:{key}"),
        owner: format!("mixed:{key}"),
        wanted: Wanted {
          role: "coder",
          path: Some("/scratch/mixed"),
          fixed,
          agent: None,
        },
        asked_by,
        ..Asked::default()
      };
      let held = enqueue(db, &ground.events, &asked, DELEGATE, async |_, _| Ok(())).await.unwrap();

      acquire(&ground, &held).await.unwrap()
    };
  let seat = |request: &Option<dispatch::Request>| {
    request
      .as_ref()
      .map(|let_in| (let_in.seat.clone().unwrap_or_default(), let_in.effort.clone()))
  };

  // No supervisor asked: the agents that name no road are passed over, and the run sits on the one that does.
  let alone = ask("alone", None, None).await;

  assert_eq!(seat(&alone), Some(("coder-02".to_owned(), Some("low".to_owned()))));
  // A second run nobody asked waits for that agent, though two agents stand free.
  assert_eq!(seat(&ask("alone-again", None, None).await), None);

  // A supervisor's runs take the free agents in order, each on the supervisor's own road.
  let first = ask("first", Some("chat"), None).await;
  let third = ask("third", Some("chat"), None).await;

  assert_eq!(seat(&first), Some(("coder-01".to_owned(), Some("max".to_owned()))));
  assert_eq!(seat(&third), Some(("coder-03".to_owned(), Some("max".to_owned()))));
  assert_eq!(first.as_ref().unwrap().model, "deepseek-flash");

  // With coder-02 free again, a supervisor's run takes it on the agent's own road, in its place in the order.
  assert!(finish(db, &ground.events, &alone.unwrap().token, "done", false).await.unwrap());
  assert!(finish(db, &ground.events, &first.unwrap().token, "done", false).await.unwrap());

  // The waiting run nobody asked is ahead in the line and takes coder-02; the supervisor's next takes coder-01.
  let waited = dispatch::requests(db, &["waiting"]).await.unwrap();
  let waited = acquire(&ground, &waited[0]).await.unwrap();

  assert_eq!(seat(&waited), Some(("coder-02".to_owned(), Some("low".to_owned()))));
  assert_eq!(
    seat(&ask("again", Some("chat"), None).await),
    Some(("coder-01".to_owned(), Some("max".to_owned())))
  );

  // A run naming a road no agent stands on is let in on an agent that names none; with none of those free it
  // waits, and it is never refused while the role holds one.
  let pinned = agency_server::order::Entry {
    provider: "claude-code".to_owned(),
    model: "claude-opus-5".to_owned(),
    effort: Some("high".to_owned()),
    thinking: None,
    answer_tokens: None,
  };

  assert_eq!(seat(&ask("pinned", None, Some(pinned)).await), None);
}

// Whether a run keeps its session is its agent's own (docs/role-agents.md, 「What an agent row holds」): a run
// reads the sessions setting of the agent it is let in on.
#[tokio::test(flavor = "multi_thread")]
async fn a_run_keeps_its_session_as_its_agent_says() {
  use agency_server::role_agents::sessions_at;
  use policy::roles::Sessions;

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;
  // The worker's first agent keeps no session; its others keep theirs.
  scratch
    .run(r#"UPDATE "role_agents" SET "sessions" = 'fresh' WHERE "name" = 'worker-01'"#, vec![])
    .await;

  // No run stands at the folder, so no agent can be told and nothing is kept.
  assert_eq!(sessions_at(db, "worker", Some("/checkouts/one")).await.unwrap(), Sessions::Fresh);

  let first = ask(&ground, "worker", "one").await;
  let second = Asked {
    wanted: Wanted {
      role: "worker",
      path: Some("/checkouts/two"),
      fixed: None,
      agent: None,
    },
    ..asked("worker", "two")
  };
  let second = enqueue(db, &ground.events, &second, "job", async |_, _| Ok(())).await.unwrap();

  assert!(admitted(&ground, &first).await && admitted(&ground, &second).await);
  assert_eq!(sessions_at(db, "worker", Some("/checkouts/one")).await.unwrap(), Sessions::Fresh);
  assert_eq!(sessions_at(db, "worker", Some("/checkouts/two")).await.unwrap(), Sessions::Keep);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_that_names_an_agent_is_let_in_on_that_agent_alone_and_waits_for_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;

  let agents = agency_server::role_agents::of_role(db, "worker").await.unwrap();
  let second = agents[1].name.clone();
  let wanted = |key: &str| {
    let mut asked = asked("worker", key);

    asked.wanted.agent = Some(second.as_str());
    asked
  };

  // The first ask naming the second agent is let in on it, passing the first agent over.
  let one = enqueue(db, &ground.events, &wanted("one"), "job", async |_, _| Ok(())).await.unwrap();

  assert!(admitted(&ground, &one).await, "the named agent is free");

  let let_in = dispatch::request(db, &one.token).await.unwrap().unwrap();

  assert_eq!(let_in.agent_wanted.as_deref(), Some(second.as_str()));
  assert_eq!(
    let_in.seat.as_deref(),
    Some(second.as_str()),
    "let in on the agent it named, not the first"
  );

  // A second ask naming the same agent waits, though the role has free agents.
  let two = enqueue(db, &ground.events, &wanted("two"), "job", async |_, _| Ok(())).await.unwrap();

  assert!(!admitted(&ground, &two).await, "the named agent carries a run, so the ask waits");
  assert!(
    admitted(&ground, &ask(&ground, "worker", "three").await).await,
    "an ask naming none takes another agent"
  );

  // Given back, the named agent takes the waiting ask.
  finish(db, &ground.events, &one.token, "ended", true).await.unwrap();
  assert!(admitted(&ground, &two).await);
  assert_eq!(
    dispatch::request(db, &two.token).await.unwrap().unwrap().seat.as_deref(),
    Some(second.as_str())
  );

  // An agent the role does not hold is an error.
  let mut stranger = asked("worker", "four");

  stranger.wanted.agent = Some("nobody-01");
  assert!(enqueue(db, &ground.events, &stranger, "job", async |_, _| Ok(())).await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_accounts_runs_take_its_own_agents_and_count_against_its_own_cap() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "provider_order", "last_mode",
           "last_model", "last_effort", "created_at", "updated_at", "owner_account")
         VALUES ('secretary-a', 'agent', 'window', '[{"provider":"deepseek","model":"deepseek-flash"}]', 'deepseek',
           'deepseek-flash', 'high', 0, 0, 7)"#,
      vec![],
    )
    .await;

  let of_account = |key: &'static str| Asked {
    asked_by: Some("secretary-a"),
    ..asked("worker", key)
  };
  let first = enqueue(&ground.db, &ground.events, &of_account("account-one"), "job", async |_, _| Ok(()))
    .await
    .unwrap();

  assert_eq!(first.account.as_deref(), Some("7"), "a slot belongs to its asking supervisor's account");
  assert!(
    !admitted(&ground, &first).await,
    "an account with no agent of the role runs nothing of it"
  );

  // One seat of the role for the account, on the road the deployment's workers stand on.
  let mut seat = agency_server::role_agents::of_role(&ground.db, "worker").await.unwrap().remove(0);

  seat.name = "worker-account-7".into();
  seat.owner_account = Some("7".into());
  agency_server::role_agents::write(&ground.db, &seat).await.unwrap();

  // The deployment's three seats fill with its own runs, and the account's run still takes its own.
  let mut deployment = vec![];

  for key in ["one", "two", "three"] {
    deployment.push(ask(&ground, "worker", key).await);
  }

  for request in &deployment {
    assert!(admitted(&ground, request).await, "the deployment's cap is its own seats'");
  }

  assert!(admitted(&ground, &first).await, "the account's run takes the account's seat");

  let let_in = dispatch::request(&ground.db, &first.token).await.unwrap().unwrap();

  assert_eq!(let_in.seat.as_deref(), Some("worker-account-7"));
  assert_eq!(used(&ground.db, "worker", Some("7")).await.unwrap(), 1);
  assert_eq!(used(&ground.db, "worker", None).await.unwrap(), 3);

  // A second run of the account waits on the account's cap of one, whatever the deployment's seats do.
  let second = enqueue(&ground.db, &ground.events, &of_account("account-two"), "job", async |_, _| Ok(()))
    .await
    .unwrap();

  assert!(!admitted(&ground, &second).await, "the account's cap is its own agents switched on");
  assert!(finish(&ground.db, &ground.events, &deployment[0].token, "done", false).await.unwrap());
  assert!(
    !admitted(&ground, &second).await,
    "a deployment seat given back is no seat of the account's"
  );
  assert!(finish(&ground.db, &ground.events, &first.token, "done", false).await.unwrap());
  assert!(
    admitted(&ground, &second).await,
    "the account's own seat given back lets its next run in"
  );
}
