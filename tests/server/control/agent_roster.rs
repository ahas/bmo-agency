//! Every agent registered and its registration ended through the agents routes, asked of a running server
//! beside a brain of the test's own: the key's rule, the kinds and roles, the owner each row carries, the
//! brain made for an agent that converses and filed under its owner, and the refusals of an ending — none
//! such, seeded, running — with the brain kept unless the caller lets it go.

use std::sync::{Arc, Mutex};

use agency_server::places::locks::{agent_lock_path, try_hold};
use axum::Json;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::scratch::Scratch;
use crate::support::standing;

/// What the brain was asked, in order, and the brains it keeps by agent key.
#[derive(Default)]
struct Brains {
  asked: Vec<(String, String, Value)>,
  kept: Vec<(String, i64)>,
}

type Kept = Arc<Mutex<Brains>>;

async fn brain(State(brains): State<Kept>, method: Method, uri: Uri, body: axum::body::Bytes) -> Response {
  let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
  let path = uri.path().trim_start_matches('/').to_owned();
  let query = uri.query().unwrap_or_default().to_owned();
  let mut brains = brains.lock().unwrap();

  brains.asked.push((method.to_string(), format!("{path}?{query}"), body.clone()));

  let member = |agent: &str, id: i64| json!({ "id": id, "accountKey": "1", "agentKey": agent, "name": agent, "createdAt": 1 });

  match (method.as_str(), path.as_str()) {
    ("GET", "brains") => {
      let agent = query.split('&').find_map(|pair| pair.strip_prefix("agentKey=")).unwrap_or_default();
      let items: Vec<Value> = brains
        .kept
        .iter()
        .filter(|(key, _)| key == agent)
        .map(|(key, id)| member(key, *id))
        .collect();

      Json(json!({ "items": items, "more": false })).into_response()
    }
    ("POST", "brains") => {
      let agent = body["agentKey"].as_str().unwrap_or_default().to_owned();
      let id = brains.kept.len() as i64 + 1;

      brains.kept.push((agent.clone(), id));

      Json(json!({ "id": id, "accountKey": body["accountKey"], "agentKey": agent, "name": body["name"], "createdAt": 1 }))
        .into_response()
    }
    ("DELETE", member_path) if member_path.starts_with("brains/") => {
      let id: i64 = member_path.trim_start_matches("brains/").parse().unwrap();

      brains.kept.retain(|(_, kept)| *kept != id);
      Json(member("gone", id)).into_response()
    }
    _ => (StatusCode::NOT_FOUND, "nothing here").into_response(),
  }
}

async fn ask(method: reqwest::Method, port: u16, path: &str, body: Option<Value>) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth("secret");

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn post(port: u16, body: Value) -> (u16, Value) {
  ask(reqwest::Method::POST, port, "/agents", Some(body)).await
}

async fn delete(port: u16, path: &str) -> (u16, Value) {
  ask(reqwest::Method::DELETE, port, path, None).await
}

