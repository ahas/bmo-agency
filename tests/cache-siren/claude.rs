//! The Claude account reader against a stand-in endpoint: an answer read into windows as the parity cases
//! record them, each account asked with its own login and its windows written under it alone, an account
//! that cannot be asked keeping what it had, an endpoint that does not answer in time a failure that leaves
//! the account unmarked, a token that cannot be read with marking the account unread so its last full
//! reading stops standing until a reading lifts the mark, and a failure said once when it starts and its
//! recovery once when it ends.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cache_siren::claude::{ClaudeAccounts, Credential, Login, Logins, MODE, windows_of};
use cache_siren::reader::Said;
use cache_siren::{Keeping, Reader, Slot, Window};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::scratch::Scratch;

// Every moment here is fixed, in epoch milliseconds; nothing reads the real clock.
const NOW_MS: i64 = 1_800_000_000_000;
const NOW: i64 = NOW_MS / 1000;

// The token the stand-in endpoint refuses as unauthorised.
const REFUSED: &str = "sk-refused";

struct Fixed(Vec<Login>);

impl Logins for Fixed {
  fn logins(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Login>, String>> + Send + '_>> {
    Box::pin(async { Ok(self.0.clone()) })
  }
}

/// An account-usage endpoint answering every token but the refused one with the same windows, and the
/// authorisation each ask carried.
async fn usage_endpoint() -> (String, Arc<Mutex<Vec<String>>>) {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let asked = Arc::new(Mutex::new(Vec::new()));
  let heard = asked.clone();
  let app = Router::new().fallback(move |request: Request| {
    let heard = heard.clone();

    async move {
      let said = request
        .headers()
        .get("authorization")
        .map(|value| value.to_str().unwrap().to_owned())
        .unwrap_or_default();

      heard.lock().unwrap().push(said.clone());

      if said == format!("Bearer {REFUSED}") {
        return StatusCode::UNAUTHORIZED.into_response();
      }

      axum::Json(json!({
        "five_hour": { "utilization": 25, "resets_at": "2026-10-07T05:00:00+00:00" },
        "seven_day": { "utilization": 50 },
      }))
      .into_response()
    }
  });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (format!("http://127.0.0.1:{port}/usage"), asked)
}

fn token_login(account: &str, file: std::path::PathBuf) -> Login {
  Login {
    account: account.into(),
    credential: Credential::Token(file),
  }
}

fn slot(account: &str) -> Slot<'_> {
  Slot {
    mode: MODE,
    account,
  }
}

fn full(name: &str) -> Window {
  Window {
    name: name.into(),
    utilization: 1.0,
    resets_at: Some(NOW + 3600),
    duration_minutes: None,
    limit_name: None,
  }
}

#[test]
fn an_answer_is_read_into_the_windows_the_parity_cases_record() {
  let cases = [
    (
      json!({
        "five_hour": { "utilization": 37.5, "resets_at": "2026-10-07T05:00:00.123456+00:00" },
        "seven_day": { "utilization": 12, "resets_at": null },
        "seven_day_opus": null,
      }),
      json!([
        { "name": "five_hour", "utilization": 0.375, "resetsAt": 1791349200, "durationMinutes": null, "limitName": null },
        { "name": "seven_day", "utilization": 0.12, "resetsAt": null, "durationMinutes": null, "limitName": null },
      ]),
    ),
    (json!({ "five_hour": { "utilization": null }, "seven_day": "none" }), json!([])),
    (json!([]), json!([])),
  ];

  for (answer, windows) in cases {
    assert_eq!(serde_json::to_value(windows_of(&answer)).unwrap(), windows, "{answer}");
  }
}

#[test]
fn a_reading_stands_six_five_minute_passes() {
  let reader = ClaudeAccounts::new("http://127.0.0.1:9/", Arc::new(Fixed(vec![])));

  assert_eq!(reader.cadence(), Duration::from_secs(300));
  assert_eq!(reader.fresh(), Duration::from_secs(1800));
}

#[tokio::test]
async fn each_account_is_read_with_its_own_login_and_one_that_cannot_be_asked_keeps_what_it_had() {
  let scratch = Scratch::new().await;
  let dir = tempfile::tempdir().unwrap();
  let token = dir.path().join("token");

  std::fs::write(&token, "  sk-token-one \n").unwrap();

  let (url, asked) = usage_endpoint().await;
  let reader = Arc::new(ClaudeAccounts::new(
    url,
    Arc::new(Fixed(vec![token_login("one", token), token_login("two", dir.path().join("gone"))])),
  ));
  let keeping = Keeping::new(scratch.db().clone(), vec![reader.clone()]);
  let mut kept = full("five_hour");

  kept.utilization = 0.9;
  keeping.replace(slot("two"), &[kept.clone()], NOW - 60).await.unwrap();
  reader.pass(&keeping, NOW_MS).await;

  assert_eq!(
    *asked.lock().unwrap(),
    ["Bearer sk-token-one"],
    "a token goes in its own account's ask alone"
  );

  let one = keeping.read(slot("one")).await.unwrap();
  let read: Vec<Value> =
    one.windows.iter().map(|kept| serde_json::to_value(&kept.window).unwrap()).collect();

  assert_eq!(
    read,
    [
      json!({ "name": "five_hour", "utilization": 0.25, "resetsAt": 1791349200, "durationMinutes": null, "limitName": null }),
      json!({ "name": "seven_day", "utilization": 0.5, "resetsAt": null, "durationMinutes": null, "limitName": null }),
    ]
  );
  assert!(one.windows.iter().all(|window| window.seen_at == NOW));
  assert_eq!(keeping.read(slot("two")).await.unwrap().windows[0].window, kept);
}

