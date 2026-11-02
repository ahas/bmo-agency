//! A tier's line on a running server, in the order seats were asked for and let in by who is asking now: a
//! head whose owner never asks again keeps its place but does not hold up a seat behind it whose owner asks,
//! and is next once its owner asks again; a head whose owner asks between a later seat's asks keeps its turn;
//! and a reservation's place behind a silent head is raised by the pool's passes.

use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing;

async fn call(method: reqwest::Method, port: u16, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth("secret")
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

// A seat asked for under its own key and owner, answering its token.
async fn enqueue(port: u16, key: &str, owner: &str) -> String {
  let (status, seat) = call(
    reqwest::Method::POST,
    port,
    "/seat-requests",
    json!({
      "requestKey": key, "owner": owner, "source": "synthesis", "role": "tracer",
      "selection": "Light", "tier": "Light", "provider": "claude-code", "yielding": false,
    }),
  )
  .await;

  assert_eq!(status, 201, "{seat}");
  seat["token"].as_str().unwrap().to_owned()
}

// The seat's owner asking for its admission: whether it was let in.
async fn asks(port: u16, token: &str) -> bool {
  let (status, admitted) =
    call(reqwest::Method::PATCH, port, &format!("/seat-requests/{token}"), json!({})).await;

  assert_eq!(status, 200, "{admitted}");
  admitted["request"]["state"] == json!("active")
}

async fn finished(port: u16, token: &str) {
  let (status, held) = call(
    reqwest::Method::PATCH,
    port,
    &format!("/seat-requests/{token}"),
    json!({ "state": "finished", "outcome": "done" }),
  )
  .await;

  assert_eq!(status, 200, "{held}");
}

async fn state(scratch: &Scratch, token: &str, state: &str) -> bool {
  scratch
    .count(
      "SELECT COUNT(*) AS count FROM coworker_pool_requests WHERE token = $1 AND state = $2",
      vec![token.into(), state.into()],
    )
    .await
    == 1
}

async fn limit(port: u16, capacity: i64) {
  let (status, written) = call(
    reqwest::Method::PATCH,
    port,
    "/pool/limits",
    json!({ "provider": "claude-code", "tier": "Light", "capacity": capacity }),
  )
  .await;

  assert_eq!(status, 200, "{written}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_head_whose_owner_does_not_ask_is_passed_over_and_is_next_when_it_asks_again() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), "secret");
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
         VALUES ('tracer', 'Traces a road', '', '', '', 1, 1, 0) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:tracer', 'Light')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;
  limit(port, 1).await;

  // The one place taken, and a head asked for behind it whose owner asks once and then is gone.
  let holder = enqueue(port, "voice:1:0", "voice:1").await;

  assert!(asks(port, &holder).await);

  let silent = enqueue(port, "voice:2:0", "voice:2").await;

  assert!(!asks(port, &silent).await);

  let later = enqueue(port, "voice:3:0", "voice:3").await;

  // A first ask passes no one: the seats ahead are not yet known to have gone quiet.
  assert!(!asks(port, &later).await);
  finished(port, &holder).await;

  // The head's owner has not asked since the later seat last did, so the later seat is let in, and the head
  // keeps its place in the line.
  assert!(asks(port, &later).await);
  assert!(state(&scratch, &silent, "waiting").await);

  // The head's owner asks again while the place is taken, and a seat asked for after it asks too.
  assert!(!asks(port, &silent).await);

  let next = enqueue(port, "voice:4:0", "voice:4").await;

  assert!(!asks(port, &next).await);

  // The head's owner asking between the later seat's asks keeps its turn: once the place is free the later
  // seat still waits, and the head is let in as it asks.
  assert!(!asks(port, &silent).await);
  finished(port, &later).await;
  assert!(!asks(port, &next).await);
  assert!(asks(port, &silent).await);
  assert!(state(&scratch, &next, "waiting").await);

  // A reservation's place behind a head whose owner has gone quiet: the pool's first pass over the bucket
  // is its first ask and passes no one; the next lets it into the place the head's lease gave back.
  finished(port, &silent).await;

  let (status, held) = call(
    reqwest::Method::POST,
    port,
    "/reservations",
    json!({
      "reservationKey": "research:1:seats", "owner": "research:1", "source": "search", "role": "tracer",
      "selection": "Light", "tier": "Light", "provider": "claude-code", "count": 1,
    }),
  )
  .await;

  assert_eq!(status, 201, "{held}");

  let reserved = || {
    scratch.count(
      "SELECT COUNT(*) AS count FROM coworker_pool_requests WHERE owner = 'research:1' AND state = 'reserved'",
      vec![],
    )
  };

  limit(port, 1).await;
  limit(port, 1).await;
  assert_eq!(reserved().await, 1);
  assert!(state(&scratch, &next, "waiting").await);
}
