//! The roles, asked of a running server (docs/api.md, 「roles」): the seeded roles with their caps, orders and
//! prompts, a role added, changed and removed, and each refusal with nothing written.

use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing;

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

#[tokio::test(flavor = "multi_thread")]
async fn a_role_is_added_changed_and_removed_on_the_control_port() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), TOKEN);
  let port = server.port;

  // The seeded roles stand in the store, the doppelganger among them with its cap, the coder beside the
  // worker on the worker line, and the translator's prompt its file's text.
  let (status, listing) = ask(reqwest::Method::GET, port, "/roles", None).await;
  assert_eq!(status, 200, "{listing}");
  assert_eq!(listing["roles"].as_array().unwrap().len(), 13, "{listing}");
  assert_eq!(role(&listing, "doppelganger").unwrap()["maxConcurrent"], 2);
  // The double's order is seeded empty: its road follows the supervisor that asks it, which a screen shows.
  assert_eq!(role(&listing, "doppelganger").unwrap()["order"], json!([]));
  assert_eq!(role(&listing, "doppelganger").unwrap()["followsSupervisor"], true);
  assert_eq!(role(&listing, "doppelganger").unwrap()["provider"], Value::Null);
  assert_eq!(role(&listing, "doppelganger").unwrap()["waitingFor"], Value::Null);
  assert_eq!(role(&listing, "worker").unwrap()["followsSupervisor"], false);
  assert_eq!(role(&listing, "worker").unwrap()["maxConcurrent"], 3);
  assert_eq!(role(&listing, "worker").unwrap()["order"][0]["model"], "deepseek-flash");
  assert_eq!(role(&listing, "scout").unwrap()["callable"], true);
  assert_eq!(role(&listing, "producer").unwrap()["callable"], false);

  // The coder is seeded beside the worker, on the worker line, with the same bundles and cap and no order
  // of its own: its road follows the supervisor that asks it until the owner sets one.
  let coder = role(&listing, "coder").unwrap();

  assert_eq!((&coder["line"], &coder["maxConcurrent"]), (&json!("worker"), &json!(3)), "{coder}");
  assert_eq!(
    coder["bundles"]
      .as_array()
      .unwrap()
      .iter()
      .map(|bundle| bundle["name"].as_str().unwrap())
      .collect::<Vec<_>>(),
    ["graph.read", "queue.read", "web.read"],
    "{coder}"
  );
  assert_eq!(coder["order"], json!([]), "{coder}");
  assert_eq!(coder["followsSupervisor"], true, "{coder}");
  assert_eq!(coder["sessions"], "keep", "{coder}");

  let coder_prompt =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/coder.md")).unwrap();
  assert_eq!(coder["prompt"], coder_prompt);

  let translator =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/translator.md")).unwrap();
  assert_eq!(role(&listing, "translator").unwrap()["prompt"], translator);

  let order = json!([{ "provider": "claude-code", "model": "claude-sonnet-4-6" }]);

  // Each refusal is 422 with one sentence, and nothing is written.
  for body in [
    json!({ "name": "Not_Kebab", "label": "X", "bundles": [], "line": "worker", "order": order, "maxConcurrent": 1 }),
    json!({ "name": "producer", "label": "X", "bundles": [], "line": "worker", "order": order, "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "", "bundles": [], "line": "worker", "order": order, "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "line": "worker", "order": order, "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": ["web.everything"], "line": "worker", "order": order,
      "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "order": order }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "nowhere", "order": order,
      "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "order": order,
      "maxConcurrent": 1, "sessions": "sometimes" }),
  ] {
    let (status, refused) = ask(reqwest::Method::POST, port, "/roles", Some(body.clone())).await;

    assert_eq!(status, 422, "{body}: {refused}");
    assert!(refused["error"].as_str().unwrap().ends_with('.'), "{refused}");
  }
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "roles""#, vec![]).await, 13);

  // A role added, its bundles once each.
  let (status, added) = ask(
    reqwest::Method::POST,
    port,
    "/roles",
    Some(json!({
      "name": "reviewer",
      "label": "Reviewer",
      "bundles": ["web.read", "queue.read", "web.read"],
      "line": "worker",
      "order": order,
      "maxConcurrent": 2,
      "prompt": "Review what you are handed.",
    })),
  )
  .await;
  assert_eq!(status, 201, "{added}");
  assert_eq!(added["bundles"].as_array().unwrap().len(), 2);
  assert_eq!(added["maxConcurrent"], 2);
  assert_eq!(added["order"][0]["provider"], "claude-code");
  assert_eq!(added["sessions"], "keep");

  // A change names only what it changes.
  let (status, changed) = ask(
    reqwest::Method::PATCH,
    port,
    "/roles/reviewer",
    Some(json!({ "label": "Checker of work", "maxConcurrent": 5, "prompt": null, "sessions": "fresh" })),
  )
  .await;
  assert_eq!(status, 200, "{changed}");
  assert_eq!(changed["label"], "Checker of work");
  assert_eq!(changed["bundles"].as_array().unwrap().len(), 2);
  assert_eq!((&changed["maxConcurrent"], &changed["prompt"]), (&json!(5), &Value::Null));
  assert_eq!(changed["sessions"], "fresh");
  assert_eq!(changed["active"], Value::Null, "its order chooses");

  // The owner forces its active provider and clears it again; the order stays as it was.
  let (status, forced) = ask(
    reqwest::Method::PATCH,
    port,
    "/roles/reviewer",
    Some(json!({ "active": { "provider": "claude-code", "model": "claude-opus-5" } })),
  )
  .await;
  assert_eq!(status, 200, "{forced}");
  assert_eq!(forced["active"], json!({ "provider": "claude-code", "model": "claude-opus-5" }));
  assert_eq!(forced["order"][0]["model"], "claude-sonnet-4-6");

  let (status, cleared) =
    ask(reqwest::Method::PATCH, port, "/roles/reviewer", Some(json!({ "active": null }))).await;
  assert_eq!((status, cleared["active"].clone()), (200, Value::Null), "{cleared}");

  for body in [
    json!({ "sessions": "never" }),
    json!({ "bundles": ["web.nothing"] }),
    json!({ "maxConcurrent": -1 }),
    json!({ "active": { "provider": "nowhere", "model": "x" } }),
    json!({ "active": "claude-code" }),
  ] {
    let (status, _) = ask(reqwest::Method::PATCH, port, "/roles/reviewer", Some(body.clone())).await;
    assert_eq!(status, 422, "{body}");
  }

  let (status, _) = ask(reqwest::Method::PATCH, port, "/roles/nobody", Some(json!({ "label": "X" }))).await;
  assert_eq!(status, 404);

  // A role a default names stays; an unknown one is not found.
  let (status, _) = ask(reqwest::Method::DELETE, port, "/roles/doppelganger", None).await;
  assert_eq!(status, 409);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/roles/nobody", None).await;
  assert_eq!(status, 404);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/roles/reviewer", None).await;
  assert_eq!(status, 204);

  let (_, listing) = ask(reqwest::Method::GET, port, "/roles", None).await;
  assert!(role(&listing, "reviewer").is_none());
}
