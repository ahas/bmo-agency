//! The DeepSeek top-up's checks (docs/tools.md, 「The DeepSeek top-up」): a call naming no amount, a setting naming
//! no card or login secret, a secret lacking a field, an amount over the cap, a balance in a currency the card road
//! does not serve and a browser another call holds are refused or answered before any browser starts; each secret
//! is read from the one its setting names at the call; no secret value reaches an answer. Agency is a stand-in on
//! the loopback that answers its settings and the balance and records every request, and the platform's address
//! is one nothing listens on, so a test here reaches no network beyond the stand-in and pays nothing.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::{Json, Router};
use bmo_agency_client::AgencyClient;
use deepseek_top_up_tool::{Payer, TopUpInput, answer, top_up_with};
use serde_json::{Value, json};
use tool_call::{Asked, Handed, PROTOCOL, Session};

pub(super) const NUMBER: &str = "4000123412341234";
pub(super) const CVC: &str = "987";
pub(super) const HOLDER: &str = "TEST HOLDER";
pub(super) const EXPIRY: &str = "12/31";
pub(super) const USERNAME: &str = "tester@example.com";
pub(super) const PASSWORD: &str = "pass-Word 42";
const SEEN_AT: i64 = 1_794_700_000;
/// The console secrets the card and the login are saved in, as the stand-in's settings name them.
pub(super) const CARD: &str = "saved-card";
pub(super) const LOGIN: &str = "saved-login";

/// What the stand-in agency answers, and every request it was asked: the method and the path. `status` is
/// the balance read's; the settings are always answered. Where `ledger` holds a balance, the balance is read from it
/// and seen at the moment it is asked, as a balance reader that has just read it.
#[derive(Clone)]
pub(super) struct Stand {
  status: StatusCode,
  balance: Option<f64>,
  currency: &'static str,
  card_secret: Option<&'static str>,
  login_secret: Option<&'static str>,
  ledger: Option<Arc<Mutex<f64>>>,
  asked: Arc<Mutex<Vec<(Method, String)>>>,
}

impl Stand {
  pub(super) fn kept(balance: Option<f64>) -> Self {
    Self {
      status: StatusCode::OK,
      balance,
      currency: "USD",
      card_secret: Some(CARD),
      login_secret: Some(LOGIN),
      ledger: None,
      asked: Arc::default(),
    }
  }

  /// A stand-in reading the balance a fake platform keeps, freshly at each ask.
  pub(super) fn reading(ledger: Arc<Mutex<f64>>) -> Self {
    Self {
      ledger: Some(ledger),
      ..Self::kept(Some(0.0))
    }
  }

  fn naming(card_secret: Option<&'static str>, login_secret: Option<&'static str>) -> Self {
    Self {
      card_secret,
      login_secret,
      ..Self::kept(Some(3.5))
    }
  }

  fn failing() -> Self {
    Self {
      status: StatusCode::INTERNAL_SERVER_ERROR,
      ..Self::kept(Some(3.5))
    }
  }

  pub(super) fn asked(&self) -> Vec<(Method, String)> {
    self.asked.lock().unwrap().clone()
  }
}

async fn route(State(stand): State<Stand>, method: Method, uri: Uri) -> (StatusCode, Json<Value>) {
  stand.asked.lock().unwrap().push((method, uri.path().to_owned()));

  if uri.path() == "/settings" {
    let settings = json!({
      "researchSupervisor": null, "downgradeReturn": null,
      "deepseekCardSecret": stand.card_secret, "deepseekLoginSecret": stand.login_secret,
    });

    return (StatusCode::OK, Json(settings));
  }

  if stand.status != StatusCode::OK {
    return (stand.status, Json(json!({ "code": "broken", "error": "the stand-in fails" })));
  }

  let (balance, seen) = match &stand.ledger {
    Some(ledger) => (Some(*ledger.lock().unwrap()), chrono::Utc::now().timestamp()),
    None => (stand.balance, SEEN_AT),
  };
  let read = balance.is_some();
  let answer = json!({
    "name": "deepseek", "code": "deepseek", "label": "DeepSeek", "enabled": true, "registered": false,
    "gateway": true, "lines": ["chat"], "reason": null, "reasonCode": null, "foldRatioDefault": 0.5,
    "quota": {
      "has": balance.is_some_and(|balance| balance > 0.0), "left": null, "window": null,
      "balance": balance, "currency": read.then_some(stand.currency),
      "balanceSeenAt": read.then_some(seen), "requests": null, "resetsAt": null,
    },
  });

  (StatusCode::OK, Json(answer))
}

