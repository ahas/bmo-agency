//! What the console reads of a running server over its control routes (docs/api.md): each provider with its
//! default compaction threshold and typed quota; a supervisor's reset point alone and the rooms its turns went
//! into; the stand-in's provider; a repository's typed checkouts and sampling settings, an add with no path
//! refused, a node checkout on a node that takes no jobs refused; the nodes with whether they take jobs; a
//! seeded companion's removal refused as seeded; and a slot asked for a model alone, its runs on `/slots`, an
//! owner's release and the admission's state after it — on a store the server settled itself.

use serde_json::{Value, json};

use crate::support::rows::{repository, supervisor};
use crate::support::scratch::Scratch;
use crate::support::standing;

async fn call(
  port: u16,
  token: &str,
  method: reqwest::Method,
  path: &str,
  body: Option<Value>,
) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token);

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  call(port, token, reqwest::Method::GET, path, None).await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_the_fields_the_console_reads() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  supervisor(scratch.db(), "chat-agent", &[("default_answerer", 1i64.into())]).await;
  supervisor(
    scratch.db(),
    "seeded-one",
    &[
      ("builtin", 1i64.into()),
      ("session_scope", "room".into()),
      ("turn_shape", "pieces".into()),
    ],
  )
  .await;
  repository(scratch.db(), "kept", "/checkouts/kept", &[]).await;
  scratch
    .run(
      r#"INSERT INTO "runs" ("agent", "chat_id", "unit", "depth", "started_at", "finished_at")
         VALUES ('chat-agent', 41, 'chat-agent-1', 0, 1000, 2000), ('chat-agent', 41, 'chat-agent-2', 0, 3000, NULL),
                ('chat-agent', 42, 'chat-agent-3', 0, 2500, 2600)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Each provider names its default compaction threshold and what it has left, typed.
  let (status, providers) = get(port, token, "/providers").await;

  assert_eq!(status, 200, "{providers}");
  for provider in providers["items"].as_array().unwrap() {
    assert!(
      provider["foldRatioDefault"].as_f64().is_some_and(|ratio| ratio > 0.0 && ratio <= 1.0),
      "{provider}"
    );
    assert!(provider["quota"]["has"].is_boolean(), "{provider}");
    assert!(provider["lines"].is_array(), "{provider}");
  }

  let deepseek = providers["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|one| one["name"] == "deepseek")
    .unwrap();

  assert_eq!(deepseek["foldRatioDefault"], json!(0.5), "DeepSeek declares 0.5");

  // The reset point alone, and the rooms the supervisor's turns went into, the latest first.
  assert_eq!(
    get(port, token, "/supervisors/chat-agent/reset-point").await,
    (200, json!({ "supervisor": "chat-agent", "resetPoint": null }))
  );
  assert_eq!(get(port, token, "/supervisors/nobody/reset-point").await.0, 404);

  let (status, rooms) = get(port, token, "/supervisors/chat-agent/rooms").await;

  assert_eq!(status, 200, "{rooms}");
  assert_eq!(rooms["items"][0]["roomId"], 41, "{rooms}");
  assert_eq!(rooms["items"][0]["runs"], 2);
  assert_eq!(rooms["items"][0]["lastRunUnit"], "chat-agent-2");
  assert_eq!(rooms["items"][0]["lastFinishedAt"], Value::Null);
  assert_eq!(rooms["items"][1]["roomId"], 42);

  // No stand-in answers while Claude Code stands open, and none is named.
  let (_, stand_in) = get(port, token, "/stand-in?agent=chat-agent").await;

  assert_eq!(stand_in["answers"], false, "{stand_in}");
  assert_eq!(stand_in["provider"], Value::Null);

  // A repository's checkouts are typed and its sampling's settings named with their bounds.
  let (status, kept) = get(port, token, "/repositories/kept").await;

  assert_eq!(status, 200, "{kept}");
  assert_eq!(kept["checkouts"][0]["kind"], "base", "{kept}");
  assert_eq!(kept["checkouts"][0]["job"], Value::Null);
  assert!(
    kept["samplingSettings"]
      .as_array()
      .unwrap()
      .iter()
      .any(|setting| setting["name"] == "temperature"),
    "{kept}"
  );
  assert!(
    !kept["samplingSettings"]
      .as_array()
      .unwrap()
      .iter()
      .any(|setting| setting["name"] == "max_tokens"),
    "the answer ceiling stays the registration's"
  );

  let (status, refused) =
    call(port, token, reqwest::Method::POST, "/repositories", Some(json!({ "name": "pathless" }))).await;

  assert_eq!((status, refused["field"].clone()), (400, json!("path")), "{refused}");

  // The node service keeps no node here, so none takes jobs and a checkout on one is refused.
  assert_eq!(get(port, token, "/nodes").await, (200, json!({ "items": [], "more": false })));

  let (status, refused) = call(
    port,
    token,
    reqwest::Method::PATCH,
    "/repositories/kept",
    Some(json!({ "nodeCheckouts": [{ "nodeName": "far", "path": null }] })),
  )
  .await;

  assert_eq!((status, refused["field"].clone()), (400, json!("nodeCheckouts")), "{refused}");
  assert_eq!(get(port, token, "/repositories/kept").await.1["checkouts"].as_array().unwrap().len(), 1);

  // A companion added with its order whole is written with it, its share beside it.
  let (status, added) = call(
    port,
    token,
    reqwest::Method::POST,
    "/companions",
    Some(json!({
      "name": "pal",
      "label": "Pal",
      "worker": "kept",
      "order": [{ "provider": "claude-code", "model": "claude-opus-5" }],
      "quotaShare": 0.05,
    })),
  )
  .await;

  assert_eq!(status, 201, "{added}");

  let (_, pal) = get(port, token, "/supervisors/pal").await;

  assert_eq!(pal["order"][0]["model"], "claude-opus-5", "{pal}");
  assert_eq!(pal["quotaShare"], json!(0.05), "{pal}");

  let (status, refused) = call(
    port,
    token,
    reqwest::Method::POST,
    "/companions",
    Some(json!({
      "name": "pal-two",
      "label": "Pal",
      "worker": "kept",
      "provider": "deepseek",
      "model": "deepseek-flash",
      "order": [{ "provider": "claude-code", "model": "claude-opus-5" }],
    })),
  )
  .await;

  assert_eq!((status, refused["field"].clone()), (400, json!("order")), "{refused}");

  // A seeded companion's removal is refused as seeded.
  let (status, refused) = call(port, token, reqwest::Method::DELETE, "/companions/seeded-one", None).await;

  assert_eq!((status, refused["code"].clone()), (409, json!("seeded")), "{refused}");

  // A slot asked for a model alone goes out on the provider the role's order names it under.
  let (status, roles) = get(port, token, "/roles/scout").await;

  assert_eq!(status, 200, "{roles}");

  let entry = roles["order"][0].clone();
  let (status, asked) = call(
    port,
    token,
    reqwest::Method::POST,
    "/slot-requests",
    Some(
      json!({ "role": "scout", "requestKey": "console:one", "owner": "console", "model": entry["model"] }),
    ),
  )
  .await;

  assert_eq!(status, 201, "{asked}");
  assert_eq!(asked["provider"], entry["provider"], "{asked}");

  let (status, refused) = call(
    port,
    token,
    reqwest::Method::POST,
    "/slot-requests",
    Some(
      json!({ "role": "scout", "requestKey": "console:two", "owner": "console", "provider": "deepseek" }),
    ),
  )
  .await;

  assert_eq!(status, 400, "{refused}");

  // The owner gives back everything it holds in one ask, and an admission then says who gave it back.
  let token_of = asked["token"].as_str().unwrap().to_owned();
  let (status, released) =
    call(port, token, reqwest::Method::DELETE, "/slot-requests?owner=console", None).await;

  assert_eq!(status, 200, "{released}");
  assert_eq!(
    released["cancelled"].as_i64().unwrap() + released["released"].as_i64().unwrap(),
    1,
    "{released}"
  );
  assert_eq!(released["sessionsEnded"], json!([]));

  let (status, admission) = call(
    port,
    token,
    reqwest::Method::PATCH,
    &format!("/slot-requests/{token_of}"),
    Some(json!({})),
  )
  .await;

  assert_eq!(status, 200, "{admission}");
  assert_eq!(admission["request"], Value::Null);
  assert!(["released", "cancelled"].contains(&admission["state"].as_str().unwrap()), "{admission}");
  assert_eq!(admission["endedBy"], "owner");
  assert_eq!(call(port, token, reqwest::Method::DELETE, "/slot-requests", None).await.0, 400);

  // Every role's slots are typed, each run let in naming its token.
  let (status, slots) = get(port, token, "/slots").await;

  assert_eq!(status, 200, "{slots}");
  assert!(slots["items"].as_array().unwrap().iter().any(|role| role["role"] == "scout"), "{slots}");
}