#[tokio::test]
async fn a_full_reading_the_reader_cannot_renew_for_its_token_stops_standing_until_a_reading_lifts_the_mark()
 {
  let scratch = Scratch::new().await;
  let dir = tempfile::tempdir().unwrap();
  let expired = dir.path().join(".credentials.json");
  let refused = dir.path().join("refused");

  std::fs::write(
    &expired,
    json!({ "claudeAiOauth": { "accessToken": "sk-expired", "expiresAt": NOW_MS - 1 } }).to_string(),
  )
  .unwrap();
  std::fs::write(&refused, REFUSED).unwrap();

  let (url, asked) = usage_endpoint().await;
  let logins = vec![
    Login {
      account: "main".into(),
      credential: Credential::Directory(expired.clone()),
    },
    token_login("spare", refused.clone()),
  ];
  let reader = Arc::new(ClaudeAccounts::new(url, Arc::new(Fixed(logins))));
  let keeping = Keeping::new(scratch.db().clone(), vec![reader.clone()]);

  for account in ["main", "spare"] {
    keeping.replace(slot(account), &[full("five_hour")], NOW - 60).await.unwrap();
  }

  let outcomes = reader.read_each(&keeping, NOW_MS).await.unwrap();

  assert_eq!(
    outcomes,
    [
      ("main".to_owned(), Err("token expired".to_owned())),
      ("spare".to_owned(), Err("HTTP 401".to_owned()))
    ]
  );
  assert_eq!(*asked.lock().unwrap(), [format!("Bearer {REFUSED}")], "an expired token is never sent");

  for account in ["main", "spare"] {
    let read = keeping.read(slot(account)).await.unwrap();

    assert_eq!(read.unread_at, Some(NOW), "{account}");
    assert_eq!(read.standing(NOW).count(), 0, "{account}'s full reading closes nothing once marked");
  }

  std::fs::write(
    &expired,
    json!({ "claudeAiOauth": { "accessToken": "sk-renewed", "expiresAt": NOW_MS + 3_600_000 } })
      .to_string(),
  )
  .unwrap();
  reader.pass(&keeping, NOW_MS + 1000).await;

  let main = keeping.read(slot("main")).await.unwrap();

  assert_eq!(main.unread_at, None, "a reading taken lifts the mark");
  assert_eq!(main.standing(NOW + 1).count(), 2);
  assert_eq!(keeping.unread_at("spare").await.unwrap(), Some(NOW), "a refused token stays marked");
}

/// An account-usage endpoint that answers nothing for longer than any ask here waits.
async fn silent_endpoint() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = Router::new().fallback(|| async {
    tokio::time::sleep(Duration::from_secs(30)).await;

    StatusCode::OK
  });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  format!("http://127.0.0.1:{port}/usage")
}

#[tokio::test]
async fn an_endpoint_that_does_not_answer_in_time_is_a_failure_and_the_account_keeps_what_it_had() {
  let scratch = Scratch::new().await;
  let dir = tempfile::tempdir().unwrap();
  let token = dir.path().join("token");

  std::fs::write(&token, "sk-token-one").unwrap();

  let reader = Arc::new(
    ClaudeAccounts::new(silent_endpoint().await, Arc::new(Fixed(vec![token_login("one", token)])))
      .waiting(Duration::from_millis(200)),
  );
  let keeping = Keeping::new(scratch.db().clone(), vec![reader.clone()]);
  let kept = full("five_hour");

  keeping.replace(slot("one"), std::slice::from_ref(&kept), NOW - 60).await.unwrap();

  let outcomes = reader.read_each(&keeping, NOW_MS).await.unwrap();

  assert_eq!(outcomes, [("one".to_owned(), Err("the endpoint did not answer in time".to_owned()))]);

  let read = keeping.read(slot("one")).await.unwrap();

  assert_eq!(read.windows[0].window, kept);
  assert_eq!(read.windows[0].seen_at, NOW - 60);
  assert_eq!(read.unread_at, None, "an endpoint that did not answer says nothing of the token");
}

#[test]
fn a_failure_is_said_once_with_its_reason_and_its_recovery_once() {
  let said = Said::default();
  let what = "claude usage for one";
  let refused = Err("HTTP 401".to_owned());

  assert_eq!(said.outcome(what, &Ok(())), None, "a reading taken says nothing");
  assert_eq!(
    said.outcome(what, &refused).as_deref(),
    Some("claude usage for one could not be read: HTTP 401")
  );
  assert_eq!(said.outcome(what, &refused), None, "the same failure is not said again");
  assert_eq!(
    said.outcome(what, &Err("the endpoint did not answer in time".to_owned())).as_deref(),
    Some("claude usage for one could not be read: the endpoint did not answer in time"),
    "another reason is said as it starts"
  );
  assert_eq!(said.outcome(what, &Ok(())).as_deref(), Some("claude usage for one read again"));
  assert_eq!(said.outcome(what, &Ok(())), None);
}
