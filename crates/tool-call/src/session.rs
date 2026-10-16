//! The parts of a request's session record agency's tools read, and the turn the request is made in, read
//! afresh from it (docs/tools.md, 「A tool's folder」). The record is the launcher's; a field it holds that no
//! tool of agency's reads is passed over.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

/// The turn variable naming the agent the turn is.
pub const AGENT_VARIABLE: &str = "BMO_TURN_AGENT";

/// The turn variable carrying the moment the turn took its words, in epoch milliseconds.
pub const BEGAN_VARIABLE: &str = "BMO_TURN_BEGAN";

/// The turn variable naming the room the turn answers.
pub const CHAT_VARIABLE: &str = "BMO_TURN_CHAT";

/// The turn variable naming the road the turn answers its room on.
pub const VIA_VARIABLE: &str = "BMO_TURN_VIA";

/// What a search, a browse or a branch is held to, of which agency's tools read the rulebooks alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Research {
  #[serde(default)]
  pub rulebooks: Vec<String>,
}

/// One session's record, its field names camel case as the launcher writes them.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
  #[serde(default)]
  pub line: String,
  #[serde(default)]
  pub capabilities: BTreeSet<String>,
  /// The data root the run reads.
  #[serde(default)]
  pub data_root: PathBuf,
  /// The store's address.
  #[serde(default)]
  pub archive: String,
  /// A resident program's place, where each request reads which run is going.
  #[serde(default)]
  pub place: Option<PathBuf>,
  /// The run's own variables, for a run spawned for itself.
  #[serde(default)]
  pub run_variables: BTreeMap<String, String>,
  #[serde(default)]
  pub research: Option<Research>,
}

impl Session {
  /// One variable of the turn the request is made in: the run the resident program's place names — none for a
  /// person's turn — standing over the run's own variables.
  pub fn turn_variable(&self, name: &str) -> Option<String> {
    let from_place = self.place.as_ref().and_then(|place| {
      let run: Value = serde_json::from_str(&std::fs::read_to_string(place.join("run.json")).ok()?).ok()?;

      if run.get("person").and_then(Value::as_bool) == Some(true) {
        return None;
      }

      run.get("env")?.get(name)?.as_str().map(str::to_owned)
    });

    from_place.or_else(|| self.run_variables.get(name).cloned())
  }

  /// The rulebooks a research run was handed; none for a session that is no research run's.
  pub fn rulebooks(&self) -> &[String] {
    self.research.as_ref().map_or(&[], |research| &research.rulebooks)
  }
}

/// What a request reads of its turn: the agent it is, the moment it took its words, the room it answers and the
/// road it answers on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Turn {
  pub agent: Option<String>,
  /// In epoch milliseconds.
  pub began: Option<i64>,
  pub chat: Option<i64>,
  pub via: Option<String>,
}

impl Turn {
  /// The turn the record's request is made in.
  pub fn of(session: &Session) -> Self {
    let named = |name: &str| {
      session
        .turn_variable(name)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };

    Self {
      agent: named(AGENT_VARIABLE),
      began: named(BEGAN_VARIABLE).and_then(|at| at.parse().ok()),
      chat: named(CHAT_VARIABLE).and_then(|chat| chat.parse().ok()),
      via: named(VIA_VARIABLE),
    }
  }
}
