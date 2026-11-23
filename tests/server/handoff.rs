//! The handoff a supervisor's session going away is asked for (docs/sessions.md, 「A restart hands the session
//! over」), against a stand-in receiver: a turn that resumes its session asks nothing, one that replaces a
//! session asks once and waits for the answer up to the policy's ceiling, a handoff under way is waited on and
//! asked again, and a receiver that refuses the connection or never answers is let go so the fresh session
//! still opens.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agency_server::handoff::{Handed, ask, asked, replaces};
use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// A stand-in receiver answering each ask with the next status of `answers` (the last standing for every ask
/// after it), waiting `delay` first, and keeping what each ask carried.
async fn receiver(
  answers: Vec<u16>,
  delay: Duration,
) -> (BTreeMap<String, String>, Arc<Mutex<Vec<Value>>>) {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let heard = Arc::new(Mutex::new(Vec::new()));
  let kept = heard.clone();
  let app = Router::new().fallback(move |request: Request| {
    let kept = kept.clone();
    let answers = answers.clone();

    async move {
      let path = request.uri().path().to_owned();
      let bytes = axum::body::to_bytes(request.into_body(), 1 << 20).await.unwrap();
      let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
      let at = {
        let mut kept = kept.lock().unwrap();

        kept.push(json!({ "path": path, "body": body }));
        kept.len() - 1
      };

      tokio::time::sleep(delay).await;

      match answers.get(at).or(answers.last()).copied().unwrap_or(204) {
        200 => axum::Json(json!({ "path": "/handoffs/agent.md", "bytes": 1234 })).into_response(),
        status => StatusCode::from_u16(status).unwrap().into_response(),
      }
    }
  });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars = BTreeMap::from([
    ("BMO_RECEIVER_URL".to_owned(), format!("http://127.0.0.1:{port}")),
    ("BMO_RECEIVER_TOKEN".to_owned(), "secret".to_owned()),
  ]);

  (vars, heard)
}

#[test]
fn only_a_fresh_session_in_place_of_one_the_agent_had_asks() {
  assert!(!replaces(true, true, true), "a turn that resumes its session asks nothing");
  assert!(!replaces(false, false, false), "an agent that never had a session asks nothing");
  assert!(replaces(false, false, true), "a fresh session in place of a standing program asks");
  assert!(replaces(true, false, false), "a kept session gone asks");
}

#[tokio::test]
async fn a_session_replaced_asks_once_with_its_reason_and_room_and_waits_for_the_file() {
  let (vars, heard) = receiver(vec![200], Duration::from_millis(200)).await;
  let started = Instant::now();
  let handed = ask(&vars, "agent", "the reason", Some(-5), Duration::from_millis(50)).await;

  assert_eq!(
    handed,
    Handed::Written {
      path: "/handoffs/agent.md".into(),
      size: 1234
    }
  );
  assert!(started.elapsed() >= Duration::from_millis(200), "the answer was waited for");

  let heard = heard.lock().unwrap();

  assert_eq!(heard.len(), 1);
  assert_eq!(
    heard[0],
    json!({ "path": "/handoffs/agent", "body": { "reason": "the reason", "room": -5 } })
  );
}

#[tokio::test]
async fn a_handoff_under_way_is_waited_on_and_asked_again() {
  let (vars, heard) = receiver(vec![409, 409, 204], Duration::ZERO).await;
  let handed = ask(&vars, "agent", "why", None, Duration::from_millis(20)).await;

  assert_eq!(handed, Handed::Nothing);
  assert_eq!(heard.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn a_slow_answer_is_waited_for_and_a_refused_connection_is_let_go_and_said() {
  let (vars, _) = receiver(vec![204], Duration::from_millis(1_500)).await;

  assert_eq!(ask(&vars, "agent", "why", None, Duration::from_millis(20)).await, Handed::Nothing);

  // A port nothing listens on: the connection is refused, which ends the ask.
  let closed = TcpListener::bind("[::1]:0").await.unwrap();
  let port = closed.local_addr().unwrap().port();

  drop(closed);

  let vars = BTreeMap::from([
    ("BMO_RECEIVER_URL".to_owned(), format!("http://[::1]:{port}")),
    ("BMO_RECEIVER_TOKEN".to_owned(), "secret".to_owned()),
  ]);
  let handed = ask(&vars, "agent", "why", None, Duration::from_millis(20)).await;

  assert!(matches!(handed, Handed::Failed(_)), "{handed:?}");
  assert!(handed.said().contains("opens all the same"));
}

#[tokio::test]
async fn a_deployment_that_names_no_receiver_asks_nothing() {
  let handed = ask(&BTreeMap::new(), "agent", "why", None, Duration::from_millis(10)).await;

  assert_eq!(handed, Handed::NoReceiver);
}

// A receiver that takes the ask and never answers it: the ask as the policy bounds it ends at the ceiling,
// saying so, so the turn waiting on it goes on and the fresh session opens all the same.
#[tokio::test(start_paused = true)]
async fn an_ask_the_receiver_never_answers_ends_at_the_ceiling() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = Router::new().fallback(std::future::pending::<StatusCode>);

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars = BTreeMap::from([
    ("BMO_RECEIVER_URL".to_owned(), format!("http://127.0.0.1:{port}")),
    ("BMO_RECEIVER_TOKEN".to_owned(), "secret".to_owned()),
  ]);
  let handed = asked(&vars, "agent", "why", None).await;

  assert!(matches!(handed, Handed::Failed(_)), "{handed:?}");
  assert!(handed.said().contains("did not answer the handoff within 1800s"), "{handed:?}");
}
