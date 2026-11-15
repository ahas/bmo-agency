//! The DeepSeek top-up's payment (docs/tools.md, 「How it pays」 and 「Where a person is asked」), driven in the
//! machine's own Chrome against a fake top-up platform on the loopback: its sign-in page, a landing page, its top-up
//! form with a custom amount, the card among the ways to pay and a next step, and the card's fields in a frame
//! served from another site, as a payment processor's are. The platform pays, refuses, or waits for a confirmation
//! nobody gives, and a human check stops the sign-in; no step reaches DeepSeek, so no test spends anything.
//!
//! A payment made moves the fake's balance, which the stand-in agency reads back as a balance reader that has just
//! read it; anything else leaves it where it stood. Every call ends its browser and the browser's screen with it.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Json, State};
use axum::http::header::{COOKIE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use deepseek_top_up_tool::{Payer, top_up_with};
use serde_json::{Value, json};

use super::deepseek_top_up::{
  CVC, EXPIRY, HOLDER, NUMBER, PASSWORD, Stand, USERNAME, agency, amount, data_root, holds_no_secret_value,
  whole_card, whole_login,
};

/// What the fake platform does with a sign-in and a payment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scenario {
  Pays,
  Refuses,
  /// The card's issuer asks for a confirmation, which never comes.
  Confirms,
  /// The sign-in is met by a human check.
  HumanCheck,
}

/// The fake platform's record: its balance, each sign-in and each payment asked for, with what it carried.
#[derive(Clone)]
struct Fake {
  scenario: Scenario,
  port: u16,
  balance: Arc<Mutex<f64>>,
  sign_ins: Arc<Mutex<Vec<Value>>>,
  payments: Arc<Mutex<Vec<Value>>>,
}

const SIGN_IN: &str = r#"<!doctype html><html><body>
<input type="text" placeholder="Phone number / email address" id="who">
<input type="password" placeholder="Password" id="pass">
<div role="button" id="go" style="padding: 8px; width: 80px">Log in</div>
<div id="said"></div>
<script>
document.getElementById("go").addEventListener("click", async () => {
  const asked = await fetch("/api/sign_in", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ who: document.getElementById("who").value, pass: document.getElementById("pass").value }),
  });
  const answered = await asked.json();
  if (answered.result === "in") location.href = "/usage";
  else if (answered.result === "human") document.getElementById("said").innerText = "Let's confirm you are human";
  else document.getElementById("said").innerText = "Your password is incorrect or this account doesn't exist.";
});
</script></body></html>"#;

const USAGE: &str = "<!doctype html><html><body><h1>Usage</h1></body></html>";

/// The top-up form; `{card}` is the card frame's address on the other site.
const TOP_UP: &str = r#"<!doctype html><html><body>
<div>Amount</div>
<label id="ten"><input type="radio" name="amount" value="10"> $10</label>
<label id="custom"><input type="radio" name="amount"> Custom</label>
<input type="text" id="amount" placeholder="Enter amount" style="display: none">
<div>Payment method</div>
<label id="card"><input type="radio" name="way"> Debit or Credit Card</label>
<label id="paypal"><input type="radio" name="way"> PayPal</label>
<div role="button" id="next" style="display: none; padding: 8px; width: 80px">Next step</div>
<div id="checkout" style="display: none">
  <iframe id="frame" width="420" height="260"></iframe>
  <div role="button" id="pay" style="padding: 8px; width: 80px">Pay</div>
</div>
<div id="said"></div>
<script>
const shown = (id) => { document.getElementById(id).style.display = "block"; };
document.getElementById("custom").addEventListener("click", () => shown("amount"));
document.getElementById("card").addEventListener("click", () => shown("next"));
document.getElementById("next").addEventListener("click", () => {
  document.getElementById("frame").src = "{card}";
  shown("checkout");
});
document.getElementById("pay").addEventListener("click", () => {
  document.getElementById("frame").contentWindow.postMessage({ amount: document.getElementById("amount").value }, "*");
});
addEventListener("message", (event) => {
  document.getElementById("said").innerText =
    event.data.result === "paid" ? "Payment successful" : "Payment failed. Please try again.";
});
</script></body></html>"#;

const CARD_FRAME: &str = r#"<!doctype html><html><body>
<input autocomplete="cc-number" placeholder="Card number" id="number">
<input autocomplete="cc-exp" placeholder="MM / YY" id="expiry">
<input autocomplete="cc-csc" placeholder="CVV" id="cvc">
<input autocomplete="cc-name" placeholder="Cardholder Name (optional)" id="holder">
<div id="ask"></div>
<script>
const value = (id) => document.getElementById(id).value;
addEventListener("message", async (event) => {
  const asked = await fetch("/api/pay", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      amount: event.data.amount, number: value("number"), expiry: value("expiry"), cvc: value("cvc"), holder: value("holder"),
    }),
  });
  const answered = await asked.json();
  if (answered.result === "confirm") {
    document.getElementById("ask").innerText = "Approve this payment in your card app";
    return;
  }
  parent.postMessage({ result: answered.result }, "*");
});
</script></body></html>"#;

