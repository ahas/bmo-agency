//! OpenRouter: its listing, its presence by the key, and its resident loop told by the script's name.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use harness_providers::openrouter::*;
use harness_providers::{Machine, Provider};
use policy::providers::openrouter::OPENROUTER;
use serde_json::{Value, json};

// A stand-in listing on the loopback, answering with a status and a body and keeping the accept
// header each call carried.
async fn listing_at(status: StatusCode, body: Value) -> (String, Arc<Mutex<Vec<String>>>) {
  let seen = Arc::new(Mutex::new(Vec::new()));
  let kept = seen.clone();
  let app = axum::Router::new().route(
    "/api/v1/models",
    get(move |headers: HeaderMap| {
      let kept = kept.clone();
      let body = body.clone();

      async move {
        kept.lock().unwrap().push(
          headers
            .get("accept")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned(),
        );
        (status, Json(body))
      }
    }),
  );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (format!("http://{address}/api/v1/models"), seen)
}

fn machine_reading(url: &str, dir: &Path) -> Machine {
  Machine::with_env([("BMO_OPENROUTER_LISTING_URL".to_owned(), url.to_owned())], dir)
}

/// needs no key and keeps the four things a choice is made on
#[tokio::test]
async fn needs_no_key_and_keeps_the_four_things_a_choice_is_made_on() {
  let dir = tempfile::tempdir().unwrap();
  let (url, seen) = listing_at(StatusCode::OK, json!({ "data": [
    { "id": "deepseek/deepseek-chat:free", "name": "DeepSeek: DeepSeek V3 (free)", "context_length": 163_840,
      "pricing": { "prompt": "0", "completion": "0" }, "description": "a paragraph nobody choosing needs" },
    { "id": "z-ai/glm-4.6", "name": "Z.AI: GLM 4.6", "context_length": 200_000, "pricing": { "prompt": "0.0000004", "completion": "0.0000017" } },
    { "name": "a row with no id at all" },
  ]}))
  .await;

  assert_eq!(
    models(&machine_reading(&url, dir.path())).await.unwrap(),
    [
      Model {
        id: "deepseek/deepseek-chat:free".into(),
        name: "DeepSeek: DeepSeek V3 (free)".into(),
        context_length: Some(163_840),
        free: true
      },
      Model {
        id: "z-ai/glm-4.6".into(),
        name: "Z.AI: GLM 4.6".into(),
        context_length: Some(200_000),
        free: false
      },
    ]
  );
  assert_eq!(*seen.lock().unwrap(), ["application/json"]);
}

/// a free suffix over a price that is not nought is not free
#[test]
fn a_free_suffix_over_a_price_that_is_not_nought_is_not_free() {
  let read = read_models(&json!({ "data": [
    { "id": "someone/model:free", "pricing": { "prompt": "0.000001", "completion": "0" } },
    { "id": "named/free:free", "pricing": { "prompt": 0, "completion": "0" } },
    { "id": "openai/gpt-5", "pricing": { "prompt": "0", "completion": "0" } },
  ]}));

  assert_eq!(read.iter().map(|one| one.free).collect::<Vec<_>>(), [false, true, false]);
  assert_eq!(read[0].name, "someone/model:free");
  assert_eq!(read_models(&json!({})), []);
}

#[tokio::test]
async fn a_listing_that_did_not_arrive_says_its_status() {
  let dir = tempfile::tempdir().unwrap();
  let (url, _) = listing_at(StatusCode::BAD_GATEWAY, json!({})).await;

  assert_eq!(
    models(&machine_reading(&url, dir.path())).await.unwrap_err(),
    "openrouter models: HTTP 502 Bad Gateway"
  );
}

#[test]
fn the_key_in_the_environment_is_the_whole_of_its_presence() {
  let dir = tempfile::tempdir().unwrap();

  assert!(OpenRouter.missing(&Machine::with_env([], dir.path())).is_some());
  assert!(
    OpenRouter
      .missing(&Machine::with_env([("BMO_OPENROUTER_KEY".into(), String::new())], dir.path()))
      .is_some()
  );
  assert_eq!(
    OpenRouter.missing(&Machine::with_env([("BMO_OPENROUTER_KEY".into(), "k".into())], dir.path())),
    None
  );
}

#[test]
fn a_loop_is_its_own_by_the_scripts_file_name_from_whichever_folder_it_was_started() {
  let dir = tempfile::tempdir().unwrap();
  let machine = Machine::with_env([], dir.path());
  let owns = |script: &str, provider: &str| {
    let args = ["bun", script, "--provider", provider, "--place", "/place"].map(str::to_owned);

    OpenRouter.owns_program(&machine, &args, &BTreeMap::new())
  };
  let declared = OPENROUTER
    .declared
    .resident
    .as_ref()
    .and_then(|resident| resident.loop_script.as_deref());

  assert_eq!(declared, Some("packages/agent/resident-loop.ts"));

  for script in ["/deploy/packages/agent/resident-loop.ts", "/checkout/packages/agent/resident-loop.ts"] {
    assert!(owns(script, "openrouter"), "{script}");
  }

  assert!(!owns("/deploy/packages/agent/resident-loop.ts", "codex"));
  assert!(!owns("/deploy/packages/agent/other-loop.ts", "openrouter"));
}
