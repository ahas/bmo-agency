//! The Claude logins as the job unit asks after them, of a running server: whether a user holds a login that
//! is on, the record homes each once, and the rest a done job's usage calls for begun on its login where a
//! window stands at its threshold and nothing begun where none does.

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

async fn patch(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .patch(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_a_user_s_login_the_record_homes_and_a_job_s_usage_rest() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "claude_accounts"
           ("id", "name", "place", "enabled", "credential_kind", "credential_path", "added_at", "user_id")
         VALUES ('login-a', 'one', 1, 1, 'directory', '/logins/one', 0, 7),
                ('login-b', 'two', 2, 0, 'directory', '/logins/two', 0, 8),
                ('login-c', 'three', 3, 1, 'directory', '/logins/one', 0, NULL)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // A user holds a login only where one of their own is on.
  let held = |accounts: &serde_json::Value, id: &str| {
    accounts["items"]
      .as_array()
      .unwrap()
      .iter()
      .find(|account| account["id"] == id)
      .map(|account| account["held"].as_bool().unwrap())
      .unwrap_or(false)
  };
  let (status, seven) = get(port, token, "/accounts?userName=7").await;
  assert_eq!(status, 200);
  assert!(held(&seven, "login-a"));
  let (_, eight) = get(port, token, "/accounts?userName=8").await;
  assert!(!held(&eight, "login-b"));
  let (_, nine) = get(port, token, "/accounts?userName=9").await;
  assert!(
    nine["items"]
      .as_array()
      .unwrap()
      .iter()
      .all(|account| !account["held"].as_bool().unwrap())
  );
  let (_, someone) = get(port, token, "/accounts?userName=someone").await;
  assert!(someone["items"].as_array().unwrap().is_empty());

  // Every login, each with the home its records are kept under.
  let (status, accounts) = get(port, token, "/accounts").await;
  let homes: Vec<&str> = accounts["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|one| one["home"].as_str().unwrap())
    .collect();

  assert_eq!(status, 200);
  assert_eq!(homes.len(), 3, "{homes:?}");
  assert_eq!(&homes[1..], ["/logins/two", "/logins/one"]);

  // A session window at its threshold rests its login until it resets; one below rests nothing.
  let until = 4_102_444_800_i64;
  let full = json!({ "usage": { "utilization": 1.0, "resetsAt": until, "weeklyUtilization": 0.2, "weeklyResetsAt": null } });
  let light = json!({ "usage": { "utilization": 0.4, "resetsAt": until, "weeklyUtilization": null, "weeklyResetsAt": null } });

  assert_eq!(patch(port, token, "/accounts/login-a", full).await.0, 200);
  assert_eq!(patch(port, token, "/accounts/login-c", light).await.0, 200);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "settings" WHERE "key" = 'rest_until:login-a' AND "value"::numeric = $1"#,
        vec![until.into()],
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "settings" WHERE "key" = 'rest_until:login-c'"#, vec![])
      .await,
    0
  );
  assert_eq!(patch(port, token, "/accounts/login-a", json!({ "usage": {} })).await.0, 400);
}
