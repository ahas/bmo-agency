//! A window-shaped conversational agent's turn as a run of the `chat` line (docs/launch-lines.md): the
//! model and effort the agent's own key is mapped to on Claude Code with the agent's own thinking and
//! effort, the system prompt of the chat's documents and rulebooks with the base persona after them, the
//! room's block and its mark, the same tools in every room with the whole of the deployment's own server
//! allowed, the one MCP configuration, the digest of what that server shows, and the room, the road and
//! the standing of the turn in the run's own variables. The room's block, the road that answers and the
//! base persona are the caller's: agency composes the line around them and judges none of them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_providers::Machine;
use policy::chat_line::CHAT_LINE;
use policy::providers::claude_code::CLAUDE_CODE;
use serde::Serialize;
use sha2::{Digest, Sha256};

use agent_host::place::ResidentCommand;
use harness_providers::claude_code::ClaudeCode;

use super::claude::{Command, Runs, Session, command_env, handed, line_environment};
use super::companion::{ComposedTurn, LOST_SESSION, walked_login};
use super::place::{Holds, Place, write_place_settings};
use super::runner::{handed_config, line_servers};
use super::standing::status_root;
use super::token::facts;
use super::worker::{Reasoning, effort_of, reasoning_of, rulebook, system_prompt_file};
use super::{Trees, fault, read_required};
use crate::Refusal;
use crate::control::tiers;
use crate::places::Ground;

/// The line the deployment's chat goes out on.
pub const LINE: &str = "chat";

// The provider the chat's own line is mapped on.
const CLAUDE: &str = "claude-code";

// The tool host's namespace the deployment's own server is shown under.
const SERVER: &str = "bmo";

/// The room's block as the caller composed it for the turn: the room it is, its text, and the mark the
/// turn's answer carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomContext {
  pub chat_id: i64,
  pub text: String,
  pub mark: String,
}

/// One turn's launch as the round asks for it: the agent whose turn it is, the room and its block, the road
/// answering it where that is not the room's own, the session it continues, whether every new message of it
/// is the owner's, whether it is the keepalive's touch, and the base persona that follows the system
/// prompt, where the caller keeps one.
#[derive(Clone)]
pub struct ChatLaunch<'a> {
  pub agent: &'a str,
  pub chat_id: i64,
  pub room: RoomContext,
  pub via: Option<&'a str>,
  pub resume: Option<String>,
  pub privileged: bool,
  pub ping: bool,
  pub base_persona: Option<&'a str>,
}

/// The run composed.
#[derive(Clone, Debug)]
pub struct ChatRun {
  /// The agent whose turn it is, whose work key, place, lock and records the run stands under.
  pub agent: String,
  pub model: String,
  pub effort: String,
  pub tier: Option<String>,
  /// The deployment's own choice of thinking and effort, as its row names them.
  pub reasoning: Reasoning,
  /// Whether the run thinks: the deployment's own choice, or yes.
  pub thinking: bool,
  pub system_path: PathBuf,
  pub room: RoomContext,
  pub tools_shown: String,
  pub tools_allowed: String,
  pub mcp_config: PathBuf,
  /// The digest of what the deployment's own server shows, where the tool host says it.
  pub mcp_tools: Option<String>,
  pub prompt: String,
  /// A fold: the fold word put into the program as its turn.
  pub fold: bool,
  pub ping: bool,
  pub resume: Option<String>,
  pub cwd: String,
  pub env: BTreeMap<String, String>,
}

/// The chat's system prompt: its opening document, the absolute rules bare, its manual and the rulebooks it
/// reads, each named, with the base persona after them all under its header where one is handed.
pub fn chat_system(trees: &Trees, base_persona: Option<&str>) -> Result<String, Refusal> {
  let system = &CHAT_LINE.system;
  let chat = trees.agent_dir("chat");
  let mut parts = vec![
    read_required(&chat.join(&system.opening))?,
    read_required(&trees.code.join("prompts").join("rules").join(&system.absolute_rules))?,
    read_required(&chat.join(&system.manual))?,
  ];

  for name in &system.rulebooks {
    parts.push(rulebook(trees, name)?);
  }

  let system = parts.join("\n\n");

  Ok(match base_persona.map(str::trim).filter(|persona| !persona.is_empty()) {
    Some(persona) => format!("{system}\n\n{}\n{persona}", CHAT_LINE.persona.header),
    None => system,
  })
}

