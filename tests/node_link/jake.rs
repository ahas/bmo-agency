//! The near end of a link to a person's PC (docs/person-nodes.md, 「The link over jake」): every listen it wants asked
//! of the hub, and the folders and the bridge the far end serves.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{Query, State};
use node_link::connect::ask_listens;
use serde_json::Value;

type Calls = Arc<Mutex<Vec<String>>>;

async fn listen(
  State(calls): State<Calls>,
  method: axum::http::Method,
  headers: axum::http::HeaderMap,
  Query(query): Query<Vec<(String, String)>>,
  axum::Json(body): axum::Json<Value>,
) -> axum::Json<Value> {
  let port = query
    .iter()
    .find(|(key, _)| key == "port")
    .map(|(_, port)| port.clone())
    .unwrap_or_default();
  let target = body.get("target").and_then(Value::as_str).unwrap_or_default();

  // The scope a credential names stands ahead of its mac, which the hub's door test checks.
  let scope = headers
    .get("authorization")
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.strip_prefix("Bearer "))
    .and_then(|credential| credential.rsplit_once('.'))
    .map_or("", |(scope, _)| scope);

  calls.lock().unwrap().push(format!("{method} {port} {target} as {scope}"));
  axum::Json(serde_json::json!({ "ok": true, "standing": false }))
}

// "every listen wanted is asked of the hub with its target, with a credential scoped to the agent alone"
#[tokio::test]
async fn every_listen_wanted_is_asked_of_the_hub_with_its_target() {
  let calls: Calls = Arc::default();
  let app = Router::new().route("/listen", axum::routing::post(listen)).with_state(calls.clone());
  let hub = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = hub.local_addr().unwrap().port();
  let settings = tempfile::NamedTempFile::new().unwrap();

  tokio::spawn(async move { axum::serve(hub, app).await });
  std::fs::write(
    settings.path(),
    format!("JAKE_CONTROL_PORT={port}\nJAKE_CONTROL_TOKEN=t\nJAKE_CONTROL_KEY=k\n"),
  )
  .unwrap();

  ask_listens("pc", settings.path(), &[(1, 11), (2, 12)]).await.unwrap();

  assert_eq!(*calls.lock().unwrap(), ["POST 1 127.0.0.1:11 as pc", "POST 2 127.0.0.1:12 as pc"]);
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

    node_link::bridge::answer(stream, deploy, "pc".into(), None, |path| path.replace("/n/data", "/h/data"))
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
