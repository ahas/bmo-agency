//! An engine's health: its answer read field by field, and the addresses asked in order.

use axum::Router;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use harness_machine::engine_health::*;
use serde_json::json;

#[test]
fn an_answer_is_read_field_by_field_and_a_wrong_shape_is_none() {
  assert_eq!(
    read_health(
      &json!({ "context": 4096, "model": "rvn.gguf", "in_hand": 0, "aside": { "reason": "training", "until": 99 } })
    ),
    Some(EngineHealth {
      context: Some(4096),
      model: Some("rvn.gguf".into()),
      in_hand: Some(0),
      aside: Some(Aside {
        reason: "training".into(),
        until: json!(99)
      }),
    })
  );
  assert_eq!(
    read_health(&json!({ "context": 0, "model": "", "in_hand": -1, "aside": { "reason": 3 } })),
    Some(EngineHealth {
      context: None,
      model: None,
      in_hand: None,
      aside: None
    })
  );
  assert_eq!(read_health(&json!("ok")), None);
}

// A stand-in engine on the loopback that answers its health only to the key it was given.
async fn engine(key: &'static str) -> String {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let at = listener.local_addr().unwrap();
  let app = Router::new().route(
    "/health",
    get(move |headers: HeaderMap| async move {
      let given = headers.get("authorization").and_then(|value| value.to_str().ok()).unwrap_or_default();

      if given == format!("Bearer {key}") {
        axum::Json(json!({ "model": "rvn.gguf", "context": 8192 })).into_response()
      } else {
        axum::http::StatusCode::UNAUTHORIZED.into_response()
      }
    }),
  );

  tokio::spawn(async move { axum::serve(listener, app).await });

  format!("http://{at}/v1/chat/completions")
}

#[tokio::test]
async fn the_first_address_with_a_key_that_answers_gives_the_health() {
  let root = tempfile::tempdir().unwrap();
  let url = engine("k-right").await;

  std::fs::create_dir_all(root.path().join("env")).unwrap();
  std::fs::write(root.path().join("env/wrong.key"), "k-wrong\n").unwrap();
  std::fs::write(root.path().join("env/right.key"), "k-right\n").unwrap();

  let addresses = |files: &[Option<&str>]| -> Vec<Address> {
    files
      .iter()
      .map(|file| Address {
        url: url.clone(),
        key_file: file.map(str::to_owned),
      })
      .collect()
  };

  assert_eq!(
    health(
      root.path(),
      &addresses(&[None, Some("missing.key"), Some("wrong.key"), Some("right.key")])
    )
    .await
    .and_then(|health| health.model),
    Some("rvn.gguf".into())
  );
  assert_eq!(health(root.path(), &addresses(&[None, Some("wrong.key")])).await, None);

  // An address that does not answer at all leaves the next to be asked.
  let unanswered = Address {
    url: "http://127.0.0.1:9/v1/chat/completions".into(),
    key_file: Some("right.key".into()),
  };

  assert!(
    health(root.path(), &[unanswered, addresses(&[Some("right.key")]).remove(0)])
      .await
      .is_some()
  );
}

#[test]
fn an_operators_arbiter_sets_the_model_aside_while_it_is_not_awake_with_its_mode_and_basis() {
  use harness_machine::engine_health::read_arbiter_aside;

  let status = json!({
    "node_mode": "setlock",
    "node_mode_basis": "pin model-a by the owner until 17:01 (standing)",
    "awake": ["model-b"],
    "pins_effective": { "model-a": { "expires": 2109225676, "until": "17:01" } }
  });

  // Not awake: aside, with the arbiter's own words and the pin's expiry.
  let aside = read_arbiter_aside(&status, "model-a").unwrap();

  assert_eq!(aside.reason, "setlock: pin model-a by the owner until 17:01 (standing)");
  assert_eq!(aside.until, json!(2109225676_i64));

  // Awake: nothing set aside.
  assert_eq!(read_arbiter_aside(&status, "model-b"), None);

  // A status with no awake list says nothing; one with no basis names the mode alone.
  assert_eq!(read_arbiter_aside(&json!({ "node_mode": "bench" }), "model-a"), None);
  assert_eq!(
    read_arbiter_aside(&json!({ "node_mode": "bench", "awake": [] }), "model-a")
      .unwrap()
      .reason,
    "bench"
  );
}
