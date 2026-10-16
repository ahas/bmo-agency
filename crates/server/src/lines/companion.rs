//! A companion's turn as a turn of its resident program on the chat line (docs/agents.md, 「Conversational
//! agents」; docs/companions.md, 「What a turn shows」): the chat line's words with the system prompt its
//! settings compose in place of the program's own, in the companion's own folder, on the room's session —
//! the one it resumes, or one it opens — and the room, the companion, the run and the messages the turn
//! answers among the run's own variables. On the registry's provider the program is Claude Code on the
//! gateway, sized to the registration and placed on one of its nodes; on Claude Code's own, the program on
//! the login the walk offers the companion, as every Claude run is.

use std::path::PathBuf;

use agent_host::place::ResidentCommand;
use harness_machine::companion_prompt;
use harness_providers::Machine;
use harness_providers::claude_code::ClaudeCode;
use harness_providers::registered::Registered;
use indexmap::IndexMap;
use policy::chat_line::CHAT_LINE;
use policy::fill_marks;
use policy::providers::registered::REGISTERED;
use serde_json::Value;

use super::chat::{Words, standing_words};
use super::claude::{Command, Runs, Session, command_env, line_environment};
use super::place::{Holds, Place, write_place_settings};
use super::standing::{OnGateway, Placing, gateway_command, status_root};
use super::worker::{effort_of, reasoning_of, system_prompt_file};
use super::{Trees, fault};
use crate::Refusal;
use crate::control::claude_accounts::{self, Account};
use crate::control::registry::{self, Registration};
use crate::places::Ground;

/// The line a companion's turn goes out on: the chat line, as every conversational agent's does.
pub const LINE: &str = super::chat::LINE;

// The provider whose models are the registry's registrations, carried by Claude Code on the gateway.
const REGISTERED_PROVIDER: &str = "registered";

/// What the program says of a session it is asked to resume and holds no record of.
pub const LOST_SESSION: &str = "No conversation found with session ID";

/// One turn's run as its runner asks for it.
pub struct CompanionTurn<'a> {
  pub agent: &'a str,
  pub chat_id: i64,
  pub provider: &'a str,
  pub model: &'a str,
  pub system: &'a str,
  pub prompt: &'a str,
  /// Whether every new message the turn answers is the owner's, or no person's message opened it, which
  /// the tool host's grant lock reads as the deployment's turns'.
  pub privileged: bool,
  /// The harness session the turn goes on in, and the place it stands on; none opens a new one.
  pub resume: Option<(&'a str, Option<&'a str>)>,
  pub run: &'a str,
  /// The messages the turn answers.
  pub answers: &'a [i64],
  /// Its generation settings, in the order they were saved.
  pub generation: &'a IndexMap<String, Value>,
  /// The moment the turn took its words, in unix milliseconds, which a reset begun after it overtakes
  /// (docs/companions.md, 「What the reset cuts off」).
  pub began: i64,
}

/// The turn composed: the command the companion's program stands on, the turn's own variables among its
/// environment; the session that command opens or resumes; the model and effort its answer is written down
/// under, the place its session is kept as standing on, and the registration's window where the run is
/// one's.
pub struct ComposedTurn {
  /// The system prompt's file the program reads.
  pub system_path: PathBuf,
  pub command: ResidentCommand,
  pub session: String,
  pub prompt: String,
  pub model: String,
  pub effort: Option<String>,
  pub placement: Option<String>,
  pub window: Option<Window>,
  /// The Claude login the run goes out on, where it goes out on one of the deployment's.
  pub account: Option<String>,
  /// The moment the turn took its words, as its variables carry it.
  pub began: i64,
}

/// What a run on a registration was sized to: its window, and the home its program keeps its records under.
pub struct Window {
  pub context_tokens: i64,
  pub home: PathBuf,
}

/// What the place's settings ask of the program: the shell guard every conversational agent's program
/// carries, and a person's typed line put in as typed, since only the chat agent's typed lines are
/// composed.
pub fn holds() -> Holds {
  Holds {
    shell_guard: true,
    ..Holds::default()
  }
}

