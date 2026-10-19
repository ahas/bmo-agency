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
  let names: Vec<&str> =
    providers.as_array().unwrap().iter().map(|one| one["name"].as_str().unwrap()).collect();

  assert_eq!(status, 200, "{providers}");
  assert_eq!(names, ["codex", "claude-code", "openrouter", "registered"]);
  assert_eq!((&providers[0]["enabled"], &providers[1]["enabled"]), (&json!(false), &json!(true)));
  assert_eq!(
    (&providers[1]["registered"], &providers[1]["coworker"], &providers[3]["registered"]),
    (&json!(false), &json!(true), &json!(true))
  );
  assert!(providers[1]["lines"].as_array().unwrap().contains(&json!("worker")), "{providers}");

  // The tier ladder, with the tiers a co-worker is borrowed at.
  let (_, tiers) = get(port, token, "/tiers").await;

  assert_eq!(tiers[0], json!({ "name": "BMO", "coworker": false }));
  assert!(
    tiers.as_array().unwrap().contains(&json!({ "name": "Reliable", "coworker": true })),
    "{tiers}"
  );

  // A route on the provider named, on the first that serves it where none is, and none on one switched off.
  let asked = json!({ "workKey": "job-worker", "provider": "claude-code", "tier": "Frontier" });
  let (status, mapping) = post(port, token, "/routes/resolve", asked).await;

  assert_eq!(status, 200, "{mapping}");
  assert_eq!((&mapping["provider"], &mapping["tier"]), (&json!("claude-code"), &json!("Frontier")));
  assert!(mapping["model"].as_str().is_some_and(|model| !model.is_empty()), "{mapping}");
  assert!(mapping.get("answerTokens").is_some(), "{mapping}");

  let (_, walked) = post(
    port,
    token,
    "/routes/resolve",
    json!({ "workKey": "job-worker", "provider": null, "tier": "Frontier" }),
  )
  .await;

  assert_eq!(
    walked["provider"],
    json!("claude-code"),
    "codex is off, so the walk's first is Claude Code"
  );

  let (status, off) = post(
    port,
    token,
    "/routes/resolve",
    json!({ "workKey": "job-worker", "provider": "codex", "tier": "Frontier" }),
  )
  .await;

  assert_eq!((status, off["code"].as_str()), (404, Some("nothing-held")), "{off}");
  assert_eq!(
    post(port, token, "/routes/resolve", json!({ "workKey": "job-worker", "tier": "Huge" }))
      .await
      .0,
    400
  );

  // A pin's value: none, a provider, and values nothing could run on.
  assert_eq!(
    post(port, token, "/pins/check", json!({ "value": "auto" })).await,
    (200, json!({ "kind": "auto" }))
  );
  assert_eq!(
    post(port, token, "/pins/check", json!({ "value": "claude-code" })).await,
    (200, json!({ "kind": "pinned", "provider": "claude-code", "model": null }))
  );
  assert_eq!(post(port, token, "/pins/check", json!({ "value": "nobody" })).await.0, 400);
  assert_eq!(post(port, token, "/pins/check", json!({ "value": "registered::none" })).await.0, 400);

  // A model's engine health: none on a provider whose models are not registrations, nor on one not held.
  assert_eq!(
    get(port, token, "/providers/claude-code/models/any/health").await,
    (200, json!({ "answers": false }))
  );
  assert_eq!(
    get(port, token, "/providers/registered/models/none/health").await,
    (200, json!({ "answers": false }))
  );
  assert_eq!(get(port, token, "/providers/nobody/models/any/health").await.0, 400);
}
