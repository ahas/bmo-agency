//! The words of the one road resolution (docs/cache-fallback.md, 「One road resolution」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roads {
  pub reasons: Reasons,
  pub logins: Logins,
  pub closed: Closed,
  pub job: Job,
}

/// Why a provider cannot answer now.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reasons {
  /// A pinned provider the owner turned off.
  pub off: String,
  /// A balance at or below zero, with `{amount}` and `{currency}`.
  pub balance: String,
  /// No login of a provider that holds logins is open.
  pub no_login: String,
}

/// Why one login cannot answer now, each with `{account}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logins {
  pub off: String,
  /// With `{until}`.
  pub resting: String,
  pub person: String,
  pub key: String,
  pub balance: String,
}

/// The sentence a closed road is refused or held with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Closed {
  pub head: String,
  /// With `{provider}` and `{reason}`.
  pub provider: String,
  /// With `{at}`.
  pub reopens: String,
  /// With `{provider}` and `{command}`.
  pub person: String,
  /// With `{providers}`, `{line}` and `{needs}`.
  pub uncarried: String,
}

/// A job launched again on the next road.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
  /// With `{from}` and `{why}`.
  pub continued: String,
  /// With `{roads}`.
  pub moved: String,
  /// With `{road}` and `{why}`.
  pub road: String,
  /// How often a job waiting for a road asks again, in seconds.
  pub wait_seconds: u64,
}

crate::part!(pub ROADS: Roads = "roads.toml");
