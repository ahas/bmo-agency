//! A person's PC reached through its hub for the workplace's setup (docs/person-nodes.md, 「The workplace on a
//! PC」), against a hub standing in on the loopback: its commands run here in a scratch home, where the user
//! manager's `systemd-run` and `systemctl` are stand-ins that run a step at once.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agency_server::peers::node::HubReach;
use agency_server::verbs::work_node_hub::{HubDoor, HubHands};
use agency_server::verbs::work_node_setup::{FarEnd, SetupHands, SetupShell};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const TOKEN: &str = "stand-in-token";
const AGENT: &str = "pc";

// The user manager stood in for: a transient unit's command run at once to its end, and no unit active.
const SYSTEMD_RUN: &str = "#!/bin/sh\nwhile [ \"$1\" != /bin/bash ]; do shift; done\nexec \"$@\"\n";
const SYSTEMCTL: &str = "#!/bin/sh\nexit 3\n";

#[derive(Clone)]
struct Hub {
  home: PathBuf,
  shims: PathBuf,
  commands: Arc<Mutex<Vec<String>>>,
}

fn authorized(headers: &HeaderMap) -> bool {
  headers.get("authorization").and_then(|value| value.to_str().ok()) == Some(&format!("Bearer {TOKEN}"))
}

fn events(events: &[(&str, Value)]) -> Response {
  let body: String =
    events.iter().map(|(event, data)| format!("event: {event}\ndata: {data}\n\n")).collect();

  ([("content-type", "text/event-stream")], body).into_response()
}

async fn status(headers: HeaderMap) -> Response {
  if !authorized(&headers) {
    return StatusCode::UNAUTHORIZED.into_response();
  }

  axum::Json(json!({ "agents": [{
    "name": AGENT,
    "capabilities": ["exec:linux", "transfer", "stream:47201", "listen:13123"],
  }] }))
  .into_response()
}

// A command run here as the agent runs it on the PC, its output and its end told as the hub tells them.
async fn exec(State(hub): State<Hub>, headers: HeaderMap, axum::Json(body): axum::Json<Value>) -> Response {
  if !authorized(&headers) || body["shell"] != "linux" {
    return StatusCode::BAD_REQUEST.into_response();
  }

  let command = body["command"].as_str().unwrap_or_default().to_owned();

  hub.commands.lock().unwrap().push(command.clone());

  let path = format!("{}:{}", hub.shims.display(), std::env::var("PATH").unwrap_or_default());
  let done = tokio::process::Command::new("bash")
    .arg("-c")
    .arg(&command)
    .env("HOME", &hub.home)
    .env("PATH", path)
    .env_remove("XDG_CACHE_HOME")
    .stdin(std::process::Stdio::null())
    .kill_on_drop(true)
    .output()
    .await
    .unwrap();
  let chunk =
    |bytes: &[u8]| json!({ "seq": 0, "chunk": { "bytes": bytes.len(), "base64": STANDARD.encode(bytes) } });

  events(&[
    ("started", json!({ "id": "c1" })),
    ("stdout", chunk(&done.stdout)),
    ("stderr", chunk(&done.stderr)),
    ("exit", json!({ "code": done.status.code(), "signal": null, "canceled": false })),
  ])
}

// An upload written whole where it is named, once its length and checksum match what was declared.
async fn upload(headers: HeaderMap, Query(query): Query<Vec<(String, String)>>, body: Bytes) -> Response {
  let asked = |name: &str| query.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());

  if !authorized(&headers) || asked("agent").as_deref() != Some(AGENT) {
    return StatusCode::BAD_REQUEST.into_response();
  }

  if asked("total") != Some(body.len().to_string())
    || asked("checksum") != Some(format!("{:x}", Sha256::digest(&body)))
  {
    return events(&[(
      "error",
      json!({ "code": "integrity", "message": "the declared upload did not match" }),
    )]);
  }

  std::fs::write(asked("path").unwrap(), &body).unwrap();
  events(&[("completed", json!({ "total": body.len() }))])
}

struct Stood {
  reach: HubReach,
  home: PathBuf,
  commands: Arc<Mutex<Vec<String>>>,
  _scratch: tempfile::TempDir,
}

