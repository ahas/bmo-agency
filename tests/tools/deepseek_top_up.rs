//! The DeepSeek top-up (docs/tools.md, 「The DeepSeek top-up」): a call naming no amount, a card lacking a field and
//! an amount over the cap are refused; a call that passes every check pays nothing and answers what a person
//! does; no card value reaches an answer. Agency is a stand-in on the loopback that answers the balance and
//! records every request, so a test reaches no network beyond it and no payment is ever asked for.

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::{Json, Router};
use bmo_agency_client::AgencyClient;
use deepseek_top_up_tool::{TopUpInput, answer, top_up_with};
use serde_json::{Value, json};
use tool_call::{Asked, Handed, PROTOCOL, Session};

const NUMBER: &str = "4000123412341234";
const CVC: &str = "987";
const HOLDER: &str = "TEST HOLDER";
const EXPIRY: &str = "12/31";
const SEEN_AT: i64 = 1_794_700_000;

/// What the stand-in agency answers, and every request it was asked: the method and the path.
#[derive(Clone)]
struct Stand {
  status: StatusCode,
  balance: Option<f64>,
  asked: Arc<Mutex<Vec<(Method, String)>>>,
}

impl Stand {
  fn kept(balance: Option<f64>) -> Self {
    Self {
      status: StatusCode::OK,
      balance,
      asked: Arc::default(),
    }
  }

  fn failing() -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      ..Self::kept(Some(3.5))
    }
  }

  fn asked(&self) -> Vec<(Method, String)> {
    self.asked.lock().unwrap().clone()
  }
}

async fn provider(State(stand): State<Stand>, method: Method, uri: Uri) -> (StatusCode, Json<Value>) {
  stand.asked.lock().unwrap().push((method, uri.path().to_owned()));

  if stand.status != StatusCode::OK {
    return (stand.status, Json(json!({ "code": "broken", "error": "the stand-in fails" })));
  }

  let read = stand.balance.is_some();
  let answer = json!({
    "name": "deepseek", "code": "deepseek", "label": "DeepSeek", "enabled": true, "registered": false,
    "gateway": true, "lines": ["chat"], "reason": null, "reasonCode": null, "foldRatioDefault": 0.5,
    "quota": {
      "has": stand.balance.is_some_and(|balance| balance > 0.0), "left": null, "window": null,
      "balance": stand.balance, "currency": read.then_some("USD"),
      "balanceSeenAt": read.then_some(SEEN_AT), "requests": null, "resetsAt": null,
    },
  });

  (StatusCode::OK, Json(answer))
}

/// The stand-in agency on a loopback port, answering every route as the provider's read; the server task ends
/// with the test's runtime.
async fn agency(stand: &Stand) -> AgencyClient {
  let app = Router::new().fallback(provider).with_state(stand.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  AgencyClient::new(&format!("http://127.0.0.1:{port}"), "test-token").unwrap()
}

/// A data root whose card secret holds the lines given, as the console's store writes them.
fn data_root(lines: Option<&[(&str, &str)]>) -> tempfile::TempDir {
  let root = tempfile::tempdir().unwrap();
  let env = root.path().join("env");

  std::fs::create_dir_all(&env).unwrap();

  if let Some(lines) = lines {
    let text: String = lines.iter().map(|(key, value)| format!("{key}={value}\n")).collect();

    std::fs::write(env.join("deepseek-card"), text).unwrap();
  }

  root
}

fn whole_card() -> Vec<(&'static str, &'static str)> {
  vec![
    ("CARD_NUMBER", NUMBER),
    ("CARD_EXPIRY", EXPIRY),
    ("CARD_CVC", CVC),
    ("CARD_HOLDER_NAME", HOLDER),
  ]
}

fn amount(amount: f64) -> TopUpInput {
  TopUpInput {
    amount: Some(amount),
  }
}

async fn top_up(stand: &Stand, root: &Path, input: TopUpInput) -> Result<String, String> {
  top_up_with(&agency(stand).await, root, &input).await.map(|answered| answered.plain())
}

fn holds_no_card_value(said: &str) {
  for value in [NUMBER, CVC, HOLDER, EXPIRY, "1234"] {
    assert!(!said.contains(value), "a card value stands in: {said}");
  }
}

#[tokio::test]
async fn a_call_naming_no_amount_is_refused_and_reaches_nothing() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&whole_card()));

  for input in [TopUpInput { amount: None }, amount(0.0), amount(-5.0), amount(f64::NAN)] {
    let refused = top_up(&stand, root.path(), input).await.unwrap_err();

    assert!(refused.starts_with("Name the amount"), "{refused}");
  }

  assert!(stand.asked().is_empty(), "a refused call asks agency nothing");
}

