//! The source a run takes as it starts (docs/supply.md, 「Choosing a source as a run starts」): the deployment's
//! own runs and the administrator's the platform's, unreserved and unasked; a person's with no hook their own key
//! of the road, else the platform unreserved, never their own on the registry's provider; with a hook its order
//! walked past a refused reservation to the first accepted, the run closed with the policy's sentence where every
//! source refuses, and personal supply alone where the hook fails or answers what does not read.

use agency_server::control::claude_accounts::moment_words;
use agency_server::supply::reservations::held;
use agency_server::supply::{Choice, Chosen, Source, choose};
use axum::http::StatusCode;
use policy::fill_marks;
use policy::supply::SUPPLY;
use serde_json::json;

use super::{Consumer, PERSON, asked, personal_key, row_of, state_of};
use crate::support::grounds::ground_on;
use crate::support::scratch::Scratch;

fn going(choice: Choice) -> Chosen {
  match choice {
    Choice::Go(chosen) => chosen,
    Choice::Closed(closed) => panic!("the run was due to go out: {closed:?}"),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_deployment_s_own_runs_and_the_administrator_s_go_out_unreserved_and_unasked() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;

  consumer.register(&ground);
  scratch
    .run(r#"INSERT INTO "settings" ("key", "value") VALUES ('users_administrator', '1')"#, vec![])
    .await;

  for account in [None, Some(1)] {
    let chosen = going(choose(&ground, &asked(account, "deepseek")).await.unwrap());

    assert_eq!((chosen.source, chosen.account), (Source::Deployment, None), "account {account:?}");
  }

  assert!(consumer.heard().is_empty(), "the hook is asked nothing for the deployment's own runs");
  consumer.down().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn with_no_hook_a_person_takes_their_own_key_of_the_road_else_the_platform_unreserved() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let platform = Source::Platform {
    name: "platform".to_owned(),
    reservation: None,
  };

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!((&chosen.source, chosen.account), (&platform, Some(PERSON)));

  personal_key(scratch.db(), "deepseek").await;
  personal_key(scratch.db(), "registered").await;

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!(chosen.source, Source::Personal);

  let chosen = going(choose(&ground, &asked(Some(PERSON), "registered")).await.unwrap());

  assert_eq!(chosen.source, platform, "nobody brings the registry's provider");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hook_s_order_is_walked_past_a_refused_reservation_to_the_first_accepted() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;
  let expires = agency_server::now_ms() + 3_600_000;

  consumer.register(&ground);
  consumer.answers("order", json!({ "sources": ["personal", "credits", "plan"] }));
  consumer.answers("reserve:credits", json!({ "accepted": false }));
  consumer.answers(
    "reserve:plan",
    json!({ "accepted": true, "reservation": "r-plan", "amountUsd": 1.25, "expiresAt": expires }),
  );

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());
  let row = row_of(&chosen);
  let kept = held(scratch.db(), row).await.unwrap().unwrap();
  let (state, _) = state_of(scratch.db(), row).await;

  assert_eq!(chosen.source.word(), "plan");
  assert_eq!(chosen.order, ["personal", "credits", "plan"]);
  assert_eq!(
    (kept.run.as_str(), kept.account, kept.source.as_str(), kept.reservation.as_str()),
    (chosen.run.as_str(), PERSON, "plan", "r-plan")
  );
  assert_eq!(
    (kept.amount_micros, kept.unit_micros, kept.expires_at, state.as_str()),
    (1_250_000, 1_250_000, Some(expires), "open")
  );

  let heard = consumer.heard();
  let order = &heard[0].ask;

  assert_eq!(consumer.ops(), ["order", "reserve", "reserve"]);
  assert_eq!(
    (&order["account"], &order["provider"], &order["kind"], &order["job"]),
    (&json!(PERSON), &json!("deepseek"), &json!("job"), &json!(41))
  );
  assert_eq!(
    consumer.keys(),
    [format!("{}:1:reserve", chosen.run), format!("{}:2:reserve", chosen.run)]
  );
  assert_eq!(heard[1].ask["source"], "credits");
  consumer.down().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn every_source_refused_closes_the_run_with_the_policy_s_sentence() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;
  let reopens = agency_server::now_ms() + 7_200_000;

  consumer.register(&ground);
  consumer.answers("order", json!({ "sources": ["credits", "plan"] }));
  consumer.answers("reserve:credits", json!({ "accepted": false }));
  consumer.answers("reserve:plan", json!({ "accepted": false, "reopensAt": reopens }));

  let Choice::Closed(closed) = choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap() else {
    panic!("the run was due to be closed");
  };
  let reopening = fill_marks(&SUPPLY.words.reopens, &[("at", &moment_words(reopens / 1000))]);

  assert_eq!((closed.source.as_str(), closed.reopens_at), ("plan", Some(reopens)));
  assert_eq!(closed.reason, fill_marks(&SUPPLY.words.refused, &[("source", "plan")]));
  assert_eq!(
    closed.sentence,
    fill_marks(
      &SUPPLY.words.closed,
      &[
        ("source", "plan"),
        ("reopens", &reopening),
        ("settings", &SUPPLY.words.settings_section)
      ]
    )
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "supply_reservations""#, vec![]).await, 0);
  consumer.down().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hook_that_fails_or_does_not_read_leaves_personal_supply_alone() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;
  let closed_for_personal = |choice: Choice| match choice {
    Choice::Closed(closed) => (closed.source, closed.reason),
    Choice::Go(chosen) => panic!("the run was due to be closed: {chosen:?}"),
  };
  let no_personal = fill_marks(&SUPPLY.words.no_personal, &[("provider", "deepseek")]);

  consumer.register(&ground);
  consumer.answers("reserve", json!({ "accepted": true, "reservation": "r", "amountUsd": 1.0 }));
  consumer.replies("order", StatusCode::INTERNAL_SERVER_ERROR, json!({}));

  assert_eq!(
    closed_for_personal(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap()),
    ("personal".to_owned(), no_personal.clone())
  );

  consumer.replies("order", StatusCode::OK, json!({ "answer": "not an answer" }));

  assert_eq!(
    closed_for_personal(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap()),
    ("personal".to_owned(), no_personal)
  );

  personal_key(scratch.db(), "deepseek").await;

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!((chosen.source, chosen.order), (Source::Personal, vec!["personal".to_owned()]));
  assert_eq!(consumer.ops(), ["order", "order", "order"], "nothing is reserved");
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "supply_reservations""#, vec![]).await, 0);
  consumer.down().await;
}