/// What the run is handed of the settings: the answer ceiling where they name one, and the rest as its
/// sampling, a setting the chat template reads under the template's own field.
pub fn generation_run(values: &IndexMap<String, Value>) -> (Option<i64>, IndexMap<String, Value>) {
  let generation = &policy::companions::GENERATION;
  let mut ceiling = None;
  let mut sampling = IndexMap::new();
  let mut template = IndexMap::new();

  for (name, value) in values {
    if name == "max_tokens" {
      ceiling = value.as_i64().or_else(|| value.as_f64().map(|value| value as i64));
      continue;
    }

    match generation.setting(name) {
      Some(setting) if setting.template => {
        template.insert(name.clone(), value.clone());
      }
      _ if value.is_number() => {
        sampling.insert(name.clone(), value.clone());
      }
      _ => {}
    }
  }

  if !template.is_empty() {
    sampling.insert("chat_template_kwargs".to_owned(), serde_json::to_value(&template).unwrap_or_default());
  }

  (ceiling, sampling)
}

/// The login the walk offers an agent now on Claude Code's own road, none where it carries no login of the
/// deployment's, and the refusal where every login it carries is closed.
async fn walked_login(ground: &Ground, agent: &str) -> Result<Option<Account>, Refusal> {
  let login = claude_accounts::walk_for(&ground.db, agent)
    .await
    .map_err(|refused| fault(refused.error))?
    .into_iter()
    .next();

  if login.is_none()
    && !claude_accounts::carried_for_user(&ground.db, agent, None)
      .await
      .map_err(|refused| fault(refused.error))?
      .is_empty()
  {
    return Err(fault(format!("{agent}: no Claude login answers now")));
  }

  Ok(login)
}

/// The registration a run names, with an address, or the refusal naming it.
pub async fn registration_named(ground: &Ground, model: &str) -> Result<Registration, Refusal> {
  registry::registration(&ground.db, model)
    .await
    .map_err(|refused| fault(refused.error))?
    .filter(|held| !held.addresses.is_empty())
    .ok_or_else(|| fault(format!("{}: {model}", REGISTERED.presence.not_registered)))
}

