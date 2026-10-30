//! The cards the arbiter's master grants a run on a registration naming a card service, against a stand-in
//! master (docs/own-llm.md, 「An engine the arbiter stands」): a run waiting with no clock until its demand is
//! granted, a demand handed in stood on and left to its caller, work asked within a run's demand lending the
//! run's card and telling the run its engine stood down, the demands left behind given back at start and
//! with their lease, and a native co-worker's card asked within the demand its caller's turn holds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::cards::{
  Ask, Asking, give_back_left, give_back_on, give_back_with_leases, hold_on, holding, native_card,
};
use agency_server::control::pool::Request;
use agency_server::control::registry::{change, registration, view};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use harness_machine::card_master::{Master, MasterAddress};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::scratch::Scratch;

/// The stand-in master's memory: the demands it holds, how often each was read, the ids given back, and
/// how it grants.
#[derive(Default)]
struct StandIn {
  next: i64,
  demands: BTreeMap<i64, Value>,
  reads: BTreeMap<i64, u32>,
  given_back: Vec<i64>,
  asked: Vec<Value>,
  /// How many reads a waiting demand waits before its card is granted.
  grant_after: u32,
  /// Whether a demand asked within another lends that other's card.
  lends: bool,
  /// The address the granted card's engine answers at, and its key file.
  engine: String,
  key_file: String,
}

type Shared = Arc<Mutex<StandIn>>;

fn card(stand_in: &StandIn) -> Value {
  json!([{ "nodeName": "n1", "address": stand_in.engine, "keyFile": stand_in.key_file }])
}

async fn ask(State(shared): State<Shared>, Json(body): Json<Value>) -> impl IntoResponse {
  let mut stand_in = shared.lock().unwrap();
  stand_in.next += 1;
  let id = stand_in.next;
  let demand = json!({
    "id": id, "serviceName": body["serviceName"], "count": 1, "cards": [], "state": "waiting",
    "reason": { "code": "in-use", "text": "Every card is in use." }, "waitedMs": 0,
    "within": body.get("within"), "workKey": body.get("workKey"),
  });

  stand_in.asked.push(body);
  stand_in.demands.insert(id, demand.clone());
  (StatusCode::ACCEPTED, Json(demand))
}

async fn read(
  State(shared): State<Shared>,
  Path(id): Path<i64>,
  Query(_query): Query<BTreeMap<String, String>>,
) -> axum::response::Response {
  let mut stand_in = shared.lock().unwrap();
  let reads = {
    let reads = stand_in.reads.entry(id).or_default();
    *reads += 1;
    *reads
  };
  let grant_after = stand_in.grant_after;
  let cards = card(&stand_in);
  let lends = stand_in.lends;
  let Some(demand) = stand_in.demands.get_mut(&id) else {
    return StatusCode::NOT_FOUND.into_response();
  };

  if demand["state"] == "waiting" && reads >= grant_after {
    demand["state"] = json!("granted");
    demand["cards"] = cards;
    demand["reason"] = Value::Null;
  }

  let answered = demand.clone();

  if lends
    && answered["state"] == "granted"
    && let Some(outer) = answered["within"].as_i64()
    && let Some(outer) = stand_in.demands.get_mut(&outer)
  {
    outer["state"] = json!("lent");
  }

  Json(answered).into_response()
}

async fn give_back(State(shared): State<Shared>, Path(id): Path<i64>) -> StatusCode {
  let mut stand_in = shared.lock().unwrap();
  let Some(demand) = stand_in.demands.remove(&id) else {
    return StatusCode::NOT_FOUND;
  };

  stand_in.given_back.push(id);

  // The lent card goes back to the demand it was asked within: its engine started again, granted anew.
  if let Some(outer) = demand["within"].as_i64()
    && let Some(held) = stand_in.demands.get_mut(&outer)
    && held["state"] == "lent"
  {
    held["state"] = json!("waiting");
    held["cards"] = json!([]);
    held["reason"] = json!({ "code": "starting", "text": "The engine is starting." });
    stand_in.reads.insert(outer, 0);
  }

  StatusCode::NO_CONTENT
}