/// The digest of what the deployment's own server shows, where the tool host says it.
pub async fn server_digest(ground: &Ground) -> Option<String> {
  facts(ground, SERVER).await?.get("digest")?.as_str().map(str::to_owned)
}

/// The chat line's MCP configuration: the deployment's own server and the toolbook, on the tool host,
/// written where the line reads it when nothing has written it and left as it stands otherwise.
pub fn mcp_config(ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal> {
  handed_config(trees.agents.join("chat.mcp.json"), || Ok(line_servers(ground, &[SERVER])))
}

/// The run a turn of the chat goes out on, its prompt as it is handed in.
pub async fn chat_run(
  ground: &Ground,
  machine: &Machine,
  launch: &ChatLaunch<'_>,
  prompt: &str,
  fold: bool,
) -> Result<ChatRun, Refusal> {
  let trees = Trees::of(ground);
  let agent = launch.agent;
  let mapping = tiers::resolve(&ground.db, agent, CLAUDE).await.map_err(fault)?;
  let reasoning = reasoning_of(ground, agent).await?;
  let effort =
    effort_of(ground, machine, CLAUDE, &mapping.model, mapping.effort.as_deref(), Some(&reasoning))
      .await?
      .unwrap_or_default();
  let thinking = reasoning.thinking.unwrap_or(true);
  let system_path = system_prompt_file(&trees, agent, &chat_system(&trees, launch.base_persona)?)?;
  let mcp = mcp_config(ground, &trees)?;
  let mut env: BTreeMap<String, String> = ground.env().clone();

  for (name, value) in &CLAUDE_CODE.launch.variables {
    env.insert(name.clone(), value.clone());
  }

  // What the turn is about, for what it runs to read without being told; a keepalive touch answers no room.
  if !launch.ping {
    env.insert("BMO_TURN_CHAT".to_owned(), launch.chat_id.to_string());
    env.insert("BMO_TURN_AGENT".to_owned(), agent.to_owned());
    env.insert("BMO_TURN_PRIVILEGED".to_owned(), if launch.privileged { "1" } else { "0" }.to_owned());

    if let Some(via) = launch.via {
      env.insert("BMO_TURN_VIA".to_owned(), via.to_owned());
    }
  }

  if !thinking {
    env.insert("MAX_THINKING_TOKENS".to_owned(), "0".to_owned());
  }

  Ok(ChatRun {
    agent: agent.to_owned(),
    model: mapping.model,
    effort,
    tier: Some(mapping.tier),
    reasoning,
    thinking,
    system_path,
    room: launch.room.clone(),
    tools_shown: CHAT_LINE.tools.shown.clone(),
    tools_allowed: CHAT_LINE.tools.allowed.clone(),
    mcp_config: mcp,
    mcp_tools: server_digest(ground).await,
    prompt: prompt.to_owned(),
    fold,
    ping: launch.ping,
    resume: launch.resume.clone(),
    cwd: trees.source.to_string_lossy().into_owned(),
    env,
  })
}

/// One turn of the chat line's agent across rooms, composed for the resident program standing in its place
/// (docs/api.md, 「Sessions」): the chat's run around the caller's room block, road and base persona, on
/// the login the walk offers the agent now, on the session the launch resumes or a new one. A session it
/// resumes that its program keeps no record of is refused as lost, which the turn reads as a session gone.
/// The messages the turn answers and the moment it took its words go among its own variables, and its run's
/// name last.
pub async fn compose_turn(
  ground: &Ground,
  machine: &Machine,
  launch: &ChatLaunch<'_>,
  prompt: &str,
  run_name: &str,
  answers: &[i64],
  began: i64,
) -> Result<ComposedTurn, Refusal> {
  let run = chat_run(ground, machine, launch, prompt, false).await?;
  let trees = Trees::of(ground);
  let login = walked_login(ground, launch.agent).await?;
  let home = match &login {
    Some(login) if login.credential_kind != "token" => PathBuf::from(&login.credential_path),
    _ => ClaudeCode::machine_config_dir(machine),
  };

  if let Some(id) = &run.resume
    && !ClaudeCode::records_under(&home, Path::new(&run.cwd), id)
      .iter()
      .any(|record| record.exists())
  {
    return Err(fault(format!("{LOST_SESSION}: {id}")));
  }

  let opened = uuid::Uuid::new_v4().to_string();
  let place = Place::of(&trees.data, launch.agent);
  let holds = Holds {
    shell_guard: true,
    compose_typed: true,
    ..Holds::default()
  };
  let settings = write_place_settings(&place.path, serde_json::Map::new(), &status_root(ground), holds)?;
  let mut env =
    command_env(&run.env, login.as_ref(), &Runs::of_place(&trees.data, &place.path), run.thinking)?;

  env.insert(
    "BMO_TURN_MESSAGES".to_owned(),
    Some(answers.iter().map(i64::to_string).collect::<Vec<_>>().join(",")),
  );
  env.insert("BMO_TURN_BEGAN".to_owned(), Some(began.to_string()));

  let command = Command {
    args: resident_args(&run, &settings, &opened)?,
    cwd: run.cwd.clone(),
    env,
  };
  let env = line_environment(&command, run_name);

  Ok(ComposedTurn {
    system_path: run.system_path.clone(),
    command: ResidentCommand {
      args: command.args,
      cwd: PathBuf::from(&run.cwd),
      env,
    },
    session: run.resume.clone().unwrap_or(opened),
    prompt: prompt.to_owned(),
    model: run.model.clone(),
    effort: (!run.effort.is_empty()).then(|| run.effort.clone()),
    placement: None,
    window: None,
    account: login.map(|login| login.id),
    began,
  })
}

/// What one conversational agent's program on the chat line is started with, whichever agent it is
/// (docs/agents.md, 「Conversational agents」): the model and effort, its own composed prompt, the line's
/// one configuration, what the line shows and allows, and whether the program's own commands are reachable.
pub struct Words<'a> {
  pub model: &'a str,
  pub effort: &'a str,
  pub system: &'a Path,
  pub mcp: &'a Path,
  pub shown: &'a str,
  pub allowed: &'a str,
  pub host_commands: bool,
}