async fn sign_in_page() -> Html<&'static str> {
  Html(SIGN_IN)
}

async fn usage_page() -> Html<&'static str> {
  Html(USAGE)
}

async fn card_frame() -> Html<&'static str> {
  Html(CARD_FRAME)
}

async fn top_up_page(State(fake): State<Fake>, headers: HeaderMap) -> Response {
  let signed = headers
    .get_all(COOKIE)
    .iter()
    .any(|cookie| cookie.to_str().is_ok_and(|cookie| cookie.contains("signed=1")));

  if !signed {
    return (StatusCode::FOUND, [(LOCATION, "/sign_in")]).into_response();
  }

  Html(TOP_UP.replace("{card}", &format!("http://localhost:{}/card", fake.port))).into_response()
}

async fn sign_in(State(fake): State<Fake>, Json(asked): Json<Value>) -> Response {
  let right = asked["who"] == USERNAME && asked["pass"] == PASSWORD;

  fake.sign_ins.lock().unwrap().push(asked);

  if fake.scenario == Scenario::HumanCheck {
    return Json(json!({ "result": "human" })).into_response();
  }

  if !right {
    return Json(json!({ "result": "refused" })).into_response();
  }

  ([(SET_COOKIE, "signed=1; Path=/; Max-Age=3600")], Json(json!({ "result": "in" }))).into_response()
}

async fn pay(State(fake): State<Fake>, Json(asked): Json<Value>) -> Json<Value> {
  let paid = asked["amount"].as_str().and_then(|amount| amount.parse::<f64>().ok()).unwrap_or(0.0);

  fake.payments.lock().unwrap().push(asked);

  let result = match fake.scenario {
    Scenario::Pays => {
      *fake.balance.lock().unwrap() += paid;
      "paid"
    }
    Scenario::Refuses => "refused",
    Scenario::Confirms | Scenario::HumanCheck => "confirm",
  };

  Json(json!({ "result": result }))
}

/// The fake platform on a loopback port, holding 3.5 USD; its server task ends with the test's runtime.
async fn platform(scenario: Scenario) -> Fake {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let fake = Fake {
    scenario,
    port: listener.local_addr().unwrap().port(),
    balance: Arc::new(Mutex::new(3.5)),
    sign_ins: Arc::default(),
    payments: Arc::default(),
  };
  let app = Router::new()
    .route("/sign_in", get(sign_in_page))
    .route("/usage", get(usage_page))
    .route("/top_up", get(top_up_page))
    .route("/card", get(card_frame))
    .route("/api/sign_in", post(sign_in))
    .route("/api/pay", post(pay))
    .with_state(fake.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  fake
}

/// The browser the fake platform is paid on and the arguments added to it: the machine's stable Chrome, as the
/// tool's policy names it, where it stands on the path; else the newest full Chromium Playwright keeps, as on a
/// machine whose browser tests run on Playwright's. The tool starts either the same way, on a virtual screen of
/// its own; Playwright's Chromium carries no profile of the machine's security module that would let its sandbox
/// start, so it runs without it, on the fake platform alone.
fn machine_browser(named: &str) -> (String, Vec<String>) {
  let on_path = std::env::var_os("PATH")
    .map(|path| std::env::split_paths(&path).any(|folder| folder.join(named).is_file()))
    .unwrap_or(false);

  if on_path {
    return (named.to_owned(), Vec::new());
  }

  let kept = std::env::var_os("PLAYWRIGHT_BROWSERS_PATH")
    .map(std::path::PathBuf::from)
    .or_else(|| {
      std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".cache/ms-playwright"))
    })
    .expect("a home to find Playwright's browsers under");
  let mut chromiums: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(&kept)
    .into_iter()
    .flatten()
    .filter_map(Result::ok)
    .filter_map(|entry| {
      let revision = entry.file_name().to_str()?.strip_prefix("chromium-")?.parse::<u32>().ok()?;
      let binary = ["chrome-linux64/chrome", "chrome-linux/chrome"]
        .iter()
        .map(|inside| entry.path().join(inside))
        .find(|binary| binary.is_file())?;

      Some((revision, binary))
    })
    .collect();

  chromiums.sort();

  let binary = chromiums.pop().map(|(_, binary)| binary.display().to_string()).unwrap_or_else(|| {
    panic!(
      "no browser to pay the fake platform in: neither {named} on the path nor a Chromium under {}",
      kept.display()
    )
  });

  (binary, vec!["--no-sandbox".to_owned()])
}

/// A payer on the fake platform under `root`, its waits short: `confirm` for a confirmation nobody gives.
fn payer(fake: &Fake, root: &std::path::Path, confirm: Duration) -> Payer {
  let mut payer = Payer::of(root);

  (payer.platform.place.chrome, payer.platform.place.added) = machine_browser(&payer.platform.place.chrome);

  payer.platform.address = format!("http://127.0.0.1:{}", fake.port);
  payer.platform.waits.step = Duration::from_secs(20);
  payer.platform.waits.confirm = confirm;
  payer.platform.waits.look = Duration::from_millis(100);
  payer.balance_wait = Duration::from_secs(5);
  payer.balance_look = Duration::from_millis(100);
  payer
}

