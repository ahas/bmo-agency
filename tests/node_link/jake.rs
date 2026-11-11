//! The near end of a link to a person's PC (docs/person-nodes.md, 「The link over jake」): the hub asked only for the
//! listens that do not stand, and the bridge's stream back held open until its asker closes it.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Query, State};
use axum::routing::get;
use node_link::connect::{RELAY_LINGER, ask_listens, relay_stream_back};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Calls = Arc<Mutex<Vec<String>>>;

async fn listens() -> axum::Json<Value> {
  axum::Json(json!({ "agent": "pc", "listens": [
    { "port": 1, "target": "127.0.0.1:11", "listening": true },
    { "port": 2, "target": "127.0.0.1:12", "listening": false },
  ] }))
}

async fn listen(
  State(calls): State<Calls>,
  method: axum::http::Method,
  Query(query): Query<Vec<(String, String)>>,
) -> axum::Json<Value> {
  let port = query
    .iter()
    .find(|(key, _)| key == "port")
    .map(|(_, port)| port.clone())
    .unwrap_or_default();

  calls.lock().unwrap().push(format!("{method} {port}"));
  axum::Json(json!({ "ok": true }))
}

// "the hub is asked only for the listens that do not stand, and one that stopped listening is asked afresh"
#[tokio::test]
async fn the_hub_is_asked_only_for_the_listens_that_do_not_stand_and_one_that_stopped_is_asked_afresh() {
  let calls: Calls = Arc::default();
  let app = Router::new()
    .route("/listens", get(listens))
    .route("/listen", axum::routing::post(listen).delete(listen))
    .with_state(calls.clone());
  let hub = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = hub.local_addr().unwrap().port();
  let settings = tempfile::NamedTempFile::new().unwrap();

  tokio::spawn(async move { axum::serve(hub, app).await });
  std::fs::write(settings.path(), format!("JAKE_CONTROL_PORT={port}\nJAKE_CONTROL_TOKEN=t\n")).unwrap();

  ask_listens("pc", settings.path(), &[(1, 11), (2, 12), (3, 13)]).await.unwrap();

  assert_eq!(*calls.lock().unwrap(), ["DELETE 2", "POST 2", "POST 3"]);
}

// "the bridge's answer goes back whole, and the stream back stays open until its asker closes it"
#[tokio::test]
async fn the_bridge_s_answer_goes_back_whole_and_the_stream_back_stays_open_until_its_asker_closes_it() {
  let relay = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let mut asker = tokio::net::TcpStream::connect(relay.local_addr().unwrap()).await.unwrap();
  let (client, _) = relay.accept().await.unwrap();
  let (far, mut bridge) = tokio::net::UnixStream::pair().unwrap();
  let relaying = tokio::spawn(relay_stream_back(client, far, RELAY_LINGER));

  asker.write_all(b"ask").await.unwrap();

  let mut asked = [0; 3];

  bridge.read_exact(&mut asked).await.unwrap();
  assert_eq!(&asked, b"ask");
  bridge.write_all(b"answer").await.unwrap();
  drop(bridge);

  let mut answer = [0; 6];

  asker.read_exact(&mut answer).await.unwrap();
  assert_eq!(&answer, b"answer");
  assert!(!relaying.is_finished(), "the stream back waits for its asker");

  asker.shutdown().await.unwrap();
  relaying.await.unwrap();

  let mut rest = Vec::new();

  assert_eq!(
    asker.read_to_end(&mut rest).await.unwrap(),
    0,
    "the stream back ends once its asker closed"
  );
}
