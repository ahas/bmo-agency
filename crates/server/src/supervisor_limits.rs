//! What a supervisor's settings close of its sessions (docs/supervisors.md, 「Its settings」): its brain
//! switch, which takes away every capability through which a session reaches a brain, its reference switch,
//! which takes away those through which it reaches the gate's reading of the brain, and its tool limit,
//! which keeps only the capabilities it names, leaves out the servers and the program's own tools the chat line
//! holds under a capability it does not name, and starts the program with no built-in tool but those it keeps. A
//! supervisor whose settings hold neither is handed everything, as every supervisor is by default.

use policy::roles::ROLES;
use policy::run_lines::RUN_LINES;
use sea_orm::ConnectionTrait;

use crate::Refusal;
use crate::control::supervisor_settings::Settings;
use crate::lines::fault;

/// The line a supervisor's turns go out on, whose `held` names what a limit closes outside the tool host.
const CHAT_LINE: &str = "chat";

/// A supervisor's brain switch, reference switch and tool limit, as its settings hold them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
  brain_on: bool,
  reference_on: bool,
  tools_allowed: Option<Vec<String>>,
}

impl Limits {
  /// What a session is handed where no supervisor's settings close anything.
  pub fn open() -> Self {
    Self {
      brain_on: true,
      reference_on: true,
      tools_allowed: None,
    }
  }

  pub fn new(brain_on: bool, reference_on: bool, tools_allowed: Option<Vec<String>>) -> Self {
    Self {
      brain_on,
      reference_on,
      tools_allowed,
    }
  }

  /// The supervisor's limits, read afresh from its settings.
  pub async fn of<C: ConnectionTrait>(db: &C, supervisor: &str) -> Result<Self, Refusal> {
    let settings = Settings::of(db, supervisor)
      .await
      .map_err(|refused| fault(format!("{supervisor}'s settings could not be read: {}", refused.error)))?;

    Ok(Self::new(settings.brain_on, settings.reference_on, settings.tools_allowed))
  }

  pub fn brain_on(&self) -> bool {
    self.brain_on
  }

  /// Whether the gate's reading of the brain reaches the supervisor's turns: its brain and its reference both
  /// switched on.
  pub fn reference_reaches(&self) -> bool {
    self.brain_on && self.reference_on
  }

  /// The capabilities a record keeps of those it would hold: none the brain is reached through where the
  /// brain is off, none the gate's reading of it is reached through where the reference is off, and only
  /// those the limit names where it names any. It never widens them.
  pub fn narrowed(&self, capabilities: Vec<String>) -> Vec<String> {
    capabilities
      .into_iter()
      .filter(|capability| self.brain_on || !ROLES.brain.contains(capability))
      .filter(|capability| self.reference_on || !ROLES.reference.contains(capability))
      .filter(|capability| self.allows(capability))
      .collect()
  }

  /// Whether the chat line's configuration keeps the server of that name: every server no `held` entry
  /// names, and each one whose capability the limit allows.
  pub fn keeps_server(&self, server: &str) -> bool {
    RUN_LINES
      .held(CHAT_LINE)
      .iter()
      .filter(|held| held.servers.iter().any(|name| name == server))
      .all(|held| self.allows(&held.capability))
  }

  /// The chat line's denied list, `denied` as the line declares it, with every program tool whose
  /// capability the limit leaves out after it.
  pub fn denied(&self, denied: &str) -> String {
    let closed: Vec<&str> = RUN_LINES
      .held(CHAT_LINE)
      .iter()
      .filter(|held| !self.allows(&held.capability))
      .flat_map(|held| held.tools.iter().map(String::as_str))
      .filter(|tool| !denied.split(',').any(|named| named.trim() == *tool))
      .collect();

    std::iter::once(denied)
      .filter(|denied| !denied.is_empty())
      .chain(closed)
      .collect::<Vec<_>>()
      .join(",")
  }

  /// The built-in tools the chat line's program is started with and no other, where a limit is held: those no
  /// capability gives that a limited program keeps, then those under each capability the limit allows. None
  /// where no limit is held, so the program keeps every tool of its own but the denied list.
  pub fn kept(&self) -> Option<String> {
    self.tools_allowed.as_ref()?;

    let held = RUN_LINES
      .held(CHAT_LINE)
      .iter()
      .filter(|held| self.allows(&held.capability))
      .flat_map(|held| held.tools.iter());
    let mut kept: Vec<&str> = vec![];

    for tool in RUN_LINES.limited_keeps(CHAT_LINE).iter().chain(held) {
      if !kept.contains(&tool.as_str()) {
        kept.push(tool);
      }
    }

    Some(kept.join(","))
  }

  fn allows(&self, capability: &str) -> bool {
    self
      .tools_allowed
      .as_ref()
      .is_none_or(|allowed| allowed.iter().any(|named| named == capability))
  }
}

/// Every capability a supervisor's tool limit may name: what the chat line gives, every bundle the policy
/// declares (a supervisor's own and its double's role's), and what the chat line holds outside the tool
/// host.
pub fn offered() -> Vec<String> {
  let mut offered: Vec<String> = vec![];

  for capability in RUN_LINES
    .capabilities(CHAT_LINE)
    .iter()
    .chain(ROLES.bundles.iter().map(|bundle| &bundle.name))
    .chain(RUN_LINES.held(CHAT_LINE).iter().map(|held| &held.capability))
  {
    if !offered.contains(capability) {
      offered.push(capability.clone());
    }
  }

  offered
}
