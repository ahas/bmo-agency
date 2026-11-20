//! The source a run takes as it starts (docs/supply.md, 「Choosing a source as a run starts」, 「Owned
//! supply」): the deployment's own runs and the administrator's the platform's, unreserved and unasked; a
//! person's personal supply alone with no hook; a member or registration the person owns their personal
//! supply, its refusal marking no key of theirs and passing it over to no platform source, and another
//! person's no platform source; with a hook its order walked past a refused reservation to the first
//! accepted, the run closed with the policy's sentence where every source refuses, and personal supply alone
//! where the hook fails or answers what does not read.

use agency_server::control::claude_accounts::moment_words;
use agency_server::supply::reservations::held;
use agency_server::supply::{Choice, Chosen, Source, choose, pass_owned, refuse_personal_key};
use axum::http::StatusCode;
use policy::fill_marks;
use policy::supply::SUPPLY;
use serde_json::json;

use super::{Consumer, PERSON, asked, personal_key, row_of, state_of};
use crate::support::grounds::{deepseek_road, ground_on};
use crate::support::scratch::Scratch;

fn going(choice: Choice) -> Chosen {
  match choice {
    Choice::Go(chosen) => chosen,
    Choice::Closed(closed) => panic!("the run was due to go out: {closed:?}"),
  }
}

// The source a closed run named last, and why it could not be taken.
fn closed(choice: Choice) -> (String, String) {
  match choice {
    Choice::Closed(closed) => (closed.source, closed.reason),
    Choice::Go(chosen) => panic!("the run was due to be closed: {chosen:?}"),
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
async fn with_no_hook_a_person_takes_their_own_supply_alone() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());

  // A Claude Code run of a person with no login of their own is lent no platform account either.
  for provider in ["deepseek", "claude-code"] {
    assert_eq!(
      closed(choose(&ground, &asked(Some(PERSON), provider)).await.unwrap()),
      ("personal".to_owned(), fill_marks(&SUPPLY.words.no_personal, &[("provider", provider)])),
      "{provider}"
    );
  }

  personal_key(scratch.db(), "deepseek").await;

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!((chosen.source, chosen.order), (Source::Personal, vec!["personal".to_owned()]));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_person_s_own_member_or_registration_is_their_personal_supply_and_another_s_offers_no_platform() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;

  deepseek_road(home.path());
  registration_named(&scratch, "deepseek-flash").await;
  consumer.register(&ground);
  consumer.answers("order", json!({ "sources": ["personal", "plan"] }));
  consumer.answers("reserve:plan", json!({ "accepted": true, "reservation": "r-plan", "amountUsd": 1.0 }));

  for (owner, kind) in [(PERSON, "registered"), (8, "registered"), (PERSON, "deepseek"), (8, "deepseek")] {
    owned_by(&scratch, home.path(), owner).await;

    let choice = choose(&ground, &asked(Some(PERSON), kind)).await.unwrap();

    if owner == PERSON {
      assert_eq!(going(choice).source, Source::Owned, "{kind} owned by the person");
    } else {
      let owned = fill_marks(&SUPPLY.words.owned, &[("model", "deepseek-flash")]);

      assert_eq!(closed(choice), ("plan".to_owned(), owned), "{kind} owned by another");
    }
  }

  assert!(
    !consumer.ops().contains(&"reserve".to_owned()),
    "nothing is reserved on a person's own model"
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "supply_reservations""#, vec![]).await, 0);
  consumer.down().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_of_a_model_the_person_owns_marks_no_key_of_theirs() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let refused =
    || scratch.count(r#"SELECT COUNT(*) FROM "personal_supply" WHERE "refused_at" IS NOT NULL"#, vec![]);

  deepseek_road(home.path());
  registration_named(&scratch, "deepseek-flash").await;
  personal_key(scratch.db(), "deepseek").await;

  // The member the person owns is taken before their key, and its refusal leaves the key standing.
  owned_by(&scratch, home.path(), PERSON).await;

  let owned = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!(owned.source, Source::Owned);
  assert_eq!(refuse_personal_key(&ground, &owned).await.unwrap(), None);
  assert_eq!(refused().await, 0);

  // On another's member the run goes out on the person's key, which its refusal marks.
  owned_by(&scratch, home.path(), 8).await;

  let keyed = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!(keyed.source, Source::Personal);
  assert!(refuse_personal_key(&ground, &keyed).await.unwrap().is_some());
  assert_eq!(refused().await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_model_the_person_owns_passed_over_offers_them_no_platform_source() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let consumer = Consumer::stand().await;

  deepseek_road(home.path());
  registration_named(&scratch, "deepseek-flash").await;
  owned_by(&scratch, home.path(), PERSON).await;
  consumer.register(&ground);
  consumer.answers("order", json!({ "sources": ["personal", "plan"] }));
  consumer.answers("reserve:plan", json!({ "accepted": true, "reservation": "r-plan", "amountUsd": 1.0 }));

  let owned = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());
  let passed = pass_owned(&ground, &owned).await.unwrap();

  assert_eq!(
    closed(passed),
    ("plan".to_owned(), fill_marks(&SUPPLY.words.owned, &[("model", "deepseek-flash")]))
  );
  assert!(
    !consumer.ops().contains(&"reserve".to_owned()),
    "nothing is reserved past a person's own model"
  );
  consumer.down().await;
}

// A registration under the name given, with no address.
async fn registration_named(scratch: &Scratch, name: &str) {
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
           "thinking", "walked", "added_at") VALUES ($1, $1, $1, 32768, 4096, 0, 0, 0)"#,
      vec![name.into()],
    )
    .await;
}

// The deepseek road's member and the registration of its name both owned by the account given.
async fn owned_by(scratch: &Scratch, home: &std::path::Path, account: i64) {
  let file = home.join("data/llm-gateway/members.json");
  let mut members: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();

  members[0]["account"] = account.into();
  std::fs::write(&file, members.to_string()).unwrap();
  scratch
    .run(r#"UPDATE "registered_models" SET "account" = $1"#, vec![account.into()])
    .await;
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
  let no_personal = fill_marks(&SUPPLY.words.no_personal, &[("provider", "deepseek")]);

  consumer.register(&ground);
  consumer.answers("reserve", json!({ "accepted": true, "reservation": "r", "amountUsd": 1.0 }));
  consumer.replies("order", StatusCode::INTERNAL_SERVER_ERROR, json!({}));

  assert_eq!(
    closed(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap()),
    ("personal".to_owned(), no_personal.clone())
  );

  consumer.replies("order", StatusCode::OK, json!({ "answer": "not an answer" }));

  assert_eq!(
    closed(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap()),
    ("personal".to_owned(), no_personal)
  );

  personal_key(scratch.db(), "deepseek").await;

  let chosen = going(choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap());

  assert_eq!((chosen.source, chosen.order), (Source::Personal, vec!["personal".to_owned()]));
  assert_eq!(consumer.ops(), ["order", "order", "order"], "nothing is reserved");
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "supply_reservations""#, vec![]).await, 0);
  consumer.down().await;
}
