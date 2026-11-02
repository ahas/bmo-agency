//! A job asked of a running server in the shapes the harness sends (docs/api.md, 「assignments and jobs」): a
//! launch refused while nothing in the order can answer, the assignment left held; a turn that
//! fails ending the job with why, written into its log; and the end releasing the
//! assignment, so the worker is held again for the next item.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::support::grounds::{OrderEntry, set_provider_order};
use axum::http::StatusCode;
use harness_providers::ARCHIVE_PROVIDERS;
use serde_json::{Value, json};

use crate::support::rows::repository;
use crate::support::scratch::Scratch;
use crate::support::standing;

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

fn item() -> Value {
  json!({ "title": "A harmless item", "brief": "Answer that nothing is to be done.", "notes": [] })
}

async fn assigned(port: u16, token: &str, item_id: i64) -> i64 {
  let (status, assignment) = post(
    port,
    token,
    "/assignments",
    json!({ "itemId": item_id, "holderName": "harness", "assigneeName": "helper", "item": item() }),
  )
  .await;

  assert_eq!(status, 201, "{assignment}");
  assignment["id"].as_i64().unwrap()
}

// A tool host on the socket given, issuing one token to every record and taking every token back: a run's
// program waits for its token until the host answers.
fn tool_host(socket: &std::path::Path) {
  let listener = tokio::net::UnixListener::bind(socket).unwrap();
  let app = axum::Router::new()
    .route(
      "/tool-sessions",
      axum::routing::post(|| async { (StatusCode::CREATED, axum::Json(json!({ "token": "job-token" }))) }),
    )
    .fallback(|| async { StatusCode::NO_CONTENT });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
}

