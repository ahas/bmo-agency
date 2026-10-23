//! The Claude logins as the job unit and the console ask after them, of a running server: whether a user
//! holds a login that is on, each login's own two headline windows as the store keeps them, the record homes
//! each once, and the rest a done job's usage calls for begun on its login where a window stands at its
//! threshold and nothing begun where none does.

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
  scratch
    .run(
      r#"INSERT INTO "usage_windows" ("mode", "account", "name", "utilization", "resets_at", "seen_at")
         VALUES ('claude-code', 'login-a', 'five_hour', 0.25, 1800003600, 1800000000),
                ('claude-code', 'login-a', 'seven_day', 0.5, NULL, 1800000000),
                ('claude-code', 'login-c', 'five_hour', 0.75, NULL, 1800000000)"#,
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

  // Each login answers its own two headline windows, and null where it holds no such window.
  let of = |id: &str| {
    accounts["items"]
      .as_array()
      .unwrap()
      .iter()
      .find(|account| account["id"] == id)
      .unwrap()
      .clone()
  };
  let one = of("login-a");

  assert_eq!(
    one["session"],
    json!({ "name": "five_hour", "account": "login-a", "mode": "claude-code", "utilization": 0.25,
            "resetsAt": 1800003600, "seenAt": 1800000000, "durationMinutes": null, "limitName": null })
  );
  assert_eq!(one["week"]["name"], "seven_day");
  assert_eq!(one["week"]["utilization"], 0.5);
  assert_eq!(of("login-b")["session"], Value::Null);
  assert_eq!(of("login-c")["session"]["utilization"], 0.75);
  assert_eq!(of("login-c")["week"], Value::Null);

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
