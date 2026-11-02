//! The cards the arbiter's master grants a run on a registration naming a card service, against a stand-in
//! master (docs/own-llm.md, 「An engine the arbiter stands」): a run waiting with no clock until its demand is
//! granted, a demand handed in stood on and left to its caller, work asked within a run's demand lending the
//! run's card and telling the run its engine stood down, the demands left behind given back at start and
//! with their lease, a native co-worker's card asked within the demand its caller's turn holds, a seat's
//! admission reading its demand as it stands, a seat standing on its caller's own demand with none asked,
//! and one card shared by the runs out at once on a registration, kept for the runs still out when one is
//! let go and given back, granted or still waiting, once none is.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agency_server::cards::{
  Ask, Asking, Gateway, SharedAsk, Sharing, give_back_lease, give_back_left, give_back_on,
  give_back_with_leases, hold_on, holding, native_card, seat_card_on, share,
};
use agency_server::control::registry::{change, registration, view};
use agency_server::order_run::dispatch::Request;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use harness_machine::card_master::{Master, MasterAddress};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::scratch::Scratch;

/// The stand-in master's memory: the demands it holds, how often each was read, the ids given back, and
/// how it grants.
#[derive(Default)]
struct StandIn {
  next: i64,
  demands: BTreeMap<i64, Value>,
  reads: BTreeMap<i64, u32>,
  /// How long each read asked the answer held open, in milliseconds, in the order the reads came.
  waits: Vec<u64>,
  given_back: Vec<i64>,
  asked: Vec<Value>,
  /// How many reads a waiting demand waits before its card is granted.
  grant_after: u32,
  /// Whether a demand asked within another lends that other's card.
  lends: bool,
  /// The address the granted card's engine answers at, and its key file.
  engine: String,
  key_file: String,
  /// The origin the stand-in answers at, its gateway's among them.
  base: String,
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
  Query(query): Query<BTreeMap<String, String>>,
) -> axum::response::Response {
  let mut stand_in = shared.lock().unwrap();
  let wait = query.get("waitMs").and_then(|wait| wait.parse::<u64>().ok()).unwrap_or_default();

  stand_in.waits.push(wait);

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
    base: base.clone(),
    ..StandIn::default()
  }));
  let app = axum::Router::new()
    .route("/demands", axum::routing::post(ask))
    .route("/demands/{id}", get(read).delete(give_back))
    .route("/health", get(|| async { Json(json!({ "context": 65_536 })) }))
    // The gateway's table: the granted card's entry served.
    .route("/v1/models", get(|| async { Json(json!({ "data": [{ "id": "flash@n1", "owned_by": "gateway" }] })) }))
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

// A seat the pool let in on the registration `flash`, for run 7.
fn seat(token: &str) -> Request {
  Request {
    sequence: 1,
    token: token.into(),
    request_key: "research:7".into(),
    owner: "research:7".into(),
    source: "search".into(),
    role: "scout".into(),
    path: None,
    ended_at: None,
    outcome: None,
    reservation: None,
    provider: "registered".into(),
    model: "flash".into(),
    effort: None,
    capacity: 1,
    state: "active".into(),
    enqueued_at: 1,
    leased_at: Some(1),
    child_id: None,
    yielding: false,
    place_key: None,
    session: Some("session-7".into()),
    title: None,
    asked_by: None,
    agent_name: None,
    held_for: None,
    ended_by: None,
  }
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
      "cardService": "lab-arena", "roleFiles": true,
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
    path: None,
    ended_at: None,
    outcome: None,
    reservation: None,
    provider: "registered".into(),
    model: "flash".into(),
    effort: None,
    capacity: 1,
    state: "active".into(),
    enqueued_at: 1,
    leased_at: Some(1),
    child_id: None,
    yielding: false,
    place_key: None,
    session: None,
    title: None,
    asked_by: None,
    agent_name: None,
    held_for: None,
    ended_by: None,
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

// The registration `flash` naming the card service `lab-arena`, and what a card its runs share is asked
// with: the stand-in as the master and as the gateway, its key file readable as the gateway's is.
async fn shared_ask(
  db: &sea_orm::DatabaseConnection,
  data: &tempfile::TempDir,
  master: &Master,
  shared: &Shared,
) -> SharedAsk {
  change(
    db,
    data.path(),
    &json!({
      "action": "save", "name": "flash", "model": "m", "contextTokens": 32_768, "answerTokens": 4_096,
      "cardService": "lab-arena", "roleFiles": true,
    }),
  )
  .await
  .unwrap()
  .unwrap();

  let key_file = data.path().join("gateway.key");

  std::fs::write(&key_file, "gateway-key\n").unwrap();
  std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600)).unwrap();

  SharedAsk {
    db: db.clone(),
    master: master.clone(),
    registration: "flash".into(),
    service: "lab-arena".into(),
    holder: "translator".into(),
    gateway: Gateway {
      url: shared.lock().unwrap().base.clone(),
      key_file,
    },
  }
}

