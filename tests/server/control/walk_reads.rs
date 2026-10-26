//! The providers and the tiers as the job unit walks them, asked of a running server on the store it seeded:
//! the providers in the provider order with each one's own declaration, the tier ladder with its co-worker
//! tiers, a route resolved on one provider and on the first that serves it, refused where its provider is
//! off or its tier is no tier, a pin's value read, and a model's engine health.

use std::os::unix::fs::PermissionsExt;

use policy::providers::deepseek::DEEPSEEK;
use policy::providers::registered::REGISTERED;
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

// DeepSeek's key and the gateway's, private under the data root's env directory, so both can answer here.
fn keys_in(data: &std::path::Path) {
  let env = data.join("env");

  std::fs::create_dir_all(&env).unwrap();

  for name in [&DEEPSEEK.presence.key_file, &REGISTERED.presence.key_file] {
    let file = env.join(name);

    std::fs::write(&file, "a key\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  }
}

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
  assert_eq!(
    (&providers[3]["code"], &providers[3]["label"]),
    (&json!("registered"), &json!("Registered models"))
  );

  // Every work key with its tier, the tiers it may be, and what it resolves to on each provider.
  let (status, keys) = get(port, token, "/work-keys").await;
  let job = keys["items"].as_array().unwrap().iter().find(|one| one["key"] == "job-worker").cloned();

  assert_eq!(status, 200, "{keys}");
  let job = job.unwrap_or_else(|| panic!("the job key is listed: {keys}"));
  assert!(job["tier"].as_str().is_some_and(|tier| !tier.is_empty()), "{job}");
  assert!(job["choices"].as_array().is_some_and(|choices| !choices.is_empty()), "{job}");
  assert!(
    job["effective"]
      .as_array()
      .is_some_and(|cells| cells.iter().all(|cell| cell.get("errorCode").is_some())),
    "{job}"
  );

  // The tier ladder, with the tiers a co-worker is borrowed at.
  let (_, tiers) = get(port, token, "/tiers").await;
  let tiers = &tiers["items"];

  assert_eq!((&tiers[0]["name"], &tiers[0]["coworker"]), (&json!("Supervisor"), &json!(false)));
  assert!(
    tiers[0]["mapping"]["claude-code"]["model"]
      .as_str()
      .is_some_and(|model| !model.is_empty()),
    "{tiers}"
  );
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

#[tokio::test(flavor = "multi_thread")]
async fn a_route_read_with_no_provider_begins_on_the_key_s_own_road_and_registration() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  keys_in(&data);
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('provider_order', '[{"name":"deepseek","enabled":true},{"name":"registered","enabled":true},{"name":"claude-code","enabled":true}]')"#,
      vec![],
    )
    .await;

  for name in ["tier-cell", "own-pick"] {
    scratch
      .run(
        r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
              "wait_seconds", "thinking", "walked", "added_at", "keeps_research")
           VALUES ($1, $1, 'served', 100000, 2000, 60, 0, 1, 1, 0)"#,
        vec![name.into()],
      )
      .await;
    scratch
      .run(
        r#"INSERT INTO "registered_model_addresses" ("registration", "node", "position", "url", "key_file", "key_owned")
           VALUES ($1, 'n1', 1, 'http://127.0.0.1:9/v1/chat/completions', NULL, 0)"#,
        vec![name.into()],
      )
      .await;
  }

  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort", "answer_tokens", "thinking")
         VALUES ('registered', 'Reliable', 'tier-cell', NULL, 2048, false)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "model" = excluded."model""#,
      vec![],
    )
    .await;
  // The translator set to the registry's provider on a registration of its own; a gate on Laya walks.
  scratch
    .run(
      r#"INSERT INTO "fallback_routes" ("key", "set_at", "route_mode", "registration")
         VALUES ('translator', 1, 'registered', 'own-pick'), ('output-format', 1, 'laya', NULL)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let (status, pinned) = get(port, token, "/work-keys/translator?tier=Reliable").await;

  assert_eq!(status, 200, "{pinned}");
  assert_eq!(
    (&pinned["provider"], &pinned["model"]),
    (&json!("registered"), &json!("own-pick")),
    "the key's own road is asked before DeepSeek, first in the order, and its registration before the cell"
  );

  let (_, walked) = get(port, token, "/work-keys/output-format?tier=Reliable").await;

  assert_eq!(
    walked["provider"],
    json!("deepseek"),
    "a key on Laya names no provider and walks the order"
  );
  assert_eq!(pinned["runMode"], json!("registered"), "the road the key is set to");
  assert_eq!(
    walked["runMode"],
    json!("laya"),
    "the road the key stands on, never a word of the read's own"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_walk_passes_over_a_shut_road_and_a_key_set_to_a_provider_stays_on_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  keys_in(&data);
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('provider_order', '[{"name":"deepseek","enabled":true},{"name":"claude-code","enabled":true}]')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "fallback_routes" ("key", "set_at", "route_mode") VALUES ('output-format', 1, 'laya'),
           ('sanitizer', 1, 'deepseek'), ('translator', 1, 'auto')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let (_, before) = get(port, token, "/work-keys/output-format?tier=Simple").await;

  assert_eq!(
    (&before["provider"], &before["runMode"]),
    (&json!("deepseek"), &json!("laya")),
    "{before}"
  );

  // DeepSeek's model shut by a failure still standing: the walk passes it over for the next road.
  let until = agency_server::now_ms() + 600_000;
  let selection = format!("deepseek::{}", before["model"].as_str().unwrap());
  let (status, blocked) = post(
    port,
    token,
    "/road-blocks",
    json!({ "selection": selection, "until": until, "reason": "the road is down" }),
  )
  .await;

  assert_eq!(status, 201, "{blocked}");

  for key in ["output-format", "translator"] {
    let (status, walked) = get(port, token, &format!("/work-keys/{key}?tier=Simple")).await;

    assert_eq!(status, 200, "{walked}");
    assert_eq!(walked["provider"], json!("claude-code"), "{key}: {walked}");
  }

  // A key set to DeepSeek is the owner's choice and stays on it, its run mode naming it.
  let (_, set) = get(port, token, "/work-keys/sanitizer?tier=Simple").await;

  assert_eq!((&set["provider"], &set["runMode"]), (&json!("deepseek"), &json!("deepseek")), "{set}");
  assert_eq!(get(port, token, "/work-keys/translator").await.1["runMode"], json!("auto"));
}
