//! A person's supply: the meter's numbers and the words said (docs/supply.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supply {
  /// The meter's beat while a reserved run goes, in seconds.
  pub meter_seconds: u64,
  /// The share of a reservation's amount the running cost reaches before it is extended.
  pub extend_at: f64,
  /// How often a job waiting for supply asks its order again, in seconds.
  pub wait_seconds: u64,
  /// The supply outbox's first wait between tries, in seconds.
  pub retry_first_seconds: i64,
  /// The ceiling the supply outbox's wait doubles to, in seconds.
  pub retry_ceiling_seconds: i64,
  pub words: Words,
  pub test: Test,
}

/// The test of one source of a road (docs/supply.md, 「Testing a source」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
  /// How long the request on a gateway road's member is waited for, in seconds.
  pub gateway_seconds: u64,
  /// How long the request on a registration is waited for, in seconds.
  pub registered_seconds: u64,
  /// The one word the request asks.
  pub asked: String,
  pub words: TestWords,
}

/// What the test says where a source does not answer, each with `{road}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestWords {
  pub off: String,
  pub no_source_own: String,
  pub no_source_platform: String,
  /// With `{model}`.
  pub no_address: String,
  pub login_needed: String,
  pub not_ready: String,
  /// With `{why}`.
  pub refused: String,
  /// With `{why}`.
  pub failed: String,
  /// With `{roads}`.
  pub road: String,
  pub model_needed: String,
  /// With `{model}`.
  pub unknown_model: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Words {
  /// With `{provider}`.
  pub no_personal: String,
  pub no_order: String,
  /// With `{provider}`.
  pub no_lendable: String,
  /// With `{provider}`.
  pub shared_node: String,
  /// With `{model}`.
  pub owned: String,
  /// With `{source}`.
  pub refused: String,
  /// With `{source}`.
  pub taken_first: String,
  /// With `{source}`.
  pub ran_out: String,
  pub withheld: String,
  /// One sentence to each reason no source is left, each with `{source}`, `{reopens}` and `{settings}`.
  pub closed: ClosedWords,
  /// With `{at}`.
  pub reopens: String,
  /// With `{source}` and `{why}`.
  pub refused_told: String,
  /// With `{from}`, `{to}` and `{why}`.
  pub moved_told: String,
  pub settings_section: String,
}

/// The sentence a person is told where no source is left, by why (docs/supply.md, 「Running out mid-turn」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedWords {
  pub no_order: String,
  pub withheld: String,
  pub no_personal: String,
  pub own_refused: String,
  pub no_organisation: String,
  pub organisation_refused: String,
  pub no_lendable: String,
  pub shared_node: String,
  pub owned: String,
  pub refused: String,
  pub ran_out: String,
}

crate::part!(pub SUPPLY: Supply = "supply.toml");
