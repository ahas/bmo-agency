//! The ceilings two gates' prompts are told (docs/agent-documents.md): the most pieces of the person book
//! its search may name for one turn, and the most entries the document seeker picks for one question.

use serde::Deserialize;

/// One gate's ceiling, the number its prompt is rendered with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ceiling {
  pub cap: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatePrompts {
  pub people_search: Ceiling,
  pub document_seek: Ceiling,
}

crate::part!(pub GATE_PROMPTS: GatePrompts = "gate-prompts.toml");
