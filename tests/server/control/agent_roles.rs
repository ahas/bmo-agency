//! The roles an agent may hold, asked of a running server (docs/api.md, 「agent-roles」): the policy's roles
//! seeded once with their prompts, a role added, changed and removed, each refusal with nothing written, a
//! role's tier written as its work key's assignment, and a co-worker tier's limit set on the pool.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

const TOKEN: &str = "secret";

async fn ask(method: reqwest::Method, port: u16, path: &str, body: Option<Value>) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(TOKEN);

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

fn role<'a>(listing: &'a Value, name: &str) -> Option<&'a Value> {
  listing["roles"].as_array().unwrap().iter().find(|role| role["name"] == name)
}

async fn assigned(scratch: &Scratch, key: &str) -> i64 {
  scratch
    .count(r#"SELECT count(*) FROM "model_tier_assignments" WHERE "key" = $1"#, vec![key.into()])
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_role_is_added_changed_and_removed_on_the_control_port() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), TOKEN);
  let port = server.port;

  // The policy's roles stand in the store, in its order, the translator's prompt its file's text.
  let (status, listing) = ask(reqwest::Method::GET, port, "/agent-roles", None).await;
  assert_eq!(status, 200, "{listing}");
  let names: Vec<&str> = listing["roles"]
    .as_array()
    .unwrap()
    .iter()
    .map(|role| role["name"].as_str().unwrap())
    .collect();
  assert_eq!(names, ["chat", "worker", "searcher", "translator", "video", "gate"]);

  let translator =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/translator.md")).unwrap();
  assert_eq!(role(&listing, "translator").unwrap()["prompt"], translator);
  assert_eq!(role(&listing, "chat").unwrap()["kindDefault"], true);
  assert_eq!(role(&listing, "video").unwrap()["kindDefault"], false);
  assert_eq!(role(&listing, "video").unwrap()["maxConcurrent"], 2);
  assert_eq!(role(&listing, "video").unwrap()["tier"], Value::Null);
  assert_eq!(
    role(&listing, "chat").unwrap()["bundles"][1],
    json!({ "name": "room.send", "label": "Send" })
  );
  assert_eq!(listing["bundles"].as_array().unwrap().len(), 8);

  // Each refusal is 422 with one sentence, and nothing is written.
  scratch
    .run(
      r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
         VALUES ('looker', 'Looks.', 'Look.', '[]', '[]', 1, 1, 1) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;

  for body in [
    json!({ "name": "Not_Kebab", "label": "X", "bundles": [] }),
    json!({ "name": "video", "label": "X", "bundles": [] }),
    json!({ "name": "looker", "label": "X", "bundles": [] }),
    json!({ "name": "reviewer", "label": "", "bundles": [] }),
    json!({ "name": "reviewer", "label": "Reviewer" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": ["web.everything"] }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "maxConcurrent": 0 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "tier": "Nowhere" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "prompt": 3 }),
  ] {
    let (status, refused) = ask(reqwest::Method::POST, port, "/agent-roles", Some(body.clone())).await;

    assert_eq!(status, 422, "{body}: {refused}");
    assert_eq!(refused["code"], "unprocessable-entity");
    assert!(refused["error"].as_str().unwrap().ends_with('.'), "{refused}");
  }
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "agent_roles""#, vec![]).await, 6);

  // A role added, its bundles once each, its tier its key's assignment, which the tier settings list.
  let (status, added) = ask(
    reqwest::Method::POST,
    port,
    "/agent-roles",
    Some(json!({
      "name": "reviewer",
      "label": "Reviewer",
      "bundles": ["web.read", "queue.read", "web.read"],
      "maxConcurrent": 2,
      "prompt": "Review what you are handed.",
      "tier": "Fine",
    })),
  )
  .await;
  assert_eq!(status, 201, "{added}");
  assert_eq!(
    added,
    json!({
      "name": "reviewer",
      "label": "Reviewer",
      "bundles": [{ "name": "web.read", "label": "Web reading" }, { "name": "queue.read", "label": "TODO reading" }],
      "maxConcurrent": 2,
      "prompt": "Review what you are handed.",
      "tier": "Fine",
      "agents": 0,
      "kindDefault": false,
    })
  );

  let (_, keys) = ask(reqwest::Method::GET, port, "/work-keys", None).await;
  let key = keys["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|item| item["key"] == "role:reviewer")
    .unwrap();
  assert_eq!((&key["tier"], &key["label"]), (&json!("Fine"), &json!("Reviewer")));

  // The agents' listing carries it, and an agent may hold it.
  let (_, agents) = ask(reqwest::Method::GET, port, "/agents", None).await;
  assert!(agents["roles"].as_array().unwrap().iter().any(|role| role["name"] == "reviewer"));

  let (status, registered) = ask(
    reqwest::Method::POST,
    port,
    "/agents",
    Some(json!({ "name": "checker", "label": "Checker", "kind": "gate", "role": "reviewer" })),
  )
  .await;
  assert_eq!(status, 201, "{registered}");

  let (status, refused) = ask(
    reqwest::Method::POST,
    port,
    "/agents",
    Some(json!({ "name": "other", "label": "Other", "kind": "gate", "role": "nobody" })),
  )
  .await;
  assert_eq!((status, refused["field"].as_str()), (400, Some("role")), "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("Reviewer"), "{refused}");

  // A change names only what it changes; a bound, a prompt or a tier given as null is cleared.
  let (status, changed) = ask(
    reqwest::Method::PATCH,
    port,
    "/agent-roles/reviewer",
    Some(json!({ "label": "Checker of work", "maxConcurrent": null, "prompt": null, "tier": null })),
  )
  .await;
  assert_eq!(status, 200, "{changed}");
  assert_eq!(changed["label"], "Checker of work");
  assert_eq!(changed["bundles"].as_array().unwrap().len(), 2);
  assert_eq!(
    (&changed["maxConcurrent"], &changed["prompt"], &changed["tier"]),
    (&Value::Null, &Value::Null, &Value::Null)
  );
  assert_eq!(changed["agents"], 1);
  assert_eq!(assigned(&scratch, "role:reviewer").await, 0);

  let (status, _) =
    ask(reqwest::Method::PATCH, port, "/agent-roles/reviewer", Some(json!({ "tier": "Light" }))).await;
  assert_eq!(status, 200);
  assert_eq!(assigned(&scratch, "role:reviewer").await, 1);

  let (status, _) = ask(
    reqwest::Method::PATCH,
    port,
    "/agent-roles/reviewer",
    Some(json!({ "bundles": ["web.nothing"] })),
  )
  .await;
  assert_eq!(status, 422);

  let (status, _) =
    ask(reqwest::Method::PATCH, port, "/agent-roles/nobody", Some(json!({ "label": "X" }))).await;
  assert_eq!(status, 404);

  // A role an agent holds, and a kind's own, stay.
  let (status, refused) = ask(reqwest::Method::DELETE, port, "/agent-roles/reviewer", None).await;
  assert_eq!((status, refused["code"].as_str()), (409, Some("conflict")), "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("checker"), "{refused}");

  let (status, _) = ask(reqwest::Method::DELETE, port, "/agent-roles/chat", None).await;
  assert_eq!(status, 409);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/agent-roles/nobody", None).await;
  assert_eq!(status, 404);

  // Once no agent holds it, it goes with its key's assignment.
  let (status, _) = ask(reqwest::Method::DELETE, port, "/agents/checker", None).await;
  assert_eq!(status, 204);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/agent-roles/reviewer", None).await;
  assert_eq!(status, 204);
  assert_eq!(assigned(&scratch, "role:reviewer").await, 0);

  let (_, listing) = ask(reqwest::Method::GET, port, "/agent-roles", None).await;
  assert!(role(&listing, "reviewer").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_co_worker_tier_s_limit_is_set_on_the_pool() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), TOKEN);
  let port = server.port;

  let (status, set) = ask(
    reqwest::Method::PATCH,
    port,
    "/pool/limits",
    Some(json!({ "provider": "claude-code", "tier": "Light", "capacity": 3 })),
  )
  .await;
  assert_eq!(status, 200, "{set}");
  assert_eq!(set, json!({ "provider": "claude-code", "tier": "Light", "capacity": 3 }));
  assert_eq!(
    scratch
      .count(
        r#"SELECT "capacity" FROM "coworker_pool_tier_limits" WHERE "provider" = 'claude-code' AND "tier" = 'Light'"#,
        vec![]
      )
      .await,
    3
  );

  for body in [
    json!({ "provider": "nowhere", "tier": "Light", "capacity": 3 }),
    json!({ "provider": "claude-code", "tier": "Frontier", "capacity": 3 }),
    json!({ "provider": "claude-code", "tier": "Light", "capacity": 500 }),
    json!({ "provider": "claude-code", "tier": "Light" }),
  ] {
    let (status, refused) = ask(reqwest::Method::PATCH, port, "/pool/limits", Some(body.clone())).await;

    assert_eq!(
      (status, refused["code"].as_str()),
      (422, Some("unprocessable-entity")),
      "{body}: {refused}"
    );
  }
}