#[tokio::test]
async fn a_call_handed_no_arguments_or_a_card_is_refused_before_anything_is_read() {
  let root = data_root(Some(&whole_card()));
  let handed = |arguments: Value| Handed {
    protocol: PROTOCOL,
    asked: Asked::Call {
      entry: deepseek_top_up_tool::entry(),
      arguments,
    },
    session: Session {
      variables: [("BMO_DATA_DIR".to_owned(), root.path().display().to_string())].into(),
      ..Session::default()
    },
  };

  let none = answer(handed(Value::Null)).await.unwrap();

  assert_eq!(none["isError"], true);
  assert!(none["content"][0]["text"].as_str().unwrap().contains("Name the amount"), "{none}");

  let card = answer(handed(json!({ "amount": 10, "CARD_NUMBER": NUMBER }))).await.unwrap();
  let said = card["content"][0]["text"].as_str().unwrap();

  assert_eq!(card["isError"], true);
  assert_eq!(said, "ERROR: the call names amount, a number, and nothing else");

  let wrong = answer(handed(json!({ "amount": NUMBER }))).await.unwrap();

  holds_no_card_value(&wrong.to_string());
}

#[tokio::test]
async fn a_card_standing_nowhere_is_refused_with_the_secret_to_ask_for() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(None);

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.contains("deepseek-card") && refused.contains("kind card"), "{refused}");
  assert!(stand.asked().is_empty());
}

#[tokio::test]
async fn a_card_lacking_its_fields_is_refused_naming_the_keys_alone() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&[("CARD_NUMBER", NUMBER), ("CARD_CVC", " "), ("CARD_EXPIRY", EXPIRY)]));

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.contains("lacks CARD_CVC, CARD_HOLDER_NAME:"), "{refused}");
  holds_no_card_value(&refused);
  assert!(stand.asked().is_empty());
}

#[tokio::test]
async fn an_amount_over_the_cap_is_refused_with_the_cap_and_its_reason() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&whole_card()));

  let refused = top_up(&stand, root.path(), amount(50.01)).await.unwrap_err();

  assert!(refused.starts_with("50.01 USD is over the single top-up cap of 50 USD: "), "{refused}");
  assert!(refused.contains("a mistaken amount or a repeated call"), "{refused}");
  holds_no_card_value(&refused);
}

#[tokio::test]
async fn a_balance_never_read_leaves_the_currency_unknown_and_is_refused() {
  let stand = Stand::kept(None);
  let root = data_root(Some(&whole_card()));

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.starts_with("The DeepSeek balance has not been read yet"), "{refused}");
}

#[tokio::test]
async fn an_agency_that_cannot_answer_the_balance_refuses_the_call() {
  let stand = Stand::failing();
  let root = data_root(Some(&whole_card()));

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.starts_with("The DeepSeek balance could not be read from agency:"), "{refused}");
}

#[tokio::test]
async fn a_call_passing_every_check_pays_nothing_and_says_what_a_person_does() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&whole_card()));

  let said = top_up(&stand, root.path(), amount(50.0)).await.unwrap();

  assert!(said.starts_with("Nothing was paid: DeepSeek takes no payment by machine"), "{said}");
  assert!(said.contains("https://platform.deepseek.com/top_up, adding 50 USD"), "{said}");
  assert!(
    said.ends_with(
      "Paid: 0 USD. Balance: 3.5 USD, as the balance reader last read it at 2026-11-14T23:46:40Z."
    ),
    "{said}"
  );
  holds_no_card_value(&said);
}

#[tokio::test]
async fn no_call_moves_the_balance_or_asks_agency_anything_but_its_read() {
  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&whole_card()));

  for input in [amount(10.0), amount(500.0), TopUpInput { amount: None }] {
    let _ = top_up(&stand, root.path(), input).await;
  }

  let failing = Stand::failing();
  let _ = top_up(&failing, root.path(), amount(10.0)).await;

  for (method, path) in stand.asked().into_iter().chain(failing.asked()) {
    assert_eq!((method, path.as_str()), (Method::GET, "/providers/deepseek"));
  }

  let after = top_up(&stand, root.path(), amount(1.0)).await.unwrap();

  assert!(after.contains("Balance: 3.5 USD"), "{after}");
}
