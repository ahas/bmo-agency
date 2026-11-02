//! The chat line (docs/launch-lines.md): what its system prompt is made of, the tools it holds, and every
//! fixed line of the room's block.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct System {
  pub opening: String,
  pub absolute_rules: String,
  pub manual: String,
  pub rulebooks: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tools {
  pub shown: String,
  pub listed: String,
  pub paragraph: String,
  pub absent: String,
  pub absent_summary: String,
  pub unreached: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
  pub base_layer: String,
  pub header: String,
  pub tone_header: String,
  pub room_header: String,
  pub default_tone: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
  pub head: String,
  pub name: String,
  pub replaces: String,
  pub group_document: String,
  pub owner_document: String,
  pub writing: String,
  pub shows: String,
  pub road: String,
  pub end: String,
  pub mark: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
  pub document: String,
  pub withheld: String,
  pub handed: String,
  pub sending: String,
  pub sending_none: String,
  pub sending_handed: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
  pub room_rules: String,
  pub grants_down: String,
  pub grants_head: String,
  pub grant: String,
  pub members: String,
  pub user: String,
  pub no_senders: String,
  pub until: String,
  pub words: String,
  pub nothing: String,
  pub grants_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Downgrade {
  pub fact: String,
  pub intent: String,
  pub follow_up: String,
  pub owner_room: String,
  pub room: String,
  pub ttl_minutes: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fallback {
  pub taken: String,
  pub moved: String,
  pub intent: String,
  pub ttl_minutes: i64,
  pub lifted: String,
  pub lifted_intent: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatLine {
  pub system: System,
  pub tools: Tools,
  pub persona: Persona,
  pub room: Room,
  pub capabilities: Capabilities,
  pub downgrade: Downgrade,
  pub fallback: Fallback,
  pub rules: Rules,
}

crate::part!(pub CHAT_LINE: ChatLine = "chat-line.toml");
