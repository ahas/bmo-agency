//! The accounts' usage windows as agency keeps them (docs/usage-windows.md): the keeping on agency's store
//! with the readers this machine runs, the registered Claude logins those readers ask for, and a turn's
//! answer kept under the login that answered.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use cache_siren::claude::{ClaudeAccounts, Login, Logins};
use cache_siren::codex::CodexReader;
use cache_siren::{Keeping, Reader, Slot, Window};
use harness_providers::Machine;
use sea_orm::DatabaseConnection;

use crate::control::claude_accounts::{Account, SEED_ACCOUNT, accounts};
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

/// The readers this machine runs: every Claude account's, and Codex's where Codex is installed.
fn readers(db: &DatabaseConnection, machine: &Machine) -> Vec<Arc<dyn Reader>> {
  let mut readers: Vec<Arc<dyn Reader>> =
    vec![Arc::new(ClaudeAccounts::of_provider(Arc::new(StoreLogins(db.clone()))))];
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
