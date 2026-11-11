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

// "the folders the placed programs work in are made on the node, and one that stands is left as it is"
#[test]
fn the_folders_the_placed_programs_work_in_are_made_on_the_node() {
  let node = tempfile::tempdir().unwrap();
  let standing = node.path().join("data/supervisor-homes/kept");
  let wanted = node.path().join("data/supervisor-homes/new");

  std::fs::create_dir_all(&standing).unwrap();
  std::fs::write(standing.join("notes.md"), "kept").unwrap();

  node_link::serve::make_folders(&[standing.display().to_string(), wanted.display().to_string()]).unwrap();

  assert!(wanted.is_dir());
  assert_eq!(std::fs::read_to_string(standing.join("notes.md")).unwrap(), "kept");
}

// "a run on the node asks the home bridge and ends as the run at home ended, its output and errors its own"
#[tokio::test]
async fn a_run_on_the_node_asks_the_home_bridge_and_ends_as_the_run_at_home_ended() {
  use std::os::unix::fs::PermissionsExt;

  let dir = tempfile::tempdir().unwrap();
  let deploy = dir.path().join("deploy");
  let verbs = deploy.join("bin/bmo");

  std::fs::create_dir_all(verbs.parent().unwrap()).unwrap();
  std::fs::write(&verbs, "#!/bin/sh\necho \"$@\"\ncat\necho said >&2\nexit 5\n").unwrap();
  std::fs::set_permissions(&verbs, std::fs::Permissions::from_mode(0o755)).unwrap();

  let socket = dir.path().join("bridge.sock");
  let listener = tokio::net::UnixListener::bind(&socket).unwrap();

  tokio::spawn(async move {
    let (stream, _) = listener.accept().await.unwrap();

    node_link::bridge::answer(stream, deploy, "pc".into(), |path| path.replace("/n/data", "/h/data"))
      .await
      .unwrap();
  });

  let (mut out, mut err) = (Vec::new(), Vec::new());
  let args = ["agent-status", "status", "--place", "/n/data/agent-places/w"].map(str::to_owned);
  let code = node_link::bridge::ask(&socket, "bin/bmo", &args, &b"payload"[..], &mut out, &mut err).await;

  assert_eq!(code, 5);
  assert_eq!(
    String::from_utf8_lossy(&out),
    "agent-status status --place /h/data/agent-places/w\npayload"
  );
  assert_eq!(String::from_utf8_lossy(&err), "said\n");
}