// The brain calls taken since the last look, as method and path with query.
fn asked(brains: &Kept) -> Vec<(String, String)> {
  std::mem::take(&mut brains.lock().unwrap().asked)
    .into_iter()
    .map(|(method, path, _)| (method, path))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_of_every_kind_are_registered_owned_given_a_brain_and_ended() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let brains = Kept::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let brain_url = format!("http://{}/", listener.local_addr().unwrap());
  let routes = axum::Router::new().fallback(brain).with_state(brains.clone());

  tokio::spawn(async move { axum::serve(listener, routes).await.unwrap() });
  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "builtin", "session_scope", "turn_shape", "default_answerer")
         VALUES ('helper', 'worker', 'Helper', 0, NULL, NULL, 0),
                ('chat-agent', 'chat', 'Chat', 0, 'agent', 'window', 1),
                ('gate-a', 'gate', 'Gate A', 1, NULL, NULL, 0)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server_with(
    home.path(),
    &data,
    &scratch.address(),
    "secret",
    &[("AGENCY_BRAIN_URL", brain_url.as_str()), ("AGENCY_BRAIN_ACCOUNT", "1")],
  );
  let port = server.port;
  let companion = json!({
    "name": "companion-a",
    "label": "Companion A",
    "kind": "chat",
    "ownerAccount": 42,
    "provider": "claude-code",
    "model": "m",
    "worker": "helper",
  });

  // A conversational agent: written as a companion added on the console, owned by the account named, and
  // given its brain under that account's key.
  let (status, created) = post(port, companion.clone()).await;

  assert_eq!(status, 201, "{created}");
  assert_eq!(created["brain"], "made");
  assert_eq!(created["agent"]["name"], "companion-a");
  assert_eq!(created["agent"]["ownerAccount"], 42);
  assert_eq!(created["agent"]["role"], "chat");
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agents" WHERE "name" = 'companion-a' AND "kind" = 'chat' AND "role" = 'chat'
           AND "owner_account" = 42 AND "session_scope" = 'room' AND "turn_shape" = 'pieces'
           AND "default_answerer" = 0 AND "pinned_provider" = 'claude-code' AND "pinned_model" = 'm'
           AND "item_worker" = 'helper' AND "brain_folder" = 'agents/companion-a' AND "builtin" = 0"#,
        vec![],
      )
      .await,
    1
  );

  let made = std::mem::take(&mut brains.lock().unwrap().asked);

  assert_eq!(
    made
      .iter()
      .map(|(method, path, _)| (method.as_str(), path.as_str()))
      .collect::<Vec<_>>(),
    [("GET", "brains?agentKey=companion-a&limit=1"), ("POST", "brains?")]
  );
  assert_eq!(
    made[1].2,
    json!({ "accountKey": "42", "agentKey": "companion-a", "name": "Companion A" })
  );

  // The key's rule, the kind, the role, the owner and a pin are each refused by their field, and a name
  // already held is a conflict; nothing is written for any of them.
  let refusals = [
    (json!({ "name": "Bad Name" }), "name"),
    (json!({ "name": "-lead" }), "name"),
    (json!({ "name": "x".repeat(41) }), "name"),
    (json!({ "label": "" }), "label"),
    (json!({ "kind": "nobody" }), "kind"),
    (json!({ "role": "nobody" }), "role"),
    (json!({ "ownerAccount": "someone" }), "ownerAccount"),
    (json!({ "effort": "enormous" }), "effort"),
    (json!({ "worker": "chat-agent" }), "worker"),
  ];

  for (change, field) in refusals {
    let mut body = companion.clone();

    body["name"] = json!("companion-b");

    for (key, value) in change.as_object().unwrap() {
      body[key] = value.clone();
    }

    let (status, refused) = post(port, body).await;

    assert_eq!(
      (status, refused["code"].as_str(), refused["field"].as_str()),
      (400, Some("bad-request"), Some(field)),
      "{refused}"
    );
  }

  let (status, taken) = post(port, companion.clone()).await;

  assert_eq!((status, taken["code"].as_str()), (409, Some("name-taken")), "{taken}");
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "agents""#, vec![]).await, 4);
  assert!(
    asked(&brains).iter().all(|(method, _)| method != "POST"),
    "no refused agent was given a brain"
  );

  // A worker and a gate: no owner named reads as the administrator's, the kind's role is theirs, and
  // neither is given a brain.
  let (status, worker) = post(
    port,
    json!({ "name": "second-worker", "label": "Second", "kind": "worker", "thinking": true }),
  )
  .await;

  assert_eq!((status, worker["brain"].as_str()), (201, Some("none")), "{worker}");
  assert_eq!(worker["agent"]["ownerAccount"], Value::Null);
  assert_eq!(worker["agent"]["role"], "worker");
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agents" WHERE "name" = 'second-worker' AND "kind" = 'worker'
           AND "cwd" IS NOT NULL AND "thinking" = 1 AND "owner_account" IS NULL AND "brain_folder" IS NULL"#,
        vec![],
      )
      .await,
    1
  );

  let (status, gate) =
    post(port, json!({ "name": "gate-b", "label": "Gate B", "kind": "gate", "ownerAccount": 7 })).await;

  assert_eq!(
    (status, gate["brain"].as_str(), gate["agent"]["role"].as_str()),
    (201, Some("none"), Some("gate"))
  );
  assert_eq!(asked(&brains), Vec::<(String, String)>::new());

  // An agent of another kind holding the chat role is given its brain too.
  let (status, runner) =
    post(port, json!({ "name": "talker", "label": "Talker", "kind": "runner", "role": "chat" })).await;

  assert_eq!((status, runner["brain"].as_str()), (201, Some("made")), "{runner}");
  asked(&brains);

  // Every row of the list carries its owner.
  let (status, list) = ask(reqwest::Method::GET, port, "/agents", None).await;

  assert_eq!(status, 200);

  // Each role and bundle carries the code a screen words it by beside its English label.
  let chat = list["roles"].as_array().unwrap().iter().find(|role| role["name"] == "chat").unwrap();

  assert_eq!((&chat["code"], &chat["label"]), (&json!("chat"), &json!("Chat")));
  assert_eq!(
    chat["bundles"][0],
    json!({ "name": "room.chat", "code": "room-chat", "label": "Memory, notes and TODO" })
  );

  let owners: Vec<(String, Value)> = list["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|agent| {
      assert!(agent.as_object().unwrap().contains_key("ownerAccount"), "{agent}");
      (agent["name"].as_str().unwrap().to_owned(), agent["ownerAccount"].clone())
    })
    .collect();

  assert!(owners.contains(&("companion-a".into(), json!(42))));

  // Every agent of a kind is shown with its kind's mark, whatever its row holds.
  for agent in list["items"].as_array().unwrap() {
    if let Some(mark) = policy::roles::ROLES.icon(agent["kind"].as_str().unwrap_or_default()) {
      assert_eq!(agent["icon"], mark, "{agent}");
    }
  }
  assert!(owners.contains(&("gate-b".into(), json!(7))));
  assert!(owners.contains(&("helper".into(), Value::Null)));

  // An ending refused: no such agent, one the deployment seeds or the default answerer whatever force
  // says, and one a turn holds unless forced.
  assert_eq!(delete(port, "/agents/nobody").await.0, 404);

  for seeded in ["/agents/gate-a", "/agents/chat-agent", "/agents/gate-a?force=true"] {
    let (status, refused) = delete(port, seeded).await;

    assert_eq!((status, refused["code"].as_str()), (409, Some("seeded")), "{seeded}: {refused}");
  }

  let turn = try_hold(&agent_lock_path(&data, "second-worker")).unwrap();
  let (status, refused) = delete(port, "/agents/second-worker").await;

  assert_eq!((status, refused["code"].as_str()), (409, Some("running")), "{refused}");
  assert_eq!(delete(port, "/agents/second-worker?force=true").await.0, 204);
  drop(turn);

  // Ended with its brain kept, the brain is not let go; ended with forgetBrain, it is.
  assert_eq!(delete(port, "/agents/companion-a?rooms=-7").await.0, 204);
  assert!(asked(&brains).iter().all(|(method, _)| method != "DELETE"), "the brain stays as data");
  assert_eq!(
    brains.lock().unwrap().kept.iter().filter(|(agent, _)| agent == "companion-a").count(),
    1
  );

  assert_eq!(delete(port, "/agents/talker?forgetBrain=true").await.0, 204);
  assert!(
    asked(&brains)
      .iter()
      .any(|(method, path)| method == "DELETE" && path.starts_with("brains/"))
  );
  assert_eq!(brains.lock().unwrap().kept.iter().filter(|(agent, _)| agent == "talker").count(), 0);

  assert_eq!(delete(port, "/agents/gate-b").await.0, 204);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agents" WHERE "name" IN ('companion-a', 'second-worker', 'talker', 'gate-b')"#,
        vec![],
      )
      .await,
    0
  );
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "agents""#, vec![]).await, 3);
}