/// A stand-in master on a port of its own, its engine's health answered at the same origin.
async fn stand_in(grant_after: u32, lends: bool, key_file: &str) -> (Master, Shared) {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let shared: Shared = Arc::new(Mutex::new(StandIn {
    grant_after,
    lends,
    engine: format!("{base}/v1/chat/completions"),
    key_file: key_file.to_owned(),
    ..StandIn::default()
  }));
  let app = axum::Router::new()
    .route("/demands", axum::routing::post(ask))
    .route("/demands/{id}", get(read).delete(give_back))
    .route("/health", get(|| async { Json(json!({ "context": 65_536 })) }))
    .with_state(shared.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let address = MasterAddress::read(|name| (name == "AGENCY_ARBITER_URL").then(|| base.clone()))
    .unwrap()
    .unwrap();

  (Master::new(address), shared)
}

// A demand the stand-in already holds, granted.
fn granted(shared: &Shared, id: i64) {
  let mut stand_in = shared.lock().unwrap();
  let cards = card(&stand_in);

  stand_in.next = stand_in.next.max(id);
  stand_in
    .demands
    .insert(id, json!({ "id": id, "serviceName": "lab-arena", "cards": cards, "state": "granted" }));
}

fn asking<'a>(holder: &'a str, ask: Ask, lease: Option<&'a str>) -> Asking<'a> {
  Asking {
    registration: "flash",
    service: "lab-arena",
    holder,
    work_key: Some("session-1"),
    ask,
    lease,
  }
}

async fn ledger(db: &sea_orm::DatabaseConnection) -> Vec<i64> {
  store::all(db, r#"SELECT "demand_id" FROM "card_demands" ORDER BY "demand_id""#, vec![])
    .await
    .unwrap()
    .iter()
    .map(|row| row.try_get::<i64>("", "demand_id").unwrap())
    .collect()
}

fn key_file(dir: &tempfile::TempDir) -> String {
  let path = dir.path().join("engine.key");

  std::fs::write(&path, "engine-key\n").unwrap();
  path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn a_run_waits_with_no_clock_until_its_demand_is_granted_and_gives_it_back_at_its_end() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(4, false, &key_file(&data)).await;
  let mut said = vec![];
  let held = hold_on(scratch.db(), &master, asking("worker-a", Ask::default(), None), &mut |why: &str| {
    said.push(why.to_owned())
  })
  .await
  .unwrap();

  assert_eq!(held.node, "n1");
  assert!(held.asked && !held.lent, "{held:?}");
  assert_eq!(said, vec!["Waiting for a card of lab-arena: Every card is in use.".to_owned()]);
  assert_eq!(shared.lock().unwrap().reads[&held.demand], 4, "read until granted, however many reads");
  assert_eq!(shared.lock().unwrap().asked[0]["workKey"], "session-1");
  assert_eq!(shared.lock().unwrap().asked[0]["count"], 1);
  assert_eq!(ledger(scratch.db()).await, vec![held.demand]);
  assert_eq!(holding(scratch.db(), "worker-a").await.unwrap(), Some(held.demand));

  let told = give_back_on(scratch.db(), &master, data.path(), &held).await;

  assert_eq!(told, None, "a run that lent nothing is told nothing");
  assert_eq!(shared.lock().unwrap().given_back, vec![held.demand]);
  assert!(ledger(scratch.db()).await.is_empty());
}

#[tokio::test]
async fn a_demand_handed_in_is_stood_on_and_left_to_its_caller() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(1, false, &key_file(&data)).await;

  granted(&shared, 7);

  let own = Ask {
    own: Some(7),
    within: None,
  };
  let held = hold_on(scratch.db(), &master, asking("research", own, None), &mut |_: &str| {})
    .await
    .unwrap();

  assert_eq!((held.demand, held.node.as_str(), held.asked), (7, "n1", false));
  assert!(shared.lock().unwrap().asked.is_empty(), "no other demand is asked for the run");

  give_back_on(scratch.db(), &master, data.path(), &held).await;

  assert!(shared.lock().unwrap().given_back.is_empty(), "the caller gives its own demand back");

  let gone = Ask {
    own: Some(99),
    within: None,
  };
  let refused = hold_on(scratch.db(), &master, asking("research", gone, None), &mut |_: &str| {}).await;

  assert!(matches!(&refused, Err(why) if why.code == "demand-gone"), "{refused:?}");
}

#[tokio::test]
async fn work_asked_within_a_run_lends_its_card_and_the_run_is_told_once_its_engine_is_back() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(2, true, &key_file(&data)).await;

  granted(&shared, 50);

  let within = Ask {
    own: None,
    within: Some(50),
  };
  let held = hold_on(scratch.db(), &master, asking("worker-a", within, Some("seat-1")), &mut |_: &str| {})
    .await
    .unwrap();

  assert!(held.lent, "the run's own card was lent: {held:?}");
  assert_eq!(held.within, Some(50));
  assert_eq!(shared.lock().unwrap().asked[0]["within"], 50);
  assert_eq!(shared.lock().unwrap().demands[&50]["state"], "lent");

  let told = give_back_on(scratch.db(), &master, data.path(), &held).await;

  assert_eq!(
    told.as_deref(),
    Some("Your own model stood down while this work ran on its card, and it has come back.")
  );
  assert_eq!(
    shared.lock().unwrap().demands[&50]["state"],
    "granted",
    "the run's demand was granted again"
  );
  assert!(shared.lock().unwrap().reads[&50] >= 2, "the run waited for its own grant");
}

