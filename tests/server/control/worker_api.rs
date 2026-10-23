//! The workers as resources asked of a running server (docs/api.md, 「workers, prefix-checks and
//! room-holdings」): a worker added with the documents its session's prefix carries, read back, changed
//! whole and cleared, a path that climbs refused, and a change to a field writing that field.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

async fn ask(
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

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_carries_its_prefix_documents_and_a_change_writes_what_it_names() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let documents = json!(["/umbrella/STRUCTURE.md", "docs/area.md"]);

  let (status, added) = ask(
    port,
    token,
    reqwest::Method::POST,
    "/workers",
    Some(json!({ "name": "helper", "cwd": "/work/helper", "prefixDocuments": documents })),
  )
  .await;

  assert_eq!(status, 201, "{added}");
  assert_eq!(added["prefixDocuments"], documents);
  assert_eq!(
    ask(port, token, reqwest::Method::GET, "/workers/helper", None).await.1["prefixDocuments"],
    documents
  );

  // A change names the list whole; a field it names is the field written.
  let (status, changed) = ask(
    port,
    token,
    reqwest::Method::PATCH,
    "/workers/helper",
    Some(json!({ "label": "Helper", "prefixDocuments": ["docs/other.md"] })),
  )
  .await;

  assert_eq!(status, 200, "{changed}");
  assert_eq!(changed["label"], "Helper");
  assert_eq!(changed["prefixDocuments"], json!(["docs/other.md"]));

  // A path that climbs out is refused, and the list stands.
  let (status, refused) = ask(
    port,
    token,
    reqwest::Method::PATCH,
    "/workers/helper",
    Some(json!({ "prefixDocuments": ["../elsewhere.md"] })),
  )
  .await;

  assert_eq!(status, 400, "{refused}");
  assert_eq!(
    ask(port, token, reqwest::Method::GET, "/workers/helper", None).await.1["prefixDocuments"],
    json!(["docs/other.md"])
  );

  // An empty list clears it.
  let (_, cleared) = ask(
    port,
    token,
    reqwest::Method::PATCH,
    "/workers/helper",
    Some(json!({ "prefixDocuments": [] })),
  )
  .await;

  assert_eq!(cleared["prefixDocuments"], json!([]));
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "agents" WHERE "name" = 'helper' AND "prefix_documents" IS NULL"#,
        vec![]
      )
      .await,
    1
  );
}
