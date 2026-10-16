//! The gate order and the gates each kind of turn passes.

use serde::Deserialize;

/// A gate's place in a step: the gate, the condition its step runs under where it runs in some turns
/// only, and whether it is one of the gates that pick a turn's reference.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Place {
  pub key: String,
  #[serde(default)]
  pub condition: Option<String>,
  #[serde(default)]
  pub reference: bool,
}

/// One step of a turn part: the gates the turn starts together and waits on together.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
  pub places: Vec<Place>,
}

/// The gates one kind of turn passes, in the two parts a turn has.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnGates {
  pub pre: Vec<Step>,
  pub post: Vec<Step>,
}

impl TurnGates {
  /// Every gate the turn passes, once, in the first part it stands in and in the turn's order, with
  /// that part's name.
  pub fn gates(&self) -> Vec<(&'static str, &str)> {
    let mut seen = std::collections::HashSet::new();

    [("pre", &self.pre), ("post", &self.post)]
      .into_iter()
      .flat_map(|(part, steps)| {
        steps.iter().flat_map(|step| &step.places).map(move |place| (part, place.key.as_str()))
      })
      .filter(|(_, key)| seen.insert(*key))
      .collect()
  }
}

/// The gate order: its chat turn's two parts, the gates that run outside any turn, and the gates
/// whose spend the usage screen reads back.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateOrder {
  pub pre: Vec<Step>,
  pub post: Vec<Step>,
  pub apart: Vec<String>,
  pub usage: Vec<String>,
}

impl GateOrder {
  /// The chat turn's two parts.
  pub fn turn(&self) -> TurnGates {
    TurnGates {
      pre: self.pre.clone(),
      post: self.post.clone(),
    }
  }

  /// Every gate once, where it first stands: the pre-gate part, then the post-gate part, then off-turn.
  pub fn keys(&self) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();

    self
      .pre
      .iter()
      .chain(&self.post)
      .flat_map(|step| &step.places)
      .map(|place| place.key.as_str())
      .chain(self.apart.iter().map(String::as_str))
      .filter(|key| seen.insert(*key))
      .collect()
  }
}

crate::part!(pub GATE_ORDER: GateOrder = "gates.toml");
crate::part!(pub COMPANION_TURN: TurnGates = "companion-turn.toml");
