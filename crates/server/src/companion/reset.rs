//! What a companion's reset tells the units that hold the rest of it (docs/api.md, 「Events」;
//! docs/companions.md, 「Starting it afresh」): each move of its reset point, so the rooms' owner lets go
//! what waited for its turns, and the reset once agency's part is taken, so each unit takes its own rows.

use serde::Serialize;

use crate::events::Event;

/// A companion cut off at its reset point: a turn begun at or before it is overtaken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionCutOff {
  pub agent: String,
  /// The point, in epoch milliseconds.
  pub point: i64,
}

impl Event for CompanionCutOff {
  const NAME: &'static str = "companion-reset.created";
}

/// A companion's reset with agency's part taken: the ranges taken, the rooms it answers alone and those it
/// answered with others, who asked and when, in epoch seconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionReset {
  pub agent: String,
  pub ranges: Vec<String>,
  pub alone: Vec<i64>,
  pub shared: Vec<i64>,
  pub by: String,
  pub at: i64,
}

impl Event for CompanionReset {
  const NAME: &'static str = "companion-reset.created";
}
