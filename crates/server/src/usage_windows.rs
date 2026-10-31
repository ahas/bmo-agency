//! The accounts' usage windows as agency keeps them (docs/usage-windows.md): the keeping on agency's store
//! with the readers this machine runs, the registered Claude logins those readers ask for, the run of the
//! CLI that renews a login the Claude reader finds expired, each provider's balance as its member declares
//! the reading, and a turn's answer kept under the login that answered.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, OnceLock, Weak};

use std::time::Duration;

use cache_siren::balance::{Asking, Balance, BalanceAsk, BalanceReader, BalanceSource};
use cache_siren::claude::{ClaudeAccounts, Login, Logins, Renewals};
use cache_siren::codex::CodexReader;
use cache_siren::{Keeping, Reader, Slot, Window};
use harness_providers::balance::{OffersBalance, key_in_file};
use harness_providers::{Machine, Provider};
use policy::providers::claude_code::CLAUDE_CODE;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::control::claude_accounts::{Account, Credential, SEED_ACCOUNT, accounts};
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::record::Reply;
use crate::door::turn::Quiet;
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::renewed;
use crate::lines::Trees;
use crate::lines::claude::{ApartRun, Command, Runs, apart_args, command_env};
use crate::now_ms;
use crate::places::Ground;

const CODEX: &str = "codex";

// The work, the line and the agent a login's renewal run goes out as.
const RENEWAL: &str = "login-renewal";

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
        account: window_account(&account).to_owned(),
      })
    })
    .collect()
}

// The account a reader's slot names an account by.
fn window_account(account: &Account) -> &str {
  if account.id.is_empty() { SEED_ACCOUNT } else { &account.id }
}

/// The run of the CLI the Claude account reader asks on a directory account's login it finds expired or
/// refused (docs/usage-windows.md, 「The Claude account reader」), on the ground the readers pass on once
/// they stand on one; none runs before.
#[derive(Clone, Default)]
pub struct LoginRenewals(Arc<OnceLock<Weak<Ground>>>);

impl LoginRenewals {
  /// The ground the runs go out on, from now on.
  pub fn stand_on(&self, ground: &Arc<Ground>) {
    let _ = self.0.set(Arc::downgrade(ground));
  }
}

impl Renewals for LoginRenewals {
  fn renew(&self, account: &str) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>> {
    let ground = self.0.get().and_then(Weak::upgrade);
    let account = account.to_owned();

    Box::pin(async move {
      let ground = ground.ok_or("no ground stands to run the CLI on")?;

      renew_login(&ground, &account).await
    })
  }
}

/// One run of Claude Code on a directory account's own login, so the CLI renews its token as it goes out:
/// a run apart under the account's configuration directory, under its renewal lock, on the model and
/// effort the usage declaration names, its one word put in with no tool shown and its answer thrown away.
/// A token or endpoint account, and a login waiting for a person, is never run on.
pub async fn renew_login(ground: &Ground, id: &str) -> Result<(), String> {
  let account = accounts(&ground.db)
    .await
    .map_err(|refused| refused.error)?
    .into_iter()
    .find(|account| window_account(account) == id)
    .ok_or("no such account is registered")?;

  if !matches!(account.credential, Credential::Directory { .. }) {
    return Err("it holds no login the CLI renews".to_owned());
  }

  if account.needs_login_at.is_some() && !account.login_back(now_ms()) {
    return Err("its login waits for a person to sign in again".to_owned());
  }

  let usage = &CLAUDE_CODE.usage;
  let session = new_session();
  let args = apart_args(
    &ApartRun {
      line: RENEWAL,
      model: &usage.renewal_model,
      effort: &usage.renewal_effort,
      system_prompt: None,
      mcp_config: None,
      tools_shown: "",
      tools_allowed: "",
      host_commands: false,
    },
    &session,
  )
  .map_err(|why| why.detail)?;
  let env = command_env(ground.env(), Some(&account), &Runs::Here, false).map_err(|why| why.detail)?;
  let cwd = Trees::of(ground).agents.join(RENEWAL);

  std::fs::create_dir_all(&cwd).map_err(|why| why.to_string())?;

  let apart = Apart {
    agent: RENEWAL,
    line: RENEWAL,
    provider: crate::lines::runner::PROVIDER,
    title: format!("{RENEWAL} · {id} · {}", usage.renewal_model),
    command: Command {
      args,
      cwd: cwd.to_string_lossy().into_owned(),
      env,
    },
    session,
    prompt: usage.renewal_words.clone(),
    work_key: RENEWAL.to_owned(),
    model: usage.renewal_model.clone(),
    effort: usage.renewal_effort.clone(),
    tier: None,
    bound: None,
    parent: None,
    root: None,
  };
  let ending = renewed(&ground.roots.data, &account, async {
    apart::run(ground, apart, &mut Quiet)
      .await
      .map_err(|message| Failure::Transport { message })
  })
  .await
  .map_err(|failure| failure.message())?;

  answered(ending, RENEWAL).map(drop).map_err(|failure| failure.message())
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

/// The readers this machine runs: every Claude account's, asking `renewals` to renew a login it finds
/// expired, Codex's where Codex is installed, and the balance of every provider that offers one.
fn readers(db: &DatabaseConnection, machine: &Machine, renewals: &LoginRenewals) -> Vec<Arc<dyn Reader>> {
  let mut readers: Vec<Arc<dyn Reader>> = vec![Arc::new(
    ClaudeAccounts::of_provider(Arc::new(StoreLogins(db.clone()))).renewing(Arc::new(renewals.clone())),
  )];

  for provider in harness_providers::providers() {
    if let Some(balances) = ProviderBalances::of(provider, machine, db) {
      readers.push(Arc::new(BalanceReader::new(Arc::new(balances))));
    }
  }

  let codex = harness_providers::provider(CODEX)
    .filter(|codex| !codex.models_on_gateway() && codex.missing(machine).is_none());

  if codex.is_some() {
    let sessions = PathBuf::from(machine.var("HOME").unwrap_or_default()).join(".codex").join("sessions");
    let program = machine.program(CODEX).unwrap_or_else(|| PathBuf::from(CODEX));

    readers.push(Arc::new(CodexReader::new(sessions, program)));
  }

  readers
}

/// The keeping on agency's store, with the readers this machine runs, the Claude reader's renewals run by
/// `renewals`.
pub fn keeping(db: &DatabaseConnection, machine: &Machine, renewals: &LoginRenewals) -> Keeping {
  Keeping::new(db.clone(), readers(db, machine, renewals))
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
