//! The companions' pieces and their limits (docs/companions.md, 「Its
//! settings」), and the generation settings their turns are asked with (「Its generation settings」).

use serde::Deserialize;

use crate::model_catalogue::NumberBounds;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionRules {
  /// Every companion's pieces, in their default order.
  pub pieces: Vec<String>,
  /// The longest a persona the settings hold may be, in characters.
  pub persona_most: usize,
  /// The longest a piece of a companion's own may be, in characters.
  pub own_piece_most: usize,
  /// The pattern a piece of a companion's own is named by.
  pub own_piece_pattern: String,
  /// The pattern a companion added on the console is named by.
  pub name_pattern: String,
  /// The mark a screen puts beside a companion added on the console.
  pub added_icon: String,
  /// Whether a companion's turns are given their reference where its settings never held the switch.
  pub reference_on: bool,
}

impl CompanionRules {
  pub fn is_piece(&self, name: &str) -> bool {
    self.pieces.iter().any(|piece| piece == name)
  }
}

crate::part!(pub COMPANIONS: CompanionRules = "companions.toml");

/// One generation setting, by the name the engine reads it under. Its bounds are read flattened, which
/// serde reads only without refusing unknown fields.
#[derive(Debug, Deserialize)]
pub struct GenerationSetting {
  pub name: String,
  /// Whether it takes on or off rather than a number.
  #[serde(default)]
  pub switch: bool,
  /// Whether the chat template reads it, so a request carries it under `chat_template_kwargs`.
  #[serde(default)]
  pub template: bool,
  /// Its bounds where it takes a number; a switch has none.
  #[serde(flatten)]
  pub bounds: Option<NumberBounds>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationSettings {
  pub settings: Vec<GenerationSetting>,
}

impl GenerationSettings {
  pub fn setting(&self, name: &str) -> Option<&GenerationSetting> {
    self.settings.iter().find(|setting| setting.name == name)
  }
}

crate::part!(pub GENERATION: GenerationSettings = "generation.toml");