#[tokio::test]
async fn demands_left_behind_are_given_back_at_start_and_with_an_ended_lease() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(1, false, &key_file(&data)).await;

  for id in [11, 12] {
    granted(&shared, id);
  }

  for (id, lease) in [(11, None), (12, Some("ended-seat"))] {
    scratch
      .run(
        r#"INSERT INTO "card_demands" ("demand_id", "service", "registration", "holder", "lease", "asked_at")
           VALUES ($1, 'lab-arena', 'flash', 'worker-a', $2, 1)"#,
        vec![i64::from(id).into(), lease.map(str::to_owned).into()],
      )
      .await;
  }

  assert_eq!(give_back_with_leases(scratch.db(), &master).await.unwrap(), 1);
  assert_eq!(
    shared.lock().unwrap().given_back,
    vec![12],
    "a lease no seat holds active gives its demand back"
  );
  assert_eq!(ledger(scratch.db()).await, vec![11]);

  assert_eq!(give_back_left(scratch.db(), &master).await.unwrap(), 1);
  assert_eq!(shared.lock().unwrap().given_back, vec![12, 11]);
  assert!(ledger(scratch.db()).await.is_empty());

  // A demand the master no longer holds counts as given back.
  scratch
    .run(
      r#"INSERT INTO "card_demands" ("demand_id", "service", "registration", "holder", "asked_at")
         VALUES (404, 'lab-arena', 'flash', 'worker-a', 1)"#,
      vec![],
    )
    .await;

  assert_eq!(give_back_left(scratch.db(), &master).await.unwrap(), 1);
  assert!(ledger(scratch.db()).await.is_empty());
}

