//! The owner's warming switch: pings go on a store that holds no switch, stop while it stands, and go again
//! once it is thrown back; and the control route reads and throws it as docs/api.md states, refusing a body
//! whose `stopped` is not a boolean.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::{Front, router};
use agency_server::keepalive::switch;
use agency_server::places::Ground;
use harness_providers::Machine;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::scratch::Scratch;

const TOKEN: &str = "secret";

#[tokio::test]
async fn pings_go_unless_the_switch_stands() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  assert!(switch::pings_go(db).await.unwrap(), "a store with no switch warms");

  switch::set_stopped(db, true).await.unwrap();
  switch::set_stopped(db, true).await.unwrap();

  assert!(switch::stopped(db).await.unwrap());
  assert!(!switch::pings_go(db).await.unwrap());
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'keepalive_stopped' AND "value" = '1'"#,
        vec![]
      )
      .await,
    1,
    "the switch stands in the setting docs/store.md names"
  );

  switch::set_stopped(db, false).await.unwrap();

  assert!(switch::pings_go(db).await.unwrap());
}

// The server's control front over a scratch store, served on a loopback port of its own.
async fn front(scratch: &Scratch, home: &std::path::Path) -> u16 {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let machine =
    Machine::with_env(vec![("HOME".to_owned(), home.to_string_lossy().into_owned())], home.join("data"));
  let ground = Arc::new(Ground::new(&service, &machine, None));
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = router(Front::new(TOKEN.to_owned(), ground).unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  port
}

async fn ask(port: u16, body: Option<Value>) -> (u16, Value) {
  let client = reqwest::Client::new();
  let url = format!("http://127.0.0.1:{port}/settings");
  let request = match body {
    Some(body) => client.patch(url).json(&body),
    None => client.get(url),
  };
  let answer = request.bearer_auth(TOKEN).send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_control_route_reads_and_throws_the_switch() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let port = front(&scratch, home.path()).await;

  let settings = |stopped: bool| json!({ "keepaliveStopped": stopped, "downgradeReturn": null });
  assert_eq!(ask(port, None).await, (200, settings(false)));
  assert_eq!(ask(port, Some(json!({ "keepaliveStopped": true }))).await, (200, settings(true)));
  assert_eq!(ask(port, None).await, (200, settings(true)));
  assert!(
    !switch::pings_go(scratch.db()).await.unwrap(),
    "the route throws the switch the pool reads"
  );

  let (status, refused) = ask(port, Some(json!({ "keepaliveStopped": "yes" }))).await;

  assert_eq!(status, 400);
  assert_eq!(refused["code"], "bad-request");
  assert_eq!(ask(port, None).await, (200, settings(true)), "a refused body changes nothing");
  assert_eq!(ask(port, Some(json!({ "keepaliveStopped": false }))).await, (200, settings(false)));
}