/// The turn of one companion, composed.
pub async fn compose(
  ground: &Ground,
  machine: &Machine,
  turn: &CompanionTurn<'_>,
) -> Result<ComposedTurn, Refusal> {
  let trees = Trees::of(ground);
  let reasoning = reasoning_of(ground, turn.agent).await?;
  let thinking = reasoning.thinking.unwrap_or(false);
  let effort = effort_of(ground, machine, turn.provider, turn.model, None, Some(&reasoning))
    .await?
    .unwrap_or_default();
  let system = system_prompt_file(&trees, &format!("companion-{}", turn.agent), turn.system)?;
  let roots = companion_prompt::Roots {
    assets: trees.code.clone(),
    data: trees.data.clone(),
  };
  let folder = companion_prompt::folder(&roots, turn.agent);

  std::fs::create_dir_all(&folder).map_err(|why| fault(format!("{}: {why}", folder.display())))?;

  let cwd = folder.to_string_lossy().into_owned();
  let held = match turn.provider {
    REGISTERED_PROVIDER => Some(registration_named(ground, turn.model).await?),
    _ => None,
  };
  // On Claude Code's own road the program goes out on the login the walk offers the companion now, as every
  // Claude run does, and never on one that cannot answer: where none answers, the turn is not run.
  let login = match held {
    Some(_) => None,
    None => walked_login(ground, turn.agent).await?,
  };
  // A program asked to resume a session it keeps no record of ends before it reads a word: such a session
  // is refused in the program's own words, which the turn reads as a session gone and opens a new one
  // for. The record stands under the home the program keeps its records in: the registration's, the
  // login's directory, or the machine's own.
  let home = match (&held, &login) {
    (Some(held), _) => Registered::home(machine, &held.name),
    (None, Some(login)) if login.credential_kind != "token" => PathBuf::from(&login.credential_path),
    _ => ClaudeCode::machine_config_dir(machine),
  };

  if let Some((id, _)) = turn.resume
    && !ClaudeCode::records_under(&home, &folder, id).iter().any(|record| record.exists())
  {
    return Err(fault(format!("{LOST_SESSION}: {id}")));
  }

  let mcp = super::chat::mcp_config(ground, &trees)?;
  let (ceiling, sampling) = generation_run(turn.generation);
  let session = match turn.resume {
    Some((id, _)) => Session::Resume(id.to_owned()),
    None => Session::Open(uuid::Uuid::new_v4().to_string()),
  };
  let line = |model: &str, effort: &str, settings: &std::path::Path| {
    standing_words(
      &Words {
        model,
        effort,
        system: &system,
        mcp: &mcp,
        shown: &CHAT_LINE.tools.shown,
        allowed: &CHAT_LINE.tools.allowed,
        host_commands: true,
      },
      settings,
      &session,
    )
  };
  // The turn's own variables, which the place holds for the run while the program keeps none of them.
  let variables = [
    ("BMO_TURN_CHAT", turn.chat_id.to_string()),
    ("BMO_TURN_AGENT", turn.agent.to_owned()),
    (
      "BMO_TURN_MESSAGES",
      turn.answers.iter().map(i64::to_string).collect::<Vec<_>>().join(","),
    ),
    ("BMO_TURN_PRIVILEGED", if turn.privileged { "1" } else { "0" }.to_owned()),
    ("BMO_TURN_BEGAN", turn.began.to_string()),
  ];

  let Some(held) = held else {
    let place = Place::of(&trees.data, turn.agent);
    let settings =
      write_place_settings(&place.path, serde_json::Map::new(), &status_root(ground), holds())?;
    let mut env =
      command_env(ground.env(), login.as_ref(), &Runs::of_place(&trees.data, &place.path), thinking)?;

    env.extend(variables.map(|(name, value)| (name.to_owned(), Some(value))));

    let command = Command {
      args: line(turn.model, &effort, &settings)?,
      cwd: cwd.clone(),
      env,
    };
    let env = line_environment(&command, turn.run);

    return Ok(ComposedTurn {
      system_path: system,
      command: ResidentCommand {
        args: command.args,
        cwd: PathBuf::from(&cwd),
        env,
      },
      session: session.id().to_owned(),
      prompt: turn.prompt.to_owned(),
      model: turn.model.to_owned(),
      effort: Some(effort),
      placement: None,
      window: None,
      account: login.map(|login| login.id),
      began: turn.began,
    });
  };

  // A session it resumes stands where it stood; a fresh one is placed where the registration places it and
  // kept as standing there. A resumed one that stood nowhere in particular is placed for its requests
  // alone.
  let (placement, kept) = match turn.resume {
    Some((_, Some(node))) => (Some(node.to_owned()), Some(node.to_owned())),
    Some((_, None)) => (super::standing::place_registered(ground, turn.model).await?, None),
    None => {
      let node = super::standing::place_registered(ground, turn.model).await?;

      (node.clone(), node)
    }
  };

  if let Some(node) = &placement
    && !held.addresses.iter().any(|address| &address.node == node)
  {
    return Err(fault(fill_marks(
      &REGISTERED.presence.node_missing,
      &[("name", &held.name), ("node", node)],
    )));
  }

  let sampled = (!sampling.is_empty())
    .then(|| serde_json::to_string(&sampling))
    .transpose()
    .map_err(|why| fault(why.to_string()))?;
  let on = OnGateway {
    agent: turn.agent,
    model: turn.model,
    effort: &effort,
    cwd: &cwd,
    settings: serde_json::Map::new(),
    compose_typed: false,
    shell_guard: true,
    sampling: sampled,
  };
  let (mut command, _) =
    gateway_command(ground, machine, on, ceiling, Placing::At(placement), |name, effort, _, settings| {
      line(name, effort, settings)
    })
    .await?;

  command.env.extend(variables.map(|(name, value)| (name.to_owned(), value)));

  Ok(ComposedTurn {
    system_path: system,
    command,
    session: session.id().to_owned(),
    prompt: turn.prompt.to_owned(),
    model: turn.model.to_owned(),
    effort: (!effort.is_empty()).then_some(effort),
    placement: kept,
    window: Some(Window {
      context_tokens: held.context_tokens,
      home,
    }),
    account: None,
    began: turn.began,
  })
}
