//! The accounts' usage windows as agency keeps them (docs/usage-windows.md): the keeping on agency's store
//! with the readers this machine runs, the registered Claude logins those readers ask for, each provider's
//! balance as its member declares the reading, and a turn's answer kept under the login that answered.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use std::time::Duration;

use cache_siren::balance::{Asking, Balance, BalanceAsk, BalanceReader, BalanceSource};
use cache_siren::claude::{ClaudeAccounts, Login, Logins};
use cache_siren::codex::CodexReader;
use cache_siren::{Keeping, Reader, Slot, Window};
use harness_providers::balance::{OffersBalance, key_in_file};
use harness_providers::{Machine, Provider};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::control::claude_accounts::{Account, Credential, SEED_ACCOUNT, accounts};
use crate::door::record::Reply;

const CODEX: &str = "codex";

/// Every registered Claude account with usage windows, on or off, as the store holds it now: an endpoint
/// account is paid by the request and is never asked of the reader.
struct StoreLogins(DatabaseConnection);

impl Logins for StoreLogins {
  fn logins(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Login>, String>> + Send + '_>> {
    Box::pin(async move { Ok(window_logins(accounts(&self.0).await.map_err(|refused| refused.error)?)) })
  }
}

/// The logins the Claude account reader asks of, one each account that has usage windows.
pub fn window_logins(held: Vec<Account>) -> Vec<Login> {
  held
    .into_iter()
    .filter_map(|account| {
      Some(Login {
        credential: account.credential.window_credential()?,
        account: if account.id.is_empty() { SEED_ACCOUNT.to_owned() } else { account.id },
      })
    })
    .collect()
}

/// One provider's balance as its member declares the reading: its own road where it can answer on this
/// machine, and every endpoint account of the store whose endpoint is the provider's, with the account's own
/// key file.
pub struct ProviderBalances {
  pub provider: &'static dyn Provider,
  pub offers: &'static dyn OffersBalance,
  pub machine: Machine,
  pub db: DatabaseConnection,
}

impl ProviderBalances {
  /// The provider's balance on this machine, where it offers one.
  pub fn of(provider: &'static dyn Provider, machine: &Machine, db: &DatabaseConnection) -> Option<Self> {
    Some(Self {
      provider,
      offers: provider.balance()?,
      machine: machine.clone(),
      db: db.clone(),
    })
  }
}

impl BalanceSource for ProviderBalances {
  fn mode(&self) -> &str {
    self.provider.name()
  }

  fn cadence(&self) -> Duration {
    Duration::from_secs(self.offers.declared().pass_seconds)
  }

  fn fresh(&self) -> Duration {
    self.cadence() * self.offers.declared().fresh_passes
  }

  fn wait(&self) -> Duration {
    Duration::from_secs(self.offers.declared().ask_seconds)
  }

  fn asks(&self) -> Asking<'_> {
    Box::pin(async move {
      let url = self.offers.url(&self.machine);
      let mut asks = Vec::new();

      if self.provider.missing(&self.machine).is_none() {
        asks.push(BalanceAsk {
          account: String::new(),
          url: url.clone(),
          key: self.offers.key(&self.machine),
        });
      }

      for account in accounts(&self.db).await.map_err(|refused| refused.error)? {
        if let Credential::Endpoint {
          url: endpoint,
          key_file,
          ..
        } = &account.credential
          && self.offers.serves(endpoint, &self.machine)
        {
          asks.push(BalanceAsk {
            account: account.id.clone(),
            url: url.clone(),
            key: key_in_file(key_file),
          });
        }
      }

      Ok(asks)
    })
  }

  fn balance_of(&self, answer: &Value) -> Result<Balance, String> {
    self.offers.read(answer).map(|read| Balance {
      amount: read.amount,
      currency: read.currency,
    })
  }
}

/// The readers this machine runs: every Claude account's, Codex's where Codex is installed, and the
/// balance of every provider that offers one.
fn readers(db: &DatabaseConnection, machine: &Machine) -> Vec<Arc<dyn Reader>> {
  let mut readers: Vec<Arc<dyn Reader>> =
    vec![Arc::new(ClaudeAccounts::of_provider(Arc::new(StoreLogins(db.clone()))))];

  for provider in harness_providers::providers() {
    if let Some(balances) = ProviderBalances::of(provider, machine, db) {
      readers.push(Arc::new(BalanceReader::new(Arc::new(balances))));
    }
  }

  let codex = harness_providers::provider(CODEX).filter(|codex| codex.missing(machine).is_none());

  if codex.is_some() {
    let sessions = PathBuf::from(machine.var("HOME").unwrap_or_default()).join(".codex").join("sessions");
    let program = machine.program(CODEX).unwrap_or_else(|| PathBuf::from(CODEX));

    readers.push(Arc::new(CodexReader::new(sessions, program)));
  }

  readers
}

/// The keeping on agency's store, with the readers this machine runs.
pub fn keeping(db: &DatabaseConnection, machine: &Machine) -> Keeping {
  Keeping::new(db.clone(), readers(db, machine))
}

/// The windows a turn's answer read, kept at `at` in epoch seconds under the login that answered: the
/// empty account on a provider that holds no logins of agency's, and nothing written where the provider
/// holds logins and the answer names none. An answer that read no windows leaves the slot as it was, and a
/// keeping that cannot be written is said with its cause rather than failing the turn.
pub async fn keep_reply(keeping: &Keeping, reply: &Reply, at: i64) {
  if reply.windows.is_empty() {
    return;
  }

  let holds_logins =
    harness_providers::provider(&reply.mode).is_some_and(|provider| provider.holds_logins());
  let account = match reply.account.as_deref().filter(|account| !account.is_empty()) {
    Some(account) if holds_logins => account,
    None if holds_logins => return,
    _ => "",
  };
  let windows: Vec<Window> = reply.windows.iter().map(Window::from).collect();

  if let Err(fault) = keeping
    .replace(
      Slot {
        mode: &reply.mode,
        account,
      },
      &windows,
      at,
    )
    .await
  {
    eprintln!(
      "{} windows of {}: not kept: {fault}",
      reply.mode,
      if account.is_empty() { "no login" } else { account }
    );
  }
}
