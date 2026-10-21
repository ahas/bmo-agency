//! The providers and the tiers as the job unit walks them, asked of a running server on the store it seeded:
//! the providers in the provider order with each one's own declaration, the tier ladder with its co-worker
//! tiers, a route resolved on one provider and on the first that serves it, refused where its provider is
//! off or its tier is no tier, a pin's value read, and a model's engine health.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn post(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .post(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_the_walk_s_providers_tiers_routes_and_pins() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('provider_order', '[{"name":"codex","enabled":false},{"name":"claude-code","enabled":true}]')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // The providers in the owner's order, each answering for itself.
  let (status, providers) = get(port, token, "/providers").await;
  let providers = &providers["items"];
  let names: Vec<&str> =
    providers.as_array().unwrap().iter().map(|one| one["name"].as_str().unwrap()).collect();

  assert_eq!(status, 200, "{providers}");
  assert_eq!(names, ["codex", "claude-code", "openrouter", "registered", "deepseek"]);
  assert_eq!((&providers[0]["enabled"], &providers[1]["enabled"]), (&json!(false), &json!(true)));
  assert_eq!(
    (&providers[1]["registered"], &providers[1]["coworker"], &providers[3]["registered"]),
    (&json!(false), &json!(true), &json!(true))
  );

  // The tier ladder, with the tiers a co-worker is borrowed at.
  let (_, tiers) = get(port, token, "/tiers").await;
  let tiers = &tiers["items"];

  assert_eq!((&tiers[0]["name"], &tiers[0]["coworker"]), (&json!("BMO"), &json!(false)));
  assert!(
    tiers
      .as_array()
      .unwrap()
      .iter()
      .any(|tier| tier["name"] == "Reliable" && tier["coworker"] == true),
    "{tiers}"
  );

  // A route on the provider named, on the first that serves it where none is, and none on one switched off.
  let (status, mapping) =
    get(port, token, "/work-keys/job-worker?provider=claude-code&tier=Frontier").await;

  assert_eq!(status, 200, "{mapping}");
  assert_eq!((&mapping["provider"], &mapping["key"]), (&json!("claude-code"), &json!("job-worker")));
  assert!(mapping["model"].as_str().is_some_and(|model| !model.is_empty()), "{mapping}");
  assert!(mapping.get("answerTokens").is_some(), "{mapping}");

  let (_, walked) = get(port, token, "/work-keys/job-worker?tier=Frontier").await;

  assert_eq!(
    walked["provider"],
    json!("claude-code"),
    "codex is off, so the walk's first is Claude Code"
  );

  let (status, off) = get(port, token, "/work-keys/job-worker?provider=codex&tier=Frontier").await;

  assert_eq!((status, off["code"].as_str()), (404, Some("not-found")), "{off}");
  assert_eq!(get(port, token, "/work-keys/job-worker?tier=Huge").await.0, 400);

  // A pin's value: none, a provider, and values nothing could run on.
  assert_eq!(
    post(port, token, "/pin-checks", json!({ "value": "auto" })).await,
    (200, json!({ "kind": "auto", "provider": null, "model": null }))
  );
  assert_eq!(
    post(port, token, "/pin-checks", json!({ "value": "claude-code" })).await,
    (200, json!({ "kind": "provider", "provider": "claude-code", "model": null }))
  );
  assert_eq!(post(port, token, "/pin-checks", json!({ "value": "nobody" })).await.0, 400);
  assert_eq!(post(port, token, "/pin-checks", json!({ "value": "registered::none" })).await.0, 400);

  // A model's engine health: none on a provider whose models are not registrations, and not-found for a
  // registration the registry does not hold or a provider this build does not carry.
  assert_eq!(get(port, token, "/models/any?provider=claude-code").await.1["healthy"], json!(false));
  assert_eq!(get(port, token, "/models/none?provider=registered").await.0, 404);
  assert_eq!(get(port, token, "/models/any?provider=nobody").await.0, 404);
}