/// The stand-in agency on a loopback port, answering its settings and every other route as the provider's
/// read; the server task ends with the test's runtime.
pub(super) async fn agency(stand: &Stand) -> AgencyClient {
  let app = Router::new().fallback(route).with_state(stand.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  AgencyClient::new(&format!("http://127.0.0.1:{port}"), "test-token").unwrap()
}

/// A data root whose card and login secrets hold the lines given, as the console's store writes them.
pub(super) fn data_root(
  card: Option<&[(&str, &str)]>,
  login: Option<&[(&str, &str)]>,
) -> tempfile::TempDir {
  let root = tempfile::tempdir().unwrap();
  let env = root.path().join("env");

  std::fs::create_dir_all(&env).unwrap();

  for (name, lines) in [(CARD, card), (LOGIN, login)] {
    if let Some(lines) = lines {
      let text: String = lines.iter().map(|(key, value)| format!("{key}={value}\n")).collect();

      std::fs::write(env.join(name), text).unwrap();
    }
  }

  root
}

pub(super) fn whole_card() -> Vec<(&'static str, &'static str)> {
  vec![
    ("CARD_NUMBER", NUMBER),
    ("CARD_EXPIRY", EXPIRY),
    ("CARD_CVC", CVC),
    ("CARD_HOLDER_NAME", HOLDER),
  ]
}

pub(super) fn whole_login() -> Vec<(&'static str, &'static str)> {
  vec![("USERNAME", USERNAME), ("PASSWORD", PASSWORD)]
}

fn saved_root() -> tempfile::TempDir {
  data_root(Some(&whole_card()), Some(&whole_login()))
}

pub(super) fn amount(amount: f64) -> TopUpInput {
  TopUpInput {
    amount: Some(amount),
  }
}

/// A payer whose platform nothing listens on: a call that reached the browser would fail there, not pay.
fn nowhere(root: &Path) -> Payer {
  let mut payer = Payer::of(root);

  payer.platform.address = "http://127.0.0.1:9".to_owned();
  payer.balance_wait = Duration::ZERO;
  payer
}

async fn top_up(stand: &Stand, root: &Path, input: TopUpInput) -> Result<String, String> {
  top_up_with(&agency(stand).await, root, &nowhere(root), &input)
    .await
    .map(|answered| answered.plain())
}

pub(super) fn holds_no_secret_value(said: &str) {
  if let Some(value) = secret_value_in(said) {
    panic!("a secret value stands in, {value}: {said}");
  }
}

/// The first secret value `said` holds. The card's short numbers, its last four and its code, are digits any
/// port, temporary folder or browser build an answer names may hold, so they count where they stand as a number
/// of their own outside a path or an address, as a card's ending is said.
fn secret_value_in(said: &str) -> Option<&'static str> {
  let last_four = &NUMBER[NUMBER.len() - 4..];
  let whole = [NUMBER, HOLDER, EXPIRY, USERNAME, PASSWORD]
    .into_iter()
    .find(|value| said.contains(value));
  let short = || {
    said
      .split_whitespace()
      .filter(|word| !word.contains('/'))
      .flat_map(|word| word.split(|letter: char| !letter.is_ascii_digit()))
      .find_map(|number| [last_four, CVC].into_iter().find(|value| *value == number))
  };

  whole.or_else(short)
}

#[test]
fn the_secret_check_finds_the_card_s_short_numbers_said_as_a_card_says_them_and_not_in_a_machine_s_places()
{
  let place = "A person signs the tool's browser in once with `$HOME/.cache/ms-playwright/chromium-1234/\
    chrome-linux64/chrome --user-data-dir=\"/tmp/.tmp1234ab/deepseek-top-up/profile\" \
    http://127.0.0.1:41234/sign_in`, then asks again on port 39870.";

  assert_eq!(secret_value_in(place), None);
  assert_eq!(secret_value_in("Paid 10 with the card ending 1234."), Some("1234"));
  assert_eq!(secret_value_in("The card's code 987 was refused."), Some(CVC));
}

