//! The sources a person's chain leaves open and organisation supply (docs/supply.md, 「The sources a person's chain
//! leaves open」 and 「Organisation supply」): the order's fixed shape, organisation supply opened by the platform's
//! word alone, and a run taking the nearest unit's key of its path through a reservation that names the unit.
//!
//! The account service stands in on the loopback: the platform opens organisation supply to the company, unit 3,
//! by a rule naming it; the person stands in unit 4 under it, which holds a key of its own; and their own supply
//! is turned off at unit 3.

use std::collections::{BTreeMap, HashMap};

use agency_server::supply::sources::{self, Open};
use agency_server::supply::{Choice, Source, choose, personal_member};
use axum::extract::{Path as Route, Query};
use serde_json::{Value, json};

use super::{Consumer, PERSON, asked, personal_key};
use crate::support::grounds::ground_with;
use crate::support::scratch::Scratch;

fn order(words: &[&str]) -> Vec<String> {
  words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn the_order_keeps_personal_then_organisation_then_the_platform_s_and_drops_what_the_chain_closes() {
  let hooked = order(&["personal", "plan", "credits"]);
  let open = |personal, platform, units: &[i64]| Open {
    personal,
    platform,
    units: units.to_vec(),
  };

  assert_eq!(
    sources::shaped(&hooked, &Open::default()),
    hooked,
    "the defaults leave the hook's order"
  );
  assert_eq!(
    sources::shaped(&hooked, &open(true, true, &[4])),
    order(&["personal", "organisation", "plan", "credits"])
  );
  assert_eq!(
    sources::shaped(&hooked, &open(false, true, &[4])),
    order(&["organisation", "plan", "credits"])
  );
  assert_eq!(sources::shaped(&hooked, &open(true, false, &[4])), order(&["personal", "organisation"]));
  assert_eq!(sources::shaped(&hooked, &open(false, false, &[])), Vec::<String>::new());
  assert_eq!(
    sources::shaped(&order(&["organisation", "plan"]), &Open::default()),
    order(&["plan"]),
    "a hook that names the organisation's supply is not followed"
  );
}

#[test]
fn organisation_supply_opens_on_the_platform_s_word_alone() {
  let answered = |layers: Value, value: bool| json!({ "value": value, "layers": layers });

  assert!(sources::organisation_open(Some(&answered(
    json!([{ "link": "platform", "value": true, "rule": "unit:3" }]),
    true
  ))));
  assert!(
    !sources::organisation_open(Some(&answered(json!([{ "link": "unit:3", "value": true }]), true))),
    "a unit's word alone never opens it"
  );
  assert!(!sources::organisation_open(Some(&answered(
    json!([{ "link": "platform", "value": true }, { "link": "unit:4", "value": false }]),
    false
  ))));
  assert!(!sources::organisation_open(None));
  assert_eq!(
    sources::path_units(Some(&json!({ "path": ["platform", "unit:3", "unit:4", "person:7"] }))),
    vec![4, 3],
    "the nearest unit first"
  );
}

// The chain as the stand-in answers it for the person: what each of the three settings resolves to.
fn resolved(key: &str) -> Value {
  let path = json!(["platform", "unit:3", "unit:4", format!("person:{PERSON}")]);

  match key {
    "supply.organisation" => json!({
      "value": true,
      "layers": [{ "link": "platform", "value": true, "rule": "unit:3" }],
      "path": path,
    }),
    "supply.personal" => {
      json!({ "value": false, "layers": [{ "link": "unit:3", "value": false }], "path": path })
    }
    _ => json!({ "value": true, "layers": [], "path": path }),
  }
}

async fn account_service() -> (String, tokio::task::JoinHandle<()>) {
  let app = axum::Router::new()
    .route(
      "/chain/kinds/{key}",
      axum::routing::put(|axum::Json(body): axum::Json<Value>| async move { axum::Json(body) }),
    )
    .route(
      "/chain/kinds/{key}/resolve",
      axum::routing::get(
        |Route(key): Route<String>, Query(asked): Query<HashMap<String, String>>| async move {
          assert_eq!(asked.get("person"), Some(&PERSON.to_string()));

          axum::Json(resolved(&key))
        },
      ),
    );
  let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let serving = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (address, serving)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_person_s_run_takes_the_nearest_unit_s_key_through_a_reservation_naming_the_unit() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, serving) = account_service().await;
  let vars =
    BTreeMap::from([("WEB_ADDRESS".to_owned(), address), ("WEB_TOKEN".to_owned(), "chain".to_owned())]);
  let ground = ground_with(scratch.db().clone(), home.path(), &vars);
  let consumer = Consumer::stand().await;

  personal_key(scratch.db(), "deepseek").await;
  scratch
    .run(
      r#"INSERT INTO "personal_supply" ("unit_id", "road", "secret", "place", "added_at")
         VALUES (4, 'deepseek', 'team-key', 1, 0)"#,
      vec![],
    )
    .await;
  consumer.register(&ground);
  consumer.answers("order", json!({ "sources": ["personal", "plan"] }));
  consumer.answers(
    "reserve:organisation",
    json!({ "accepted": true, "reservation": "r-org", "amountUsd": 1.0 }),
  );

  let chosen = match choose(&ground, &asked(Some(PERSON), "deepseek")).await.unwrap() {
    Choice::Go(chosen) => chosen,
    Choice::Closed(closed) => panic!("the run was due to go out: {closed:?}"),
  };
  let Source::Organisation { unit, reservation } = chosen.source else {
    panic!("the organisation's supply was due: {:?}", chosen.source);
  };

  assert_eq!(unit, 4);
  assert_eq!(chosen.order, order(&["organisation", "plan"]), "the person's own is closed at unit 3");
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "supply_reservations" WHERE "id" = $1 AND "unit_id" = 4"#,
        vec![reservation.into()]
      )
      .await,
    1
  );
  assert_eq!(personal_member(Some(&chosen), "deepseek-flash").as_deref(), Some("deepseek-flash@o4"));

  consumer.down().await;
  serving.abort();
}