// Read until `done` holds of the stand-in, the work under test going on its own meanwhile.
async fn until(shared: &Shared, done: impl Fn(&StandIn) -> bool) {
  for _ in 0..500 {
    if done(&shared.lock().unwrap()) {
      return;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
  }

  panic!("the stand-in never came to the state looked for");
}

#[tokio::test]
async fn the_runs_out_at_once_on_a_registration_share_one_demand_given_back_by_the_last() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(3, false, &key_file(&data)).await;
  let sharing = Arc::new(Sharing::default());
  let db = scratch.db();
  let ask = shared_ask(db, &data, &master, &shared).await;
  let parts = (0..4).map(|_| share(&sharing, ask.clone()));
  let mut sharers: Vec<_> =
    futures_util::future::join_all(parts).await.into_iter().map(Result::unwrap).collect();
  let demand = sharers[0].held.demand;

  assert_eq!(shared.lock().unwrap().asked.len(), 1, "four parts out at once ask one demand");
  assert!(sharers.iter().all(|sharer| sharer.held.demand == demand && sharer.held.node == "n1"));

  let last = sharers.pop().unwrap();
  for sharer in sharers {
    sharer.leave().await;
  }

  assert!(shared.lock().unwrap().given_back.is_empty(), "kept while a part is still out");
  assert_eq!(ledger(db).await, vec![demand]);

  last.leave().await;

  assert_eq!(shared.lock().unwrap().given_back, vec![demand], "given back once, by the last");
  assert!(ledger(db).await.is_empty());

  let again = share(&sharing, ask.clone()).await.unwrap();

  assert_eq!(shared.lock().unwrap().asked.len(), 2, "a part out after the last left asks anew");
  again.leave().await;
  assert_eq!(shared.lock().unwrap().given_back.len(), 2);
}

#[tokio::test]
async fn a_part_let_go_while_the_shared_demand_waits_leaves_it_to_the_rest_and_the_last_gives_it_back() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(u32::MAX, false, &key_file(&data)).await;
  let sharing = Arc::new(Sharing::default());
  let db = scratch.db();
  let ask = shared_ask(db, &data, &master, &shared).await;
  let out = |sharing: &Arc<Sharing>| {
    let (sharing, ask) = (sharing.clone(), ask.clone());

    tokio::spawn(async move { share(&sharing, ask).await.map(|sharer| sharer.held.demand) })
  };
  let first = out(&sharing);

  until(&shared, |stand_in| stand_in.asked.len() == 1).await;

  let second = out(&sharing);
  let third = out(&sharing);
  let demand = *shared.lock().unwrap().demands.keys().next().unwrap();

  // The part that asked the demand is let go by its caller: the others keep waiting on the same demand.
  let read = shared.lock().unwrap().reads.get(&demand).copied().unwrap_or_default();

  first.abort();
  until(&shared, |stand_in| stand_in.reads.get(&demand).is_some_and(|reads| *reads > read + 20)).await;

  assert_eq!(shared.lock().unwrap().asked.len(), 1, "no part asks a demand of its own");
  assert!(shared.lock().unwrap().given_back.is_empty(), "kept while a part still waits on it");
  assert_eq!(ledger(db).await, vec![demand]);

  second.abort();
  third.abort();
  until(&shared, |stand_in| !stand_in.given_back.is_empty()).await;

  assert_eq!(
    shared.lock().unwrap().given_back,
    vec![demand],
    "given back by the last let go, still waiting"
  );
  assert_eq!(shared.lock().unwrap().asked.len(), 1);
  assert!(ledger(db).await.is_empty());

  // A part out after that asks anew, and is granted as the master grants.
  shared.lock().unwrap().grant_after = 1;

  let again = share(&sharing, ask.clone()).await.unwrap();

  assert_eq!(shared.lock().unwrap().asked.len(), 2);

  // The last let go while the card stands gives it back as it goes.
  drop(again);
  until(&shared, |stand_in| stand_in.given_back.len() == 2).await;
  assert!(ledger(db).await.is_empty());
}