fn reads_nothing_past_the_settings(stand: &Stand) {
  assert_eq!(
    stand.asked(),
    [(Method::GET, "/settings".to_owned())],
    "nothing past the settings is read"
  );
}

#[tokio::test]
async fn a_call_naming_no_amount_is_refused_and_reaches_nothing() {
  let stand = Stand::kept(Some(3.5));
  let root = saved_root();

  for input in [TopUpInput { amount: None }, amount(0.0), amount(-5.0), amount(f64::NAN)] {
    let refused = top_up(&stand, root.path(), input).await.unwrap_err();

    assert!(refused.starts_with("Name the amount"), "{refused}");
  }

  assert!(stand.asked().is_empty(), "a refused call asks agency nothing");
}

#[tokio::test]
async fn a_call_handed_no_arguments_or_a_card_is_refused_before_anything_is_read() {
  let root = saved_root();
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

  for wrong in [
    json!({ "amount": 10, "CARD_NUMBER": NUMBER }),
    json!({ "amount": 10, "PASSWORD": PASSWORD }),
  ] {
    let refused = answer(handed(wrong)).await.unwrap();

    assert_eq!(refused["isError"], true);
    assert_eq!(
      refused["content"][0]["text"].as_str().unwrap(),
      "ERROR: the call names amount, a number, and nothing else"
    );
  }

  let wrong = answer(handed(json!({ "amount": NUMBER }))).await.unwrap();

  holds_no_secret_value(&wrong.to_string());
}

#[tokio::test]
async fn a_setting_naming_no_secret_is_refused_with_the_setting_to_fill() {
  let root = saved_root();

  for unset in [None, Some(""), Some("  ")] {
    let card = Stand::naming(unset, Some(LOGIN));
    let refused = top_up(&card, root.path(), amount(10.0)).await.unwrap_err();

    assert!(
      refused.contains("fill agency's setting deepseekCardSecret") && refused.contains("kind card,"),
      "{refused}"
    );
    reads_nothing_past_the_settings(&card);

    let login = Stand::naming(Some(CARD), unset);
    let refused = top_up(&login, root.path(), amount(10.0)).await.unwrap_err();

    assert!(
      refused.starts_with("No DeepSeek login secret is named")
        && refused.contains("fill agency's setting deepseekLoginSecret")
        && refused.contains("kind account-login"),
      "{refused}"
    );
    reads_nothing_past_the_settings(&login);
  }
}

#[tokio::test]
async fn a_secret_standing_nowhere_is_refused_with_the_secret_to_ask_for() {
  let stand = Stand::kept(Some(3.5));

  let card = top_up(&stand, data_root(None, Some(&whole_login())).path(), amount(10.0))
    .await
    .unwrap_err();

  assert!(
    card.starts_with("No card is saved") && card.contains(CARD) && card.contains("kind card"),
    "{card}"
  );

  let login = top_up(&stand, data_root(Some(&whole_card()), None).path(), amount(10.0))
    .await
    .unwrap_err();

  assert!(
    login.starts_with("No DeepSeek login is saved")
      && login.contains(LOGIN)
      && login.contains("kind account-login"),
    "{login}"
  );
}

#[tokio::test]
async fn each_secret_is_read_from_the_one_its_setting_names_at_the_call() {
  let root = saved_root();

  let elsewhere = top_up(&Stand::naming(Some("other-card"), Some(LOGIN)), root.path(), amount(10.0))
    .await
    .unwrap_err();

  assert!(
    elsewhere.starts_with("No card is saved") && elsewhere.contains("other-card"),
    "{elsewhere}"
  );

  let elsewhere = top_up(&Stand::naming(Some(CARD), Some("other-login")), root.path(), amount(10.0))
    .await
    .unwrap_err();

  assert!(
    elsewhere.starts_with("No DeepSeek login is saved") && elsewhere.contains("other-login"),
    "{elsewhere}"
  );
}

