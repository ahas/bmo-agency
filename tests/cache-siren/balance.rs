//! The balance reader against a stand-in endpoint: each account asked with its own key and its balance kept
//! under it alone, an account refused or with no key keeping what it had, an endpoint slow to answer waited
//! for, and the reader declaring its source's provider, cadence and span.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cache_siren::balance::{Asking, BalanceAsk, BalanceReader, BalanceSource};
use cache_siren::{Balance, Keeping, Reader, Slot};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::scratch::Scratch;

const NOW_MS: i64 = 1_800_000_000_000;
const NOW: i64 = NOW_MS / 1000;

// The key the stand-in refuses as unauthorised, and the one it answers only after a while.
const REFUSED: &str = "sk-refused";
const SLOW: &str = "sk-slow";

/// A source naming its asks as given, reading `left` out of an answer.
struct Fixed {
  url: String,
  asks: Vec<(&'static str, Result<&'static str, &'static str>)>,
}

impl BalanceSource for Fixed {
  fn mode(&self) -> &str {
    "paid"
  }

  fn cadence(&self) -> Duration {
    Duration::from_secs(300)
  }

  fn fresh(&self) -> Duration {
    Duration::from_secs(1800)
  }

  fn asks(&self) -> Asking<'_> {
    Box::pin(async move {
      Ok(
        self
          .asks
          .iter()
          .map(|(account, key)| BalanceAsk {
            account: (*account).to_owned(),
            url: self.url.clone(),
            key: key.map(str::to_owned).map_err(str::to_owned),
          })
          .collect(),
      )
    })
  }

  fn balance_of(&self, answer: &Value) -> Result<Balance, String> {
    Ok(Balance {
      amount: answer["left"].as_f64().ok_or("the answer states nothing left")?,
      currency: "CNY".into(),
    })
  }
}

/// A balance endpoint answering each key with what it has left, and the authorisation each ask carried.
async fn balance_endpoint() -> (String, Arc<Mutex<Vec<String>>>) {
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

      match said.strip_prefix("Bearer ").unwrap_or_default() {
        REFUSED => StatusCode::UNAUTHORIZED.into_response(),
        SLOW => {
          tokio::time::sleep(Duration::from_millis(300)).await;
          axum::Json(json!({ "left": 3.5 })).into_response()
        }
        _ => axum::Json(json!({ "left": 42.5 })).into_response(),
      }
    }
  });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (format!("http://127.0.0.1:{port}/balance"), asked)
}

fn slot(account: &str) -> Slot<'_> {
  Slot {
    mode: "paid",
    account,
  }
}

#[tokio::test]
async fn each_account_is_read_with_its_own_key_and_one_that_cannot_be_asked_keeps_what_it_had() {
  let scratch = Scratch::new().await;
  let (url, asked) = balance_endpoint().await;
  let source = Fixed {
    url,
    asks: vec![
      ("", Ok("sk-road")),
      ("refused", Ok(REFUSED)),
      ("keyless", Err("the key file is unreadable")),
      ("slow", Ok(SLOW)),
    ],
  };
  let reader = Arc::new(BalanceReader::new(Arc::new(source)));
  let keeping = Keeping::new(scratch.db().clone(), vec![reader.clone()]);
  let before = Balance {
    amount: 7.0,
    currency: "CNY".into(),
  };

  for account in ["refused", "keyless", "slow"] {
    keeping.replace_balance(slot(account), &before, NOW - 60).await.unwrap();
  }

  let outcomes = reader.read_each(&keeping, NOW_MS).await.unwrap();

  assert_eq!(
    outcomes,
    [
      (
        String::new(),
        Ok(Balance {
          amount: 42.5,
          currency: "CNY".into()
        })
      ),
      ("refused".to_owned(), Err("HTTP 401".to_owned())),
      ("keyless".to_owned(), Err("the key file is unreadable".to_owned())),
      (
        "slow".to_owned(),
        Ok(Balance {
          amount: 3.5,
          currency: "CNY".into()
        })
      ),
    ]
  );
  assert_eq!(
    *asked.lock().unwrap(),
    ["Bearer sk-road", "Bearer sk-refused", "Bearer sk-slow"],
    "a key goes in its own account's ask alone, and no ask goes out without one"
  );

  let road = keeping.balance(slot("")).await.unwrap().unwrap();

  assert_eq!(road.balance.amount, 42.5);
  assert_eq!(road.seen_at, NOW);

  let slow = keeping.balance_of_account("slow").await.unwrap().unwrap();

  assert_eq!((slow.balance.amount, slow.seen_at), (3.5, NOW), "an ask slow to answer is waited for");

  for account in ["refused", "keyless"] {
    let kept = keeping.balance_of_account(account).await.unwrap().unwrap();

    assert_eq!((kept.balance, kept.seen_at), (before.clone(), NOW - 60), "{account}");
  }

  assert_eq!(keeping.balance_of_account("").await.unwrap(), None, "the road's own is no account's");
  assert_eq!(keeping.balance(slot("never")).await.unwrap(), None);
}

#[test]
fn the_reader_declares_its_source_s_provider_cadence_and_span() {
  let reader = BalanceReader::new(Arc::new(Fixed {
    url: "http://127.0.0.1:9/".into(),
    asks: vec![],
  }));

  assert_eq!(reader.mode(), "paid");
  assert_eq!(reader.cadence(), Duration::from_secs(300));
  assert_eq!(reader.fresh(), Duration::from_secs(1800));
}