async fn stand_hub() -> Stood {
  let scratch = tempfile::tempdir().unwrap();
  let home = scratch.path().join("home");
  let shims = scratch.path().join("shims");

  std::fs::create_dir_all(&home).unwrap();
  std::fs::create_dir_all(&shims).unwrap();

  for (name, body) in [("systemd-run", SYSTEMD_RUN), ("systemctl", SYSTEMCTL)] {
    let file = shims.join(name);

    std::fs::write(&file, body).unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
  }

  let token_file = scratch.path().join("hub.env");

  std::fs::write(
    &token_file,
    format!("JAKE_HUB_PORT=8790\nJAKE_CONTROL_TOKEN={TOKEN}\nJAKE_REVERSE_TARGETS=127.0.0.1:13123,127.0.0.1:47213\n"),
  )
  .unwrap();

  let commands = Arc::new(Mutex::new(Vec::new()));
  let hub = Hub {
    home: home.clone(),
    shims,
    commands: commands.clone(),
  };
  let app = Router::new()
    .route("/status", get(status))
    .route("/exec", post(exec))
    .route("/upload", post(upload))
    .with_state(hub);
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await });

  Stood {
    reach: HubReach {
      address,
      token_file: token_file.to_string_lossy().into_owned(),
      agent: AGENT.into(),
    },
    home,
    commands,
    _scratch: scratch,
  }
}

async fn door(stood: &Stood) -> HubDoor {
  HubDoor::open(&stood.reach, Path::new("/")).await.unwrap()
}

// "a step runs as a transient unit of the person's own and comes back with its output and its code"
#[tokio::test]
async fn a_step_runs_as_a_transient_unit_and_comes_back_with_its_output_and_its_code() {
  let stood = stand_hub().await;
  let door = door(&stood).await;
  let ran = door.run(r#"echo "out $HOME"; echo err >&2; exit 3"#).await;

  assert_eq!(
    (ran.code, ran.stdout.as_str(), ran.stderr.as_str()),
    (3, format!("out {}\n", stood.home.display()).as_str(), "err\n")
  );
  assert!(
    stood.commands.lock().unwrap()[0]
      .contains("systemd-run --user --collect --quiet --unit=bmo-node-step-")
  );
  assert!(stood.commands.lock().unwrap()[0].contains("-p KillMode=process"));
  // The step's files are gone once it is read back.
  assert_eq!(std::fs::read_dir(stood.home.join(".cache/bmo-node-setup")).unwrap().count(), 0);
}

// "an agent the hub does not have attached is refused, and what an attached one does not offer is said"
#[tokio::test]
async fn an_absent_agent_is_refused_and_what_an_attached_one_does_not_offer_is_said() {
  let stood = stand_hub().await;
  let absent = HubReach {
    agent: "elsewhere".into(),
    ..stood.reach.clone()
  };
  let refused = HubDoor::open(&absent, Path::new("/")).await.err().unwrap();
  let ports = FarEnd {
    mirror_port: 47201,
    herdr_port: 47202,
    bridge_port: 47203,
  };

  assert!(refused.contains("no agent elsewhere attached"));
  assert_eq!(
    door(&stood).await.wanting(&ports, 13123, 47213),
    [
      "jake: the agent pc does not offer port 47202 for herdr to stream to; its JAKE_STREAM_PORTS must list it",
      "jake: the agent pc does not offer to listen on port 47203 for the bridge; its JAKE_LISTEN_PORTS must list it",
    ]
  );
  // A relay port the hub's settings do not list is said with them.
  assert_eq!(
    door(&stood).await.wanting(&ports, 13123, 47214)[2],
    "jake: the hub joins no stream back to 127.0.0.1:47214, the bridge's relay here; its JAKE_REVERSE_TARGETS must list it"
  );
}

// "a tree goes up through one upload and is laid over the node's copy, leaving what was built there"
#[tokio::test]
async fn a_tree_goes_up_through_one_upload_and_is_laid_over_the_node_s_copy_leaving_what_was_built_there() {
  let stood = stand_hub().await;
  let door = door(&stood).await;
  let here = tempfile::tempdir().unwrap();
  let from = here.path().join("deploy");
  let to = stood.home.join("w/deploy");

  for (file, body) in [("docs/a.md", "a"), ("node_modules/x/i.js", "x"), (".env", "secret")] {
    std::fs::create_dir_all(from.join(file).parent().unwrap()).unwrap();
    std::fs::write(from.join(file), body).unwrap();
  }

  for (file, body) in [("bin/agent-host", "built"), ("stale.md", "old")] {
    std::fs::create_dir_all(to.join(file).parent().unwrap()).unwrap();
    std::fs::write(to.join(file), body).unwrap();
  }

  let hands = HubHands {
    install: false,
    copy_code: true,
    door: &door,
    staging: here.path().join("staging"),
  };
  let (copied, said) = hands.copy(&format!("{}/", from.display()), &format!("{}/", to.display())).await;

  assert!(copied, "{said}");
  assert_eq!(std::fs::read_to_string(to.join("docs/a.md")).unwrap(), "a");
  assert_eq!(std::fs::read_to_string(to.join("bin/agent-host")).unwrap(), "built");
  assert!(!to.join("stale.md").exists() && !to.join("node_modules").exists() && !to.join(".env").exists());
  assert!(
    !stood.home.join("w/deploy.incoming").exists() && !stood.home.join("w/deploy.incoming.tar.gz").exists()
  );
}