#[tokio::test]
async fn a_seat_s_admission_reads_its_demand_as_it_stands_and_answers_at_once() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(2, false, &key_file(&data)).await;

  change(
    scratch.db(),
    data.path(),
    &json!({
      "action": "save", "name": "flash", "model": "m", "contextTokens": 32_768, "answerTokens": 4_096,
      "cardService": "lab-arena", "roleFiles": true,
    }),
  )
  .await
  .unwrap()
  .unwrap();

  let seat = seat("seat-polled");
  let within = Ask {
    own: None,
    within: Some(30),
  };
  let admission = || seat_card_on(scratch.db(), Some(&master), &seat, within);

  // The first admission asks the demand, within the run's; the stand-in grants it on its second read.
  let asked = admission().await.unwrap().unwrap();

  assert!(!asked.granted() && asked.state == "waiting", "{asked:?}");
  assert_eq!(shared.lock().unwrap().asked[0]["within"], 30);
  assert_eq!(ledger(scratch.db()).await, vec![asked.id], "the demand is kept under the seat's lease");

  let waiting = admission().await.unwrap().unwrap();

  assert_eq!(waiting.id, asked.id);
  assert!(!waiting.granted() && waiting.reason.is_some(), "{waiting:?}");

  let granted = admission().await.unwrap().unwrap();

  assert_eq!(granted.id, asked.id);
  assert_eq!(granted.node_name.as_deref(), Some("n1"));
  assert!(granted.granted() && granted.reason.is_none(), "{granted:?}");
  assert_eq!(
    shared.lock().unwrap().waits,
    vec![0, 0],
    "an admission reads the demand as it stands, holding nothing open at the master"
  );
}

#[tokio::test]
async fn a_seat_standing_on_its_caller_s_own_demand_asks_none_and_leaves_it_to_its_caller() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (master, shared) = stand_in(1, true, &key_file(&data)).await;

  change(
    scratch.db(),
    data.path(),
    &json!({
      "action": "save", "name": "flash", "model": "m", "contextTokens": 32_768, "answerTokens": 4_096,
      "cardService": "lab-arena", "roleFiles": true,
    }),
  )
  .await
  .unwrap()
  .unwrap();
  granted(&shared, 7);

  let seat = seat("seat-standing");
  let own = Ask {
    own: Some(7),
    within: None,
  };
  let card = seat_card_on(scratch.db(), Some(&master), &seat, own).await.unwrap().unwrap();

  assert_eq!((card.id, card.node_name.as_deref()), (7, Some("n1")));
  assert!(card.granted(), "{card:?}");
  assert!(shared.lock().unwrap().asked.is_empty(), "no demand of the seat's own is asked");
  assert!(ledger(scratch.db()).await.is_empty(), "nothing is kept under the seat's lease");
  assert_eq!(shared.lock().unwrap().waits, vec![0], "the demand is read as it stands");

  let told = give_back_lease(scratch.db(), &master, data.path(), &seat.token).await.unwrap();

  assert_eq!(told, None, "the caller's engine never stood down");
  assert!(shared.lock().unwrap().given_back.is_empty(), "the caller gives its own demand back");
  assert_eq!(shared.lock().unwrap().demands[&7]["state"], "granted");

  let gone = Ask {
    own: Some(99),
    within: None,
  };
  let refused = seat_card_on(scratch.db(), Some(&master), &seat, gone).await;

  assert!(matches!(&refused, Err(why) if why.code == "demand-gone"), "{refused:?}");
}