/// The line's words for the interactive program: the model and effort, the composed prompt in place of the
/// program's own, nothing of this machine but the one configuration handed, the program's own commands off
/// where they are not reachable, and what it is shown and allowed.
pub fn words(line: &Words<'_>) -> Result<Vec<String>, Refusal> {
  let mut args: Vec<String> = vec!["claude".to_owned()];

  args.extend([
    "--model".to_owned(),
    line.model.to_owned(),
    "--effort".to_owned(),
    line.effort.to_owned(),
  ]);
  args.extend(["--system-prompt-file".to_owned(), line.system.to_string_lossy().into_owned()]);
  args.extend(["--setting-sources", "", "--strict-mcp-config", "--mcp-config"].map(str::to_owned));
  args.push(handed(LINE, line.mcp)?);

  if !line.host_commands {
    args.push("--disable-slash-commands".to_owned());
  }

  args.extend(["--no-chrome", "--permission-mode", "dontAsk", "--tools"].map(str::to_owned));
  args.push(line.shown.to_owned());
  args.push("--allowedTools".to_owned());
  args.push(line.allowed.to_owned());

  Ok(args)
}

/// The words of a conversational agent's resident program: the line with its own commands reachable, since
/// a person may type into it, the place's settings, and the session it resumes or opens.
pub fn standing_words(
  line: &Words<'_>,
  settings: &Path,
  session: &Session,
) -> Result<Vec<String>, Refusal> {
  let mut args = words(&Words {
    host_commands: true,
    ..*line
  })?;

  args.extend(["--settings".to_owned(), handed(LINE, settings)?]);

  match session {
    Session::Resume(id) => args.extend(["--resume".to_owned(), id.clone()]),
    Session::Open(id) => args.extend(["--session-id".to_owned(), id.clone()]),
  }

  Ok(args)
}

impl ChatRun {
  // The run's line, its own commands reachable, since a person may type into the deployment's program and a
  // fold is put into it.
  fn line(&self) -> Words<'_> {
    Words {
      model: &self.model,
      effort: &self.effort,
      system: &self.system_path,
      mcp: &self.mcp_config,
      shown: &self.tools_shown,
      allowed: &self.tools_allowed,
      host_commands: true,
    }
  }
}

/// The run's line as the deployment's resident program is started on it, before its settings and its
/// session.
pub fn args(run: &ChatRun) -> Result<Vec<String>, Refusal> {
  words(&run.line())
}

/// The words of the resident program standing in the deployment's pane: the run's line, the place's
/// settings, and the session it resumes or the one it opens.
pub fn resident_args(run: &ChatRun, settings: &Path, opened: &str) -> Result<Vec<String>, Refusal> {
  let session = match &run.resume {
    Some(resumed) => Session::Resume(resumed.clone()),
    None => Session::Open(opened.to_owned()),
  };

  standing_words(&run.line(), settings, &session)
}