#[tokio::test]
async fn a_secret_lacking_its_fields_is_refused_naming_the_keys_alone() {
  let stand = Stand::kept(Some(3.5));
  let card = [("CARD_NUMBER", NUMBER), ("CARD_CVC", " "), ("CARD_EXPIRY", EXPIRY)];
  let root = data_root(Some(&card), Some(&whole_login()));

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.contains("lacks CARD_CVC, CARD_HOLDER_NAME:"), "{refused}");
  holds_no_secret_value(&refused);
  reads_nothing_past_the_settings(&stand);

  let stand = Stand::kept(Some(3.5));
  let root = data_root(Some(&whole_card()), Some(&[("USERNAME", USERNAME)]));

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(
    refused.starts_with("The DeepSeek login saved as saved-login lacks PASSWORD:"),
    "{refused}"
  );
  holds_no_secret_value(&refused);
  reads_nothing_past_the_settings(&stand);
}

#[tokio::test]
async fn an_amount_over_the_cap_is_refused_with_the_cap_and_its_reason() {
  let stand = Stand::kept(Some(3.5));
  let root = saved_root();

  let refused = top_up(&stand, root.path(), amount(50.01)).await.unwrap_err();

  assert!(refused.starts_with("50.01 USD is over the single top-up cap of 50 USD: "), "{refused}");
  assert!(refused.contains("a mistaken amount or a repeated call"), "{refused}");
  holds_no_secret_value(&refused);
}

#[tokio::test]
async fn a_balance_never_read_leaves_the_currency_unknown_and_is_refused() {
  let stand = Stand::kept(None);
  let root = saved_root();

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.starts_with("The DeepSeek balance has not been read yet"), "{refused}");
}

#[tokio::test]
async fn an_agency_that_cannot_answer_the_balance_refuses_the_call() {
  let stand = Stand::failing();
  let root = saved_root();

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(refused.starts_with("The DeepSeek balance could not be read from agency:"), "{refused}");
}

#[tokio::test]
async fn a_balance_kept_in_yuan_is_paid_by_no_card_and_says_what_a_person_does() {
  let stand = Stand {
    currency: "CNY",
    ..Stand::kept(Some(3.5))
  };
  let root = saved_root();

  let said = top_up(&stand, root.path(), amount(100.0)).await.unwrap();

  assert!(
    said.starts_with("Nothing was paid: DeepSeek takes no card for a balance kept in CNY"),
    "{said}"
  );
  assert!(said.contains("https://platform.deepseek.com/top_up"), "{said}");
  assert!(
    said.ends_with(
      "Paid: 0 CNY. Balance: 3.5 CNY, as the balance reader last read it at 2026-11-14T23:46:40Z."
    ),
    "{said}"
  );
  assert!(!root.path().join("deepseek-top-up").exists(), "no browser was started");
}

#[tokio::test]
async fn a_browser_another_call_holds_refuses_the_call_and_pays_nothing() {
  let stand = Stand::kept(Some(3.5));
  let root = saved_root();
  let folder = root.path().join("deepseek-top-up");

  std::fs::create_dir_all(&folder).unwrap();

  let held = std::fs::File::create(folder.join("browser.lock")).unwrap();

  held.lock().unwrap();

  let refused = top_up(&stand, root.path(), amount(10.0)).await.unwrap_err();

  assert!(
    refused.starts_with("Nothing was paid: another DeepSeek top-up is under way"),
    "{refused}"
  );
  assert!(!folder.join("profile").exists(), "no browser was started");
}

#[tokio::test]
async fn no_refused_call_moves_the_balance_or_asks_agency_anything_but_its_reads() {
  let stand = Stand::kept(Some(3.5));
  let root = saved_root();

  for input in [amount(500.0), TopUpInput { amount: None }] {
    let _ = top_up(&stand, root.path(), input).await;
  }

  let failing = Stand::failing();
  let _ = top_up(&failing, root.path(), amount(10.0)).await;

  for (method, path) in stand.asked().into_iter().chain(failing.asked()) {
    assert_eq!(method, Method::GET);
    assert!(["/settings", "/providers/deepseek"].contains(&path.as_str()), "{path}");
  }
}