#[tokio::test]
async fn a_native_co_worker_s_card_is_asked_within_the_demand_its_caller_s_turn_holds() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(1, false, &key_file(&data)).await;

  change(
    scratch.db(),
    data.path(),
    &json!({
      "action": "save", "name": "flash", "model": "m", "contextTokens": 32_768, "answerTokens": 4_096,
      "waitSeconds": 90, "cardService": "lab-arena", "roleFiles": true,
    }),
  )
  .await
  .unwrap()
  .unwrap();
  granted(&shared, 30);
  scratch
    .run(
      r#"INSERT INTO "card_demands" ("demand_id", "service", "registration", "holder", "asked_at")
         VALUES (30, 'lab-arena', 'flash', 'worker-a', 1)"#,
      vec![],
    )
    .await;

  let seat = Request {
    sequence: 1,
    token: "seat-native".into(),
    request_key: "native:s:t".into(),
    owner: "native:s".into(),
    source: "native".into(),
    role: "scout".into(),
    work_key: "role:scout".into(),
    generation: 1,
    ended_at: None,
    outcome: None,
    reservation: None,
    provider: "registered".into(),
    model: "flash".into(),
    effort: None,
    tier: "light".into(),
    capacity: 1,
    state: "active".into(),
    enqueued_at: 1,
    leased_at: Some(1),
    child_id: None,
    yielding: false,
    place_key: None,
    turn: false,
    session: None,
  };
  let held = native_card(scratch.db(), Some(&master), &seat, Some("worker-a"))
    .await
    .unwrap()
    .unwrap();

  assert_eq!(held.within, Some(30));
  assert_eq!(shared.lock().unwrap().asked[0]["within"], 30);

  let lease: Option<String> = store::one(
    scratch.db(),
    r#"SELECT "lease" FROM "card_demands" WHERE "demand_id" = $1"#,
    vec![held.demand.into()],
  )
  .await
  .unwrap()
  .unwrap()
  .try_get("", "lease")
  .unwrap();

  assert_eq!(lease.as_deref(), Some("seat-native"), "the seat's card is kept under its lease");

  let refused = native_card(scratch.db(), None, &seat, Some("worker-a")).await;

  assert!(matches!(&refused, Err(why) if why.detail.contains("AGENCY_ARBITER_URL")), "{refused:?}");
}

#[tokio::test]
async fn naming_a_card_service_takes_the_addresses_away_and_refuses_new_ones() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let data = tempfile::tempdir().unwrap();
  let save = |more: Value| {
    let mut body = json!({
      "action": "save", "name": "flash", "model": "m", "contextTokens": 32_768, "answerTokens": 4_096,
      "waitSeconds": 90,
    });

    body.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    body
  };
  let address = json!({
    "action": "set-address", "name": "flash", "node": "alpha",
    "url": "http://127.0.0.1:18021/v1/chat/completions", "key": null,
  });

  change(db, data.path(), &save(json!({}))).await.unwrap().unwrap();
  change(db, data.path(), &address).await.unwrap().unwrap();

  let held = registration(db, "flash").await.unwrap().unwrap();

  assert!(held.card_service.is_none() && held.addresses.len() == 1 && held.offered());

  change(
    db,
    data.path(),
    &json!({ "action": "save", "name": "flash", "cardService": "lab-arena", "roleFiles": true }),
  )
  .await
  .unwrap()
  .unwrap();

  let held = registration(db, "flash").await.unwrap().unwrap();

  assert_eq!(held.card_service.as_deref(), Some("lab-arena"));
  assert!(held.role_files && held.addresses.is_empty() && held.offered());
  assert!(held.holds_node("n4"), "a session goes wherever its demand is granted");

  let refused = change(db, data.path(), &address).await.unwrap();

  assert!(matches!(&refused, Err(why) if why.contains("lab-arena")), "{refused:?}");

  let shown = view(db, data.path()).await.unwrap();

  assert_eq!(shown["registrations"][0]["cardService"], "lab-arena");
  assert_eq!(shown["registrations"][0]["roleFiles"], true);

  let bad = change(db, data.path(), &save(json!({ "cardService": "Not A Service" }))).await.unwrap();

  assert!(bad.is_err(), "{bad:?}");

  change(db, data.path(), &save(json!({ "cardService": "none" }))).await.unwrap().unwrap();

  let held = registration(db, "flash").await.unwrap().unwrap();

  assert!(
    held.card_service.is_none() && !held.offered(),
    "no address and no card service is offered nowhere"
  );
}
