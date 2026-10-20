//! An event a verb's own process published reaches the running server's streams: `bmo-agency agent
//! --remove` takes a worker the owner added away in its own process, and the `worker-removed` its bus
//! published there is posted to the server before the verb ends and read off the server's event stream.

use std::process::Command;
use std::time::Duration;

use crate::scratch::Scratch;
use crate::{binary, standing};

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_the_verb_removed_is_read_off_the_servers_event_stream() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "builtin", "cwd") VALUES ('removable', 'worker', 0, $1)"#,
      vec![home.path().join("removable").to_string_lossy().into_owned().into()],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let mut stream = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{}/events?kinds=worker.deleted", server.port))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  assert!(stream.status().is_success());

  let verb = binary::built("agency-server", "bmo-agency");
  let (address, port, data_root, home_path) =
    (scratch.address(), server.port.to_string(), data.clone(), home.path().to_path_buf());
  let removed = tokio::task::spawn_blocking(move || {
    Command::new(verb)
      .args(["agent", "--remove", "removable"])
      .current_dir(&home_path)
      .env("HOME", &home_path)
      .env("XDG_CONFIG_HOME", home_path.join(".config"))
      .env("BMO_DATA_DIR", &data_root)
      .env("BMO_ARCHIVE_URL", address)
      .env("AGENCY_CONTROL_PORT", port)
      .env("AGENCY_CONTROL_TOKEN", token)
      .output()
      .unwrap()
  })
  .await
  .unwrap();

  assert!(removed.status.success(), "{}", String::from_utf8_lossy(&removed.stderr));
  assert_eq!(
    String::from_utf8_lossy(&removed.stdout).trim().lines().last(),
    Some("worker removable removed")
  );

  let mut read = String::new();
  let found = tokio::time::timeout(Duration::from_secs(20), async {
    while !read.contains("removable") {
      match stream.chunk().await.unwrap() {
        Some(chunk) => read.push_str(&String::from_utf8_lossy(&chunk)),
        None => break,
      }
    }
  })
  .await;

  assert!(found.is_ok(), "the stream said: {read}");
  assert!(read.contains("event: worker.deleted"), "{read}");
  assert!(read.contains(r#""worker":"removable""#), "{read}");
}
