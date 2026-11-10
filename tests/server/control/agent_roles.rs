//! The roles and their agents, asked of a running server (docs/api.md, 「roles」; docs/role-agents.md): the
//! seeded roles with their agents and prompts, a role added, changed and removed, its agents added, changed
//! and removed, and each refusal with nothing written.

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
  assert_eq!(listing["roles"].as_array().unwrap().len(), 14, "{listing}");
  assert_eq!(role(&listing, "doppelganger").unwrap()["maxConcurrent"], 2);
  // The double's agents are seeded on no road: its road follows the supervisor that asks it, which a screen
  // shows.
  let doubles = role(&listing, "doppelganger").unwrap()["agents"].as_array().unwrap().clone();

  assert_eq!(doubles.len(), 2, "{doubles:?}");
  assert!(
    doubles.iter().all(|agent| agent["provider"].is_null() && agent["enabled"] == true),
    "{doubles:?}"
  );
  assert_eq!(role(&listing, "doppelganger").unwrap()["followsSupervisor"], true);
  assert_eq!(role(&listing, "doppelganger").unwrap()["provider"], Value::Null);
  assert_eq!(role(&listing, "doppelganger").unwrap()["waitingFor"], Value::Null);
  assert_eq!(role(&listing, "worker").unwrap()["followsSupervisor"], false);
  assert_eq!(role(&listing, "worker").unwrap()["maxConcurrent"], 3);
  // A seeded role's agents are as many as its declared count, each on the road it declares, in their order.
  let workers = role(&listing, "worker").unwrap()["agents"].as_array().unwrap().clone();

  assert_eq!(
    workers
      .iter()
      .map(|agent| (agent["name"].as_str().unwrap(), agent["model"].as_str().unwrap()))
      .collect::<Vec<_>>(),
    [
      ("worker-01", "deepseek-flash"),
      ("worker-02", "deepseek-flash"),
      ("worker-03", "deepseek-flash")
    ]
  );
  // Whether a run keeps its session, the share its session is folded at and whether it may be called are
  // each agent's own, given by its role's declaration and answered on no role.
  let settings = |name: &str, field: &str| -> Vec<Value> {
    role(&listing, name).unwrap()["agents"]
      .as_array()
      .unwrap()
      .iter()
      .map(|agent| agent[field].clone())
      .collect()
  };

  assert!(settings("scout", "callable").iter().all(|callable| callable == true));
  assert!(settings("producer", "callable").iter().all(|callable| callable == false));
  assert!(settings("worker", "sessions").iter().all(|sessions| sessions == "keep"));
  assert!(settings("worker", "foldRatio").iter().all(Value::is_number));
  for field in ["sessions", "foldRatio", "callable"] {
    assert_eq!(role(&listing, "worker").unwrap().get(field), None, "{field}");
  }

  // The coder is seeded beside the worker, on the worker line, with the same bundles and as many agents, each
  // on no road: its road follows the supervisor that asks it until the owner sets an agent one.
  let coder = role(&listing, "coder").unwrap();

  assert_eq!((&coder["line"], &coder["maxConcurrent"]), (&json!("worker"), &json!(3)), "{coder}");
  assert_eq!(
    coder["bundles"]
      .as_array()
      .unwrap()
      .iter()
      .map(|bundle| bundle["name"].as_str().unwrap())
      .collect::<Vec<_>>(),
    ["graph.read", "queue.read", "web.read", "brain.lessons"],
    "{coder}"
  );
  assert_eq!(coder["followsSupervisor"], true, "{coder}");

  let coder_prompt =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/coder.md")).unwrap();
  assert_eq!(coder["prompt"], coder_prompt);

  let translator =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/translator.md")).unwrap();
  assert_eq!(role(&listing, "translator").unwrap()["prompt"], translator);

  // Each refusal is 422 with one sentence, and nothing is written: a role names no order and no cap.
  for body in [
    json!({ "name": "Not_Kebab", "label": "X", "bundles": [], "line": "worker" }),
    json!({ "name": "producer", "label": "X", "bundles": [], "line": "worker" }),
    json!({ "name": "reviewer", "label": "", "bundles": [], "line": "worker" }),
    json!({ "name": "reviewer", "label": "Reviewer", "line": "worker" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": ["web.everything"], "line": "worker" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "nowhere" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "sessions": "keep" }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "foldRatio": 0.5 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "callable": true }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker", "maxConcurrent": 1 }),
    json!({ "name": "reviewer", "label": "Reviewer", "bundles": [], "line": "worker",
      "order": [{ "provider": "claude-code", "model": "claude-sonnet-4-6" }] }),
  ] {
    let (status, refused) = ask(reqwest::Method::POST, port, "/roles", Some(body.clone())).await;

    assert_eq!(status, 422, "{body}: {refused}");
    assert!(refused["error"].as_str().unwrap().ends_with('.'), "{refused}");
  }
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "roles""#, vec![]).await, 14);

  // A role added, its bundles once each: it holds no agent yet, so it carries no run.
  let (status, added) = ask(
    reqwest::Method::POST,
    port,
    "/roles",
    Some(json!({
      "name": "reviewer",
      "label": "Reviewer",
      "bundles": ["web.read", "queue.read", "web.read"],
      "line": "worker",
      "prompt": "Review what you are handed.",
    })),
  )
  .await;
  assert_eq!(status, 201, "{added}");
  assert_eq!(added["bundles"].as_array().unwrap().len(), 2);
  assert_eq!((&added["maxConcurrent"], &added["agents"]), (&json!(0), &json!([])));

  // A change names only what it changes.
  let (status, changed) = ask(
    reqwest::Method::PATCH,
    port,
    "/roles/reviewer",
    Some(json!({ "label": "Checker of work", "prompt": null })),
  )
  .await;
  assert_eq!(status, 200, "{changed}");
  assert_eq!(changed["label"], "Checker of work");
  assert_eq!(changed["bundles"].as_array().unwrap().len(), 2);
  assert_eq!(changed["prompt"], Value::Null);

  // A role names the skills its runs carry among those a plugin of the release carries, each once, and null
  // leaves its runs the whole plugin the release places for it again.
  let carried: Vec<String> = listing["skills"]
    .as_array()
    .unwrap()
    .iter()
    .map(|skill| skill.as_str().unwrap().to_owned())
    .collect();

  assert!(carried.len() >= 2, "the release carries skills a role may name: {listing}");
  assert_eq!(changed["skills"], Value::Null, "a role added with none sets none");

  let (status, skilled) = ask(
    reqwest::Method::PATCH,
    port,
    "/roles/reviewer",
    Some(json!({ "skills": [carried[1], carried[0], carried[1]] })),
  )
  .await;
  assert_eq!(status, 200, "{skilled}");
  assert_eq!(skilled["skills"], json!([carried[1], carried[0]]));
  assert_eq!(skilled["label"], "Checker of work", "a change of the skills leaves every other field");

  let (_, relabelled) = ask(
    reqwest::Method::PATCH,
    port,
    "/roles/reviewer",
    Some(json!({ "description": "Checks work." })),
  )
  .await;
  assert_eq!(
    relabelled["skills"],
    json!([carried[1], carried[0]]),
    "a change that names no skills keeps them"
  );

  let (status, cleared) =
    ask(reqwest::Method::PATCH, port, "/roles/reviewer", Some(json!({ "skills": null }))).await;
  assert_eq!((status, &cleared["skills"]), (200, &Value::Null), "{cleared}");

  for body in [
    json!({ "sessions": "fresh" }),
    json!({ "foldRatio": 0.5 }),
    json!({ "callable": true }),
    json!({ "bundles": ["web.nothing"] }),
    json!({ "skills": ["no-such-skill"] }),
    json!({ "skills": "clean" }),
    json!({ "skills": [3] }),
    json!({ "maxConcurrent": 5 }),
    json!({ "active": { "provider": "claude-code", "model": "claude-opus-5" } }),
    json!({ "order": [] }),
  ] {
    let (status, _) = ask(reqwest::Method::PATCH, port, "/roles/reviewer", Some(body.clone())).await;
    assert_eq!(status, 422, "{body}");
  }

  // Its agents are added one a seat: named after the role and a number where none is named, each on the road
  // it names or on none, in the order they were added.
  let (status, first) = ask(
    reqwest::Method::POST,
    port,
    "/role-agents",
    Some(json!({ "role": "reviewer", "provider": "claude-code", "model": "claude-sonnet-4-6", "effort": "high" })),
  )
  .await;
  assert_eq!(status, 201, "{first}");
  assert_eq!(
    (
      &first["name"],
      &first["position"],
      &first["enabled"],
      &first["followsSupervisor"],
      &first["carries"]
    ),
    (&json!("reviewer-01"), &json!(1), &json!(true), &json!(false), &json!(false)),
    "{first}"
  );

  let (status, second) = ask(
    reqwest::Method::POST,
    port,
    "/role-agents",
    Some(json!({ "role": "reviewer", "name": "reviewer-local", "prompt": "Review as the local one." })),
  )
  .await;
  assert_eq!(status, 201, "{second}");
  assert_eq!(
    (
      &second["provider"],
      &second["followsSupervisor"],
      &second["position"],
      &second["prompt"]
    ),
    (&Value::Null, &json!(true), &json!(2), &json!("Review as the local one.")),
    "{second}"
  );

  let (_, reviewer) = ask(reqwest::Method::GET, port, "/roles/reviewer", None).await;

  assert_eq!(reviewer["maxConcurrent"], 2, "{reviewer}");
  assert_eq!(reviewer["agents"].as_array().unwrap().len(), 2, "{reviewer}");

  // An agent switched off counts for nothing; a field given as null is cleared.
  let (status, off) = ask(
    reqwest::Method::PATCH,
    port,
    "/role-agents/reviewer-local",
    Some(json!({
      "enabled": false, "prompt": null, "keepsCache": true, "bundles": ["web.read"],
      "sessions": "fresh", "callable": true, "foldRatio": 0.5
    })),
  )
  .await;
  assert_eq!(status, 200, "{off}");
  // An agent added with none named keeps its sessions, may not be called and sets no threshold.
  assert_eq!(
    (&second["sessions"], &second["callable"], &second["foldRatio"]),
    (&json!("keep"), &json!(false), &Value::Null)
  );
  assert_eq!(
    (&off["sessions"], &off["callable"], &off["foldRatio"]),
    (&json!("fresh"), &json!(true), &json!(0.5))
  );
  assert_eq!(
    (&off["enabled"], &off["prompt"], &off["keepsCache"], &off["bundles"]),
    (&json!(false), &Value::Null, &json!(true), &json!(["web.read"])),
    "{off}"
  );

  let (_, reviewer) = ask(reqwest::Method::GET, port, "/roles/reviewer", None).await;

  assert_eq!(reviewer["maxConcurrent"], 1, "{reviewer}");

  let (_, listed) = ask(reqwest::Method::GET, port, "/role-agents?role=reviewer", None).await;

  assert_eq!(
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|agent| agent["name"].as_str().unwrap())
      .collect::<Vec<_>>(),
    ["reviewer-01", "reviewer-local"]
  );

  // Each refusal writes nothing: a role nobody holds, a name taken, a road that is half named, a provider
  // agency does not carry, a bundle the policy does not declare, a skill no plugin of the release carries.
  for (body, refused) in [
    (json!({ "role": "nobody" }), 422),
    (json!({ "role": "reviewer", "name": "reviewer-01" }), 409),
    (json!({ "role": "reviewer", "name": "Not_Kebab" }), 422),
    (json!({ "role": "reviewer", "provider": "claude-code" }), 422),
    (json!({ "role": "reviewer", "provider": "nowhere", "model": "x" }), 422),
    (json!({ "role": "reviewer", "bundles": ["web.nothing"] }), 422),
    (json!({ "role": "reviewer", "skills": ["no-such-skill"] }), 422),
    (json!({ "role": "reviewer", "sessions": "sometimes" }), 422),
    (json!({ "role": "reviewer", "callable": "yes" }), 422),
  ] {
    let (status, said) = ask(reqwest::Method::POST, port, "/role-agents", Some(body.clone())).await;

    assert_eq!(status, refused, "{body}: {said}");
  }
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "role_agents" WHERE "role" = 'reviewer'"#, vec![])
      .await,
    2
  );

  let (status, _) =
    ask(reqwest::Method::PATCH, port, "/role-agents/nobody", Some(json!({ "enabled": true }))).await;
  assert_eq!(status, 404);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/role-agents/reviewer-local", None).await;
  assert_eq!(status, 204);

  let (status, _) = ask(reqwest::Method::DELETE, port, "/role-agents/reviewer-local", None).await;
  assert_eq!(status, 404);

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
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "role_agents" WHERE "role" = 'reviewer'"#, vec![])
      .await,
    0,
    "a role removed takes its agents with it"
  );
}