/// Every process whose command line names the folder, read from `/proc`.
fn processes_naming(folder: &std::path::Path) -> Vec<String> {
  let named = folder.display().to_string();

  std::fs::read_dir("/proc")
    .unwrap()
    .filter_map(Result::ok)
    .filter_map(|entry| std::fs::read(entry.path().join("cmdline")).ok())
    .map(|line| String::from_utf8_lossy(&line).replace('\0', " "))
    .filter(|line| line.contains(&named))
    .collect()
}

async fn pay_on(fake: &Fake, root: &std::path::Path, confirm: Duration) -> String {
  let stand = Stand::reading(fake.balance.clone());
  let payer = payer(fake, root, confirm);

  let said = top_up_with(&agency(&stand).await, root, &payer, &amount(10.0))
    .await
    .unwrap_or_else(|refused| panic!("{refused}"))
    .plain();

  holds_no_secret_value(&said);
  assert!(
    processes_naming(&payer.platform.place.folder).is_empty(),
    "the browser and its screen ended with the call"
  );

  said
}

#[tokio::test]
async fn a_payment_the_platform_makes_answers_the_amount_and_the_balance_after_and_the_sign_in_stands() {
  let fake = platform(Scenario::Pays).await;
  let root = data_root(Some(&whole_card()), Some(&whole_login()));

  let said = pay_on(&fake, root.path(), Duration::from_secs(20)).await;

  assert!(
    said.starts_with("Paid: 10 USD. Balance: 13.5 USD, as the balance reader last read it at "),
    "{said}"
  );
  assert!(!said.contains("before it"), "the balance was read since the payment: {said}");
  assert_eq!(
    *fake.payments.lock().unwrap(),
    [json!({ "amount": "10", "number": NUMBER, "expiry": EXPIRY, "cvc": CVC, "holder": HOLDER })],
    "the card's fields were typed as saved, and pay was pressed once"
  );
  assert_eq!(
    *fake.sign_ins.lock().unwrap(),
    [json!({ "who": USERNAME, "pass": PASSWORD })],
    "the login was typed as saved"
  );

  let again = pay_on(&fake, root.path(), Duration::from_secs(20)).await;

  assert!(again.starts_with("Paid: 10 USD. Balance: 23.5 USD"), "{again}");
  assert_eq!(fake.sign_ins.lock().unwrap().len(), 1, "the profile kept the sign-in for the next call");
}

#[tokio::test]
async fn a_refused_payment_moves_no_balance_and_is_not_tried_again() {
  let fake = platform(Scenario::Refuses).await;
  let root = data_root(Some(&whole_card()), Some(&whole_login()));

  let said = pay_on(&fake, root.path(), Duration::from_secs(20)).await;

  assert!(
    said.starts_with("Nothing was paid: DeepSeek or the card's issuer refused the payment"),
    "{said}"
  );
  assert!(said.contains("Paid: 0 USD. Balance: 3.5 USD"), "{said}");
  assert_eq!(fake.payments.lock().unwrap().len(), 1, "pay was pressed once");
  assert_eq!(*fake.balance.lock().unwrap(), 3.5);
}

#[tokio::test]
async fn a_confirmation_asked_of_a_person_stops_the_tool_there_and_moves_no_balance() {
  let fake = platform(Scenario::Confirms).await;
  let root = data_root(Some(&whole_card()), Some(&whole_login()));

  let said = pay_on(&fake, root.path(), Duration::from_secs(2)).await;

  assert!(
    said.starts_with("Nothing was paid that the platform confirmed within 2 seconds"),
    "{said}"
  );
  assert!(said.contains("a person approves it in the card's app"), "{said}");
  assert!(said.contains("Paid: 0 USD. Balance: 3.5 USD"), "{said}");
  assert_eq!(fake.payments.lock().unwrap().len(), 1, "pay was pressed once, and nothing after it");
  assert_eq!(*fake.balance.lock().unwrap(), 3.5);
}

#[tokio::test]
async fn a_human_check_at_the_sign_in_stops_before_any_card_is_typed() {
  let fake = platform(Scenario::HumanCheck).await;
  let root = data_root(Some(&whole_card()), Some(&whole_login()));

  let said = pay_on(&fake, root.path(), Duration::from_secs(20)).await;

  assert!(
    said.starts_with("Nothing was paid: DeepSeek asks a person to finish signing in"),
    "{said}"
  );
  assert!(
    said.contains(&format!(
      "--user-data-dir=\"{}\"",
      root.path().join("deepseek-top-up/profile").display()
    )),
    "the answer names the profile a person signs in: {said}"
  );
  assert!(said.contains("Paid: 0 USD. Balance: 3.5 USD"), "{said}");
  assert!(fake.payments.lock().unwrap().is_empty(), "no card was typed or paid");
}