/// The room's block as a run carries it, for a road that hands the run on whole.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomBlock<'a> {
  pub chat_id: i64,
  pub text: &'a str,
  pub mark: &'a str,
}

/// The deployment's own choice of thinking and effort as a run carries it.
#[derive(Serialize)]
pub struct RunReasoning<'a> {
  pub thinking: Option<bool>,
  pub effort: Option<&'a str>,
}

/// The chat's run as a stand-in road takes it, in the fields and the order the TypeScript harness
/// (`agents/agent`) describes a run in: what a Codex thread is kept with and a warm OpenRouter run warmed
/// from, and what the deployment's loop is handed. The prompt, the session it resumes and the variables are
/// each turn's own, and are left out of what is kept.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandIn<'a> {
  pub line: &'a str,
  pub model: &'a str,
  pub effort: &'a str,
  pub system_prompt_path: &'a std::path::Path,
  pub room_context: RoomBlock<'a>,
  pub tools_shown: &'a str,
  pub tools_allowed: &'a str,
  pub tools_denied: Option<&'a str>,
  pub mcp_config_path: &'a std::path::Path,
  pub mcp_tools: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub prompt: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ping: Option<bool>,
  pub prompt_delivery: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resume_session_id: Option<Option<&'a str>>,
  pub keep_session: bool,
  pub host_commands: bool,
  pub reasoning: RunReasoning<'a>,
  pub thinking: bool,
  pub answer_shape: &'a str,
  pub cwd: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub env: Option<&'a BTreeMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tier: Option<&'a str>,
}

impl ChatRun {
  /// The run as a stand-in road takes it, on the model and effort the walk chose, with the prompt, the
  /// session and the variables each turn hands it where they are given.
  pub fn stand_in<'a>(
    &'a self,
    model: &'a str,
    effort: &'a str,
    delivery: &'a str,
    keep_session: bool,
  ) -> StandIn<'a> {
    StandIn {
      line: LINE,
      model,
      effort,
      system_prompt_path: &self.system_path,
      room_context: RoomBlock {
        chat_id: self.room.chat_id,
        text: &self.room.text,
        mark: &self.room.mark,
      },
      tools_shown: &self.tools_shown,
      tools_allowed: &self.tools_allowed,
      tools_denied: None,
      mcp_config_path: &self.mcp_config,
      mcp_tools: self.mcp_tools.as_deref(),
      prompt: None,
      ping: self.ping.then_some(true),
      prompt_delivery: delivery,
      resume_session_id: None,
      keep_session,
      host_commands: self.fold,
      reasoning: RunReasoning {
        thinking: self.reasoning.thinking,
        effort: self.reasoning.level.as_deref(),
      },
      thinking: self.thinking,
      answer_shape: "text",
      cwd: &self.cwd,
      env: None,
      tier: self.tier.as_deref(),
    }
  }
}

impl StandIn<'_> {
  /// What is kept of the run: everything but the prompt, the session and the variables.
  pub fn stored(&self) -> String {
    serde_json::to_string(self).unwrap_or_default()
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Prefix<'a> {
  mode: &'a str,
  line: &'a str,
  model: &'a str,
  effort: &'a str,
  thinking: bool,
  host_commands: bool,
  system: Option<String>,
  tools_shown: &'a str,
  tools_allowed: &'a str,
  tools_denied: Option<&'a str>,
  mcp: Option<String>,
  mcp_tools: Option<&'a str>,
  cwd: &'a str,
}

/// What a session of the chat stands on, as one digest: every byte of its line that its cached prefix is
/// made of. The room's block is not in it, since changing rooms does not change the prefix.
pub fn prefix(run: &ChatRun, mode: &str) -> String {
  let file = |path: &PathBuf| std::fs::read_to_string(path).ok();
  let prefix = Prefix {
    mode,
    line: LINE,
    model: &run.model,
    effort: &run.effort,
    thinking: run.thinking,
    host_commands: run.fold,
    system: file(&run.system_path),
    tools_shown: &run.tools_shown,
    tools_allowed: &run.tools_allowed,
    tools_denied: None,
    mcp: file(&run.mcp_config),
    mcp_tools: run.mcp_tools.as_deref(),
    cwd: &run.cwd,
  };

  format!("{:x}", Sha256::digest(serde_json::to_string(&prefix).unwrap_or_default().as_bytes()))
}
