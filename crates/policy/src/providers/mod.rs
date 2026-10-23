//! What each harness provider declares (docs/provider-presence.md): one file to a provider under
//! `policy/providers/`, read by that provider's own module here and by nothing else.

use serde::Deserialize;

pub mod claude_code;
pub mod codex;
pub mod deepseek;
pub mod openrouter;
pub mod registered;

/// What every provider declares, whoever it is.
#[derive(Debug, Deserialize)]
pub struct Declared {
  /// The provider's name as a screen shows it.
  pub label: String,
  /// The lines its program carries.
  pub lines: Vec<String>,
  /// Whether a mapping of it carries a ceiling on the answer's tokens.
  pub caps_answer_tokens: bool,
  /// Whether co-workers are borrowed on it.
  pub lends_coworkers: bool,
  /// The share of a model's window a session on it may fill before it is folded, where the fold-ratio
  /// table holds none for the session's tier (docs/sessions.md, 「Session compaction」); none leaves the
  /// general share `resident-turn.toml` names.
  #[serde(default)]
  pub fold_ratio: Option<f64>,
  pub effort: Effort,
  /// Its standing program, where it stands one in an agent's pane.
  pub resident: Option<Resident>,
  /// The route its turns take through dsh, where it has one.
  #[serde(default)]
  pub dsh: Option<Dsh>,
}

/// Where a provider's dsh route answers, and the variable its key is read from.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dsh {
  pub api: String,
  #[serde(default)]
  pub api_env: Option<String>,
  pub key_env: String,
  /// A key file under the data root's env directory, where the key is not in the environment.
  #[serde(default)]
  pub key_file: Option<String>,
}

impl Dsh {
  /// The API's address on a machine: the stand-in's its environment names, or the route's own.
  pub fn api_on<'a>(&'a self, vars: &'a std::collections::BTreeMap<String, String>) -> &'a str {
    self
      .api_env
      .as_ref()
      .and_then(|name| vars.get(name))
      .map(String::as_str)
      .filter(|named| !named.is_empty())
      .unwrap_or(&self.api)
  }
}

/// What a provider's standing program takes from outside its turns.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resident {
  /// The keys typed into the program's pane that cut a working turn short.
  pub interrupt: Vec<String>,
  /// The program that stands in the agent's pane, by the name this machine's PATH resolves or a path of
  /// its own, where its surface stands one.
  #[serde(default)]
  pub program: Option<String>,
  /// The harness program the pane program starts on its `dsh` subcommand, which speaks the Agent Client
  /// Protocol; a name on the machine's PATH or a path of its own.
  #[serde(default)]
  pub dsh: Option<String>,
  /// The door one turn of its standing program goes through.
  pub turns: Turns,
  /// Whether words put in while a turn works are taken into that turn.
  pub words_mid_turn: bool,
  /// How a conversation's session grown to its mark is handed over (docs/agents.md, 「What a provider
  /// declares for a conversation」).
  #[serde(default)]
  pub handover: Handover,
  /// Whether a conversation's turn on it is watched for going nowhere and cut at the conversation limits.
  #[serde(default)]
  pub turn_limits: bool,
  /// Whether a conversation's turn on it waits while an open job runs on the engine it asks.
  #[serde(default)]
  pub waits_for_engine: bool,
  /// The question its screen shows before its input box where it asks whether to trust a folder, for
  /// a program whose question herdr cannot tell from rest.
  #[serde(default)]
  pub trust_question: Option<String>,
}

/// How a conversation's full session is handed over: the program's own fold put into it as a turn, or the
/// session asked in one more turn for a note a new session opens with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Handover {
  #[default]
  Fold,
  Handoff,
}

/// How a turn of a standing program goes in and is read back: by its line's own turn through Claude
/// Code's transcript, which the surface carries none of; or by the surface's own turn — Claude Code's
/// transcript door on another's endpoint, Codex's rollout, or the pane program's inbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Turns {
  Transcript,
  Endpoint,
  Rollout,
  Inbox,
}

impl Declared {
  /// Whether its program carries a line.
  pub fn carries(&self, line: &str) -> bool {
    self.lines.iter().any(|one| one == line)
  }
}

/// The effort words a mapping of the provider may hold, and what a launch makes of the one it names.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effort {
  pub words: Vec<String>,
  /// Whether a mapping may name no effort at all.
  pub none_held: bool,
  pub launch: Launch,
  /// What a launch the rule refuses says.
  pub fault: Option<String>,
}

/// What a launch makes of a mapping's effort word.
#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Launch {
  /// A run goes out only with an effort.
  Required,
  /// A run goes out only with none.
  Refused,
  /// A run goes out either way.
  Either,
}

impl Effort {
  /// Every word a mapping may hold, no effort first where a mapping may name none.
  pub fn vocabulary(&self) -> Vec<Option<&str>> {
    let none = self.none_held.then_some(None);

    none.into_iter().chain(self.words.iter().map(|word| Some(word.as_str()))).collect()
  }

  /// Whether a mapping may hold this word.
  pub fn holds(&self, effort: Option<&str>) -> bool {
    match effort {
      None => self.none_held,
      Some(word) => self.words.iter().any(|one| one == word),
    }
  }

  /// Why a run of the provider cannot go out with the effort its mapping named, or nothing where it
  /// can.
  pub fn fault(&self, effort: Option<&str>) -> Option<&str> {
    let refused = match self.launch {
      Launch::Required => effort.is_none(),
      Launch::Refused => effort.is_some(),
      Launch::Either => false,
    };

    refused.then(|| self.fault.as_deref().unwrap_or("this provider refuses the effort"))
  }
}
