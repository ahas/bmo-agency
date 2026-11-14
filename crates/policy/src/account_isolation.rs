//! An account's isolation node on this machine (docs/account-isolation.md): its name, the system user it runs as,
//! that user's home and the folders in it, and the sandbox every program of it runs in.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountIsolation {
  pub node_prefix: String,
  pub user_prefix: String,
  pub home_base: String,
  pub link_folder: String,
  pub scratch_folder: String,
  /// How long after a setup that left something missing it is run again by the links pass, in seconds.
  pub setup_again_seconds: i64,
  /// The words ahead of every program's command line, with `{user}`, `{home}`, `{scratch}` and `{path}` marks.
  pub sandbox: Vec<String>,
}

crate::part!(pub ACCOUNT_ISOLATION: AccountIsolation = "account-isolation.toml");