async fn run_job(port: u16, token: &str, assignment: i64, item_id: i64) -> (u16, Value) {
  post(
    port,
    token,
    "/jobs",
    json!({
      "assignmentId": assignment,
      "itemId": item_id,
      "leaseId": 1,
      "holderName": "harness",
      "item": item(),
    }),
  )
  .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_waits_for_a_login_ends_with_why_and_frees_its_worker() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let login = home.path().join("login-a");
  let host = home.path().join("mcp-host.sock");
  let token = "secret";
  let resting = chrono::Utc::now().timestamp() + 3600;

  std::fs::create_dir_all(&data).unwrap();

  // The server is handed the receiver's address and token as a test names them, as a worker's environment
  // would have handed it a live one. A test ground still makes no call: a job ending here must not post a
  // real job-ended instruction into a room.
  let heard: Arc<AtomicUsize> = Arc::default();
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let leaked_address = format!("http://{}", listener.local_addr().unwrap());
  let counted = heard.clone();

  tokio::spawn(async move {
    let routes = axum::Router::new().fallback(move || {
      let counted = counted.clone();

      async move {
        counted.fetch_add(1, Ordering::SeqCst);

        StatusCode::NO_CONTENT
      }
    });

    axum::serve(listener, routes).await.unwrap();
  });

  std::fs::create_dir_all(&login).unwrap();
  std::fs::create_dir_all(home.path().join("helper")).unwrap();
  tool_host(&host);
  repository(scratch.db(), "helper", &home.path().join("helper").to_string_lossy(), &[]).await;
  scratch.run(r#"DELETE FROM "claude_accounts""#, vec![]).await;
  scratch
    .run(
      r#"INSERT INTO "claude_accounts" ("id", "name", "place", "enabled", "credential_kind", "credential_path", "added_at")
         VALUES ('login-a', 'login-a', 1, 1, 'directory', $1, 0)"#,
      vec![login.to_string_lossy().into_owned().into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until:login-a', $1)"#,
      vec![resting.to_string().into()],
    )
    .await;
  // Claude Code is the one provider of the order the owner has on, so its logins are the whole order.
  set_provider_order(
    scratch.db(),
    &ARCHIVE_PROVIDERS
      .iter()
      .map(|name| OrderEntry {
        name: (*name).to_owned(),
        enabled: *name == "claude-code",
      })
      .collect::<Vec<_>>(),
  )
  .await
  .unwrap();

  let server = standing::stand_server_with(
    home.path(),
    &data,
    &scratch.address(),
    token,
    &[
      ("BMO_RECEIVER_URL", leaked_address.as_str()),
      ("BMO_RECEIVER_TOKEN", "leaked"),
      ("BMO_MCP_HOST_SOCKET", &host.to_string_lossy()),
      ("BMO_MCP_HOST_ISSUER_TOKEN", "issuer"),
    ],
  );
  let port = server.port;

  // An item naming a role the store does not hold is refused, and nothing is held for it.
  let (status, refused) = post(
    port,
    token,
    "/assignments",
    json!({ "itemId": 40, "holderName": "harness", "assigneeName": "helper", "roleName": "nobody", "item": item() }),
  )
  .await;

  assert_eq!(status, 422, "{refused}");
  assert_eq!(refused["code"], "unknown-role", "{refused}");

  // Nothing in the order can answer: nothing is launched, the worker stays held for the item, and the
  // refusal names every provider's reason and the moment the first login reopens.
  let assignment = assigned(port, token, 41).await;
  let (status, refused) = run_job(port, token, assignment, 41).await;

  assert_eq!(status, 503, "{refused}");
  assert_eq!(refused["code"], "no-login-open", "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("login-a is at its limit"), "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("reopens on its own"), "{refused}");
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "jobs""#, vec![]).await, 0);
  assert_eq!(get(port, token, &format!("/assignments/{assignment}")).await.1["state"], "held");

  // The login reopens; the launch goes out and its turn, with no herdr to stand it in, fails: the job ends
  // saying why, in its report and in its log.
  scratch
    .run(r#"DELETE FROM "settings" WHERE "key" = 'rest_until:login-a'"#, vec![])
    .await;

  let (status, job) = run_job(port, token, assignment, 41).await;

  assert_eq!(status, 201, "{job}");

  let id = job["id"].as_i64().unwrap();
  let mut ended = Value::Null;

  for _ in 0..100 {
    ended = get(port, token, &format!("/jobs/{id}")).await.1;

    if ended["state"] != "starting" && ended["state"] != "running" {
      break;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  assert_eq!(ended["state"], "ended", "{ended}");
  assert_eq!(ended["outcome"], "failed", "{ended}");
  assert!(!ended["report"].as_str().unwrap_or_default().is_empty(), "{ended}");
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "agent_log" WHERE "job_id" = $1 AND "kind" = 'note' AND "text" LIKE 'The job ended failed:%'"#,
        vec![id.into()],
      )
      .await,
    1
  );

  // The turn was a run of the ledger under the job's run name, closed as it failed, with nothing spent.
  let run = ended["runId"].as_i64().expect("the job carries its run");
  let (_, kept) = get(port, token, &format!("/runs/{run}")).await;

  assert_eq!(kept["unitName"], "job-1", "{kept}");
  assert_eq!(kept["state"], "closed", "{kept}");
  assert_eq!(kept["outcome"], "failed", "{kept}");
  assert_eq!(ended["usage"], Value::Null, "{ended}");
  assert_eq!(ended["tokenCount"], 0, "{ended}");

  // The job ran as its assignment's role, `worker` where the item named none, holding a slot of that role
  // keyed by its checkout, which its end gave back.
  assert_eq!(ended["roleName"], "worker", "{ended}");
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "role_slot_requests" WHERE "role" = 'worker' AND "source" = 'job'
             AND "place_key" = 'helper' AND "owner" = $1 AND "state" = 'released'"#,
        vec![format!("job-{id}").into()],
      )
      .await,
    1
  );

  // The end released the assignment, so the same worker is held for the next item.
  assert_eq!(get(port, token, &format!("/assignments/{assignment}")).await.1["state"], "released");
  assigned(port, token, 42).await;

  assert_eq!(heard.load(Ordering::SeqCst), 0, "the test server posted to the leaked receiver");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_assignment_whose_job_ended_is_released_as_the_assignments_are_read() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  repository(scratch.db(), "helper", &home.path().join("helper").to_string_lossy(), &[]).await;
  scratch
    .run(
      r#"INSERT INTO "assignments" ("id", "item_id", "holder_name", "worker_name", "state", "created_at", "changed_at")
         VALUES (7, 41, 'harness', 'helper', 'running', 0, 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
              "outcome", "token_count", "input_tokens", "output_tokens", "cache_read_tokens",
              "cache_write_tokens", "started_at", "ended_at", "prompt")
         VALUES (41, 1, 'harness', 7, 'helper', 'ended', 'failed', 1111, 1, 10, 1000, 100, 0, 1, '# A harmless item')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  assert_eq!(get(port, token, "/assignments/7").await.1["state"], "released");

  // The job answers what its turn spent, apart and together.
  let (_, jobs) = get(port, token, "/jobs").await;
  let job = &jobs["items"][0];

  assert_eq!(
    job["usage"],
    json!({ "inputTokens": 1, "outputTokens": 10, "cacheReadTokens": 1000, "cacheWriteTokens": 100 })
  );
  assert_eq!(job["tokenCount"], 1111);
  assigned(port, token, 42).await;
}

// A job's small-model instruction (docs/worker-bench.md, 「A small-model instruction」): refused naming the
// field it lacks, kept and answered whole, rendered under the item's title ahead of its brief, and
// composed into the one line a work-order step is written as.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_carries_its_instruction_whole_into_its_prompt_and_a_step_s_line() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";
  let instruction = json!({
    "repository": "services/area",
    "documents": ["docs/area.md"],
    "sentences": ["A refused call names the field it lacks."],
    "checks": ["cargo test --test area"],
  });

  std::fs::create_dir_all(&data).unwrap();
  repository(scratch.db(), "helper", &home.path().join("helper").to_string_lossy(), &[]).await;
  scratch.run(r#"DELETE FROM "claude_accounts""#, vec![]).await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let assignment = assigned(port, token, 43).await;
  let job = |instruction: Value| {
    json!({
      "assignmentId": assignment,
      "itemId": 43,
      "leaseId": 1,
      "holderName": "harness",
      "item": item(),
      "instruction": instruction,
    })
  };

  let mut lacking = instruction.clone();
  lacking["sentences"] = json!([]);
  let (status, refused) = post(port, token, "/jobs", job(lacking)).await;

  assert_eq!(status, 400, "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("instruction.sentences"), "{refused}");

  let (status, created) = post(port, token, "/jobs", job(instruction.clone())).await;

  assert_eq!(status, 201, "{created}");
  assert_eq!(created["instruction"], instruction);

  let id = created["id"].as_i64().unwrap();
  let prompt = get(port, token, &format!("/jobs/{id}/prompt")).await.1["text"]
    .as_str()
    .unwrap()
    .to_owned();
  let rendered = prompt.find("## Instruction").expect("the prompt renders the instruction");

  assert!(prompt.starts_with("# A harmless item\n\n## Instruction"), "{prompt}");
  assert!(rendered < prompt.find("Answer that nothing is to be done.").unwrap(), "{prompt}");
  assert!(prompt.contains("Repository: `services/area`"), "{prompt}");
  assert!(prompt.contains("- docs/area.md\n"), "{prompt}");
  assert!(prompt.contains("- A refused call names the field it lacks.\n"), "{prompt}");
  assert!(prompt.contains("- `cargo test --test area`\n"), "{prompt}");

  let (status, text) = post(port, token, "/instruction-texts", instruction).await;

  assert_eq!(status, 200, "{text}");
  assert_eq!(
    text["text"],
    "In `services/area`, reading docs/area.md: A refused call names the field it lacks. Checked by \
     `cargo test --test area`."
  );
}
