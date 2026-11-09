//! A window-shaped conversational agent's turn as a run of the `chat` line (docs/launch-lines.md): the
//! model and effort the agent's own key is mapped to on Claude Code with the agent's own thinking and
//! effort, the system prompt of the chat's documents and rulebooks with the base persona after them, the
//! room's block and its mark, the same tools in every room with the whole of the deployment's own server
//! allowed, the one MCP configuration, the digest of what that server shows, and the room, the road and
//! the standing of the turn in the run's own variables. The base persona is the agent's brain's base layer,
//! read once at the launch (`super::persona`), else the caller's. The room's block, the road that answers
//! and the persona the caller hands in are the caller's: agency composes the line around them and judges
//! none of them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_session::GateAsk;
use harness_providers::Machine;
use policy::chat_line::CHAT_LINE;
use policy::providers::claude_code::CLAUDE_CODE;
use serde::Serialize;
use sha2::{Digest, Sha256};

use agent_host::place::ResidentCommand;
use harness_providers::claude_code::ClaudeCode;
use policy::run_lines::RUN_LINES;

use super::claude::{Command, Runs, Session, command_env, handed, line_environment};
use super::codex::{Shape, resident_command_of};
use super::dsh::Composed;
use super::openrouter::resident_command;
use super::ordered::Ordered;
use super::pieces::{ComposedTurn, LOST_SESSION, Window, walked_login};
use super::place::{Holds, Place, read_settings, write_place_settings};
use super::standing::{GatewayModel, OnGateway, Placing, gateway_command, status_root};
use super::token::{self, facts};
use super::worker::{Reasoning, WorkerRun, effort_of, reasoning_of, rulebook, system_prompt_file};
use super::{Trees, fault, publish_text, read_required};
use crate::Refusal;
use crate::control::pool::native_provider;
use crate::places::{Ground, file_variable, token_or_file};
use crate::run_log::RunLog;
use crate::run_log::lines::Line;
use crate::stand_in::Step;
use crate::verbs::CONTROL_TOKEN;

/// The line the deployment's chat goes out on.
pub const LINE: &str = "chat";

// The provider the chat's own line is mapped on.
const CLAUDE: &str = "claude-code";

// The tool host's namespace the deployment's own digest is read under.
const SERVER: &str = "bmo";

// The holder the harness plugins report and subscribe under. It is the line's own role, the same in
// every supervisor that stands on it, and never an agent's name.
const HARNESS: &str = "harness";

/// The chat line's denied list (`policy/run-lines.toml`): the one restriction on a supervisor's program, which
/// runs in bypass-permissions mode over every other tool of its own on every road (docs/launch-lines.md,
/// 「What a line's program is allowed, by road」).
pub fn denied() -> String {
  RUN_LINES.line(LINE).map(|line| line.tools_denied.join(",")).unwrap_or_default()
}

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
/// is the owner's, whether it is the keepalive's touch, and the base persona the caller keeps, which follows
/// the system prompt where the agent's brain gives none.
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
  /// The model the turn goes out on where the caller names one rather than the agent's key.
  pub model: Option<&'a str>,
  /// The folder the turn works in where the caller names one.
  pub cwd: Option<&'a str>,
}

/// The run composed.
#[derive(Clone, Debug)]
pub struct ChatRun {
  /// The line the run composes under — `chat`, `gate` or `worker` — which its stand-in roads name its
  /// program, prompt and files by. The fallback mechanism is one; only the line it runs differs.
  pub line: &'static str,
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
  /// The SHA-256 of the system prompt as composed, which the next turn compares against the one its session
  /// was last sent with (docs/supervisors.md, 「A session in each room」).
  pub system_hash: String,
  /// What the chat's system prompt was composed of, where the run is the chat's, so a road on another
  /// program composes it with that program's tools named; none for a run handed its prompt whole.
  pub system_parts: Option<SystemParts>,
  pub room: RoomContext,
  /// What a program that is told its tools is shown and allowed; on Claude Code, what a run in don't-ask mode
  /// is shown and allowed.
  pub tools_shown: String,
  pub tools_allowed: String,
  /// What the program is denied where it runs in bypass-permissions mode over every other tool of its own: a
  /// supervisor's, on every road whose program is Claude Code (docs/launch-lines.md, 「What a line's program is
  /// allowed, by road」); none for a run in don't-ask mode over what it is shown and allowed.
  pub tools_denied: Option<String>,
  pub mcp_config: PathBuf,
  /// The digest of what the deployment's own server shows, where the tool host says it.
  pub mcp_tools: Option<String>,
  pub prompt: String,
  /// Why the system prompt does not end with the brain's base persona, where it does not.
  pub persona_unused: Option<String>,
  /// A fold: the fold word put into the program as its turn.
  pub fold: bool,
  pub ping: bool,
  pub resume: Option<String>,
  pub cwd: String,
  pub env: BTreeMap<String, String>,
  /// The demands of its own the run's caller handed in, on a registration whose engine the arbiter
  /// stands (docs/own-llm.md, 「An engine the arbiter stands」).
  pub cards: crate::cards::Ask,
  /// The worker line whole, where the run is a role's run on it: a program that is Claude Code is started on
  /// the worker line's own words — its permission mode, denied tools, settings, plugin, co-workers and MCP
  /// configuration — on every road, never on the chat line's (docs/launch-lines.md, 「What a line's program is
  /// allowed, by road」).
  pub worker: Option<Box<WorkerRun>>,
}

/// The owner's own text of each of the chat's documents, by the name the store keeps (`chat_documents`): the
/// opening, the absolute rules and the manual, each standing in place of its file where one is kept
/// (docs/agent-documents.md, 「A supervisor of the window shape」).
#[derive(Clone, Debug, Default)]
pub struct ChatDocuments {
  pub opening: Option<String>,
  pub absolute_rules: Option<String>,
  pub manual: Option<String>,
}

impl ChatDocuments {
  /// What the owner keeps under one document's name, or none.
  pub fn of(&self, name: &str) -> Option<&str> {
    match name {
      "opening" => self.opening.as_deref(),
      "absolute-rules" => self.absolute_rules.as_deref(),
      "manual" => self.manual.as_deref(),
      _ => None,
    }
  }
}

/// The chat's documents as the store keeps them, in one read; a store that cannot be read leaves every
/// document as its file stands.
pub async fn documents(db: &impl sea_orm::ConnectionTrait) -> ChatDocuments {
  let Ok(rows) = store::all(db, r#"SELECT "name", "text" FROM "chat_documents""#, vec![]).await else {
    return ChatDocuments::default();
  };
  let mut kept = ChatDocuments::default();

  for row in rows {
    let (Ok(name), Ok(text)) = (row.try_get::<String>("", "name"), row.try_get::<String>("", "text"))
    else {
      continue;
    };

    match name.as_str() {
      "opening" => kept.opening = Some(text),
      "absolute-rules" => kept.absolute_rules = Some(text),
      "manual" => kept.manual = Some(text),
      _ => {}
    }
  }

  kept
}

/// The chat's system prompt: its opening document, the absolute rules bare, its manual, the tools paragraph
/// for the program the session runs on where one is composed, and the rulebooks it reads, each named, the
/// brain's discipline where the brain answered one, the tool host's paragraph on finding and using tools
/// where the host answered one, with the base persona after them all under its header where one is handed.
/// A document the owner keeps a text of stands as that text, and its file stands where none is kept.
pub fn chat_system(
  trees: &Trees,
  documents: &ChatDocuments,
  base_persona: Option<&str>,
  discipline: Option<&str>,
  tool_discipline: Option<&str>,
  tools: Option<&str>,
) -> Result<String, Refusal> {
  let system = &CHAT_LINE.system;
  let chat = trees.agent_dir("chat");
  let document = |name: &str, path: PathBuf| -> Result<String, Refusal> {
    match documents.of(name) {
      Some(text) => Ok(text.to_owned()),
      None => read_required(&path),
    }
  };
  let mut parts = vec![
    document("opening", chat.join(&system.opening))?,
    document("absolute-rules", trees.code.join("prompts").join("rules").join(&system.absolute_rules))?,
    document("manual", chat.join(&system.manual))?,
  ];

  if let Some(tools) = tools.map(str::trim).filter(|text| !text.is_empty()) {
    parts.push(tools.to_owned());
  }

  for name in &system.rulebooks {
    parts.push(rulebook(trees, name)?);
  }
  for discipline in [discipline, tool_discipline] {
    if let Some(discipline) = discipline.map(str::trim).filter(|text| !text.is_empty()) {
      parts.push(discipline.to_owned());
    }
  }

  let system = parts.join("\n\n");

  Ok(match base_persona.map(str::trim).filter(|persona| !persona.is_empty()) {
    Some(persona) => format!("{system}\n\n{}\n{persona}", CHAT_LINE.persona.header),
    None => system,
  })
}

/// The chat's tools paragraph (docs/agent-documents.md, 「A supervisor of the window shape」): the program's own tools by name,
/// the servers the line's MCP configuration hands the session, and the tool host's entries the line shows by
/// name, each as a program calls it, in the words the chat line's policy holds.
pub fn tools_paragraph(program: &[&str], servers: &[String], shown: &[String]) -> String {
  let shown: Vec<String> = shown
    .iter()
    .map(|entry| match entry.split_once('/') {
      Some((namespace, tool)) => format!("mcp__{namespace}__{tool}"),
      None => entry.clone(),
    })
    .collect();

  policy::fill_marks(
    &CHAT_LINE.tools.paragraph,
    &[
      ("program", &program.join(", ")),
      ("servers", &servers.join(", ")),
      ("shown", &shown.join(", ")),
    ],
  )
}

/// What a chat run's system prompt is composed of beside the documents, kept so a road whose program is
/// not Claude Code's composes the same prompt with its own program's tools named.
#[derive(Clone, Debug, Default)]
pub struct SystemParts {
  pub above: Option<String>,
  pub persona: Option<String>,
  pub discipline: Option<String>,
  pub tool_discipline: Option<String>,
  pub servers: Vec<String>,
  /// The owner's texts of the chat's own documents, which stand in place of their files where kept.
  pub documents: ChatDocuments,
}

impl SystemParts {
  /// The chat's whole system prompt on Claude Code, whose own tools are these: the platform's harness
  /// opening, then the role and the persona (docs/agent-documents.md, 「A supervisor of the window shape」).
  pub fn whole(&self, trees: &Trees, program: &[&str]) -> Result<String, Refusal> {
    super::harnessed(trees, &self.compose(trees, program)?)
  }

  /// The chat's role and persona for a program whose own tools are these: what follows the harness, which
  /// a program that is a harness of its own, as dsh is, carries after its own prompt.
  pub fn compose(&self, trees: &Trees, program: &[&str]) -> Result<String, Refusal> {
    let tools = tools_paragraph(program, &self.servers, RUN_LINES.shown(LINE));
    let system = chat_system(
      trees,
      &self.documents,
      self.persona.as_deref(),
      self.discipline.as_deref(),
      self.tool_discipline.as_deref(),
      Some(&tools),
    )?;

    Ok(match &self.above {
      Some(above) => format!("{above}\n\n{system}"),
      None => system,
    })
  }
}

/// The digest of what the deployment's own server shows, where the tool host says it.
pub async fn server_digest(ground: &Ground) -> Option<String> {
  facts(ground, SERVER).await?.get("digest")?.as_str().map(str::to_owned)
}

/// The chat line's MCP configuration: the tool host's own namespaces, each behind the run's session token,
/// where a supervisor's sending, filing and memory tools stand; then the job service's harness plugin,
/// which leases an item from a queue, and the agency's own, which assigns a worker and launches the item's
/// job. A hosted tool those namespaces do not show is reached with `bmo use`. The two plugins start as
/// programs beside the chat and take their bearers from the environment the chat inherits; a deployment
/// that names neither leaves that server out. Every supervisor's program is handed the same servers, whatever
/// its shape (docs/supervisors.md, 「The bundles a supervisor holds」). Written on every composition, since this line is
/// the one writer.
pub fn mcp_config(ground: &Ground, trees: &Trees, agent: &str) -> Result<PathBuf, Refusal> {
  let path = trees.agents.join(format!("chat.{agent}.mcp.json"));
  let text = format!("{}\n", Ordered::map([("mcpServers", harness_servers(ground, agent))]).pretty());

  agent_host::disk::private_dir(&trees.agents)
    .map_err(|why| fault(format!("{}: {why}", trees.agents.display())))?;
  agent_host::disk::publish_bytes(&path, text.as_bytes())
    .map_err(|why| fault(format!("{}: {why}", path.display())))?;

  Ok(path)
}

/// What a supervisor's double is handed of the supervisor's servers (docs/worker-mcp.md, 「A supervisor's
/// double」): its MCP configuration. A program that is told its tools tells a double the supervisor's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Double {
  pub mcp_config: PathBuf,
}

/// The servers of `supervisor`'s double on a role line whose own configuration is `line_config`: the
/// supervisor's chat configuration, with each
/// server of the line's configuration the chat's holds none of beside it, written as
/// `double.<supervisor>.<line>.mcp.json` under the agents tree where it differs from what stands there.
pub async fn double_of(
  ground: &Ground,
  trees: &Trees,
  supervisor: &str,
  line: &str,
  line_config: &std::path::Path,
) -> Result<Double, Refusal> {
  crate::supervisors::named(&ground.db, supervisor)
    .await
    .map_err(|why| fault(format!("the supervisor {supervisor} could not be read: {why}")))?
    .ok_or_else(|| fault(format!("No supervisor named {supervisor} is held")))?;
  let mut servers: indexmap::IndexMap<String, Ordered> =
    harness_servers(ground, supervisor).entries().cloned().unwrap_or_default();
  let read: Ordered = serde_json::from_str(
    &std::fs::read_to_string(line_config)
      .map_err(|why| fault(format!("{}: {why}", line_config.display())))?,
  )
  .map_err(|why| fault(format!("{}: {why}", line_config.display())))?;
  for (name, server) in read
    .entries()
    .and_then(|top| top.get("mcpServers"))
    .and_then(Ordered::entries)
    .into_iter()
    .flatten()
  {
    servers.entry(name.clone()).or_insert_with(|| server.clone());
  }

  let path = trees.agents.join(format!("double.{supervisor}.{line}.mcp.json"));
  let text = format!("{}\n", Ordered::map([("mcpServers", Ordered::Map(servers))]).pretty());

  if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
    agent_host::disk::private_dir(&trees.agents)
      .map_err(|why| fault(format!("{}: {why}", trees.agents.display())))?;
    agent_host::disk::publish_bytes(&path, text.as_bytes())
      .map_err(|why| fault(format!("{}: {why}", path.display())))?;
  }

  Ok(Double { mcp_config: path })
}

// The shell in front of a stdio plugin that takes its job token from its environment alone: the token read from
// the file `BMO_JOB_TOKEN_FILE` names, and the plugin, its first word, started on its own words with it.
const TOKEN_FROM_FILE: &str = r#"BMO_JOB_TOKEN=$(cat "$BMO_JOB_TOKEN_FILE") || exit 1; export BMO_JOB_TOKEN; unset BMO_JOB_TOKEN_FILE; exec "$0" "$@""#;

// The chat's servers: the tool host's namespaces its line names, then the two the chat starts as programs of
// its own — the job plugin on its `mcp` verb, told the queues it subscribes to and the holder it takes them
// under, and the agency's own binary on the same verb.
// A value that differs by installation is read from the environment, and each program's bearer is handed to
// it in the configuration's own environment: a harness starts its children on an environment scrubbed of
// everything but the entries written there.
fn harness_servers(ground: &Ground, agent: &str) -> Ordered {
  let env = ground.env();
  let namespaces: Vec<&str> = RUN_LINES.namespaces(LINE).iter().map(String::as_str).collect();
  let mut servers: Vec<(String, Ordered)> = super::runner::line_servers(ground, &namespaces)
    .entries()
    .into_iter()
    .flatten()
    .map(|(name, server)| (name.clone(), server.clone()))
    .collect();

  if let (Some(plugin), Some(url)) = (env.get("BMO_JOB_PLUGIN"), env.get("BMO_JOB_URL")) {
    let mut args = vec![
      Ordered::text("mcp"),
      Ordered::text("--service"),
      Ordered::text(url),
      Ordered::text("--holder"),
      Ordered::text(HARNESS),
    ];

    // The plugin's own tools the line keeps from the chat, which the plugin then neither lists nor calls on
    // any program, by the names it serves them under.
    let denied = denied();
    let without: Vec<&str> =
      denied.split(',').filter_map(|tool| tool.trim().strip_prefix("mcp__job__")).collect();

    if !without.is_empty() {
      args.push(Ordered::text("--without"));
      args.push(Ordered::text(without.join(",")));
    }

    for queue in env
      .get("BMO_JOB_QUEUES")
      .into_iter()
      .flat_map(|queues| queues.split(','))
      .map(str::trim)
      .filter(|queue| !queue.is_empty())
    {
      args.push(Ordered::text("--queue"));
      args.push(Ordered::text(queue));
    }

    // The chat's own caller token where the deployment names one, so the job service records what the
    // chat does under the chat's name; the shared token otherwise. Each is handed as the file it stands in,
    // which a shell in front of the plugin reads, since the plugin takes its token from its environment
    // alone; one only held by value goes as it is (docs/agent-control.md, 「A token is handed as the file it
    // stands in」).
    let readable = |variable: &str| {
      env
        .get(&file_variable(variable))
        .filter(|file| token_or_file(None, Some((*file).clone())).is_some())
        .cloned()
    };
    let by_value = |variable: &str| env.get(variable).filter(|token| !token.trim().is_empty()).cloned();
    let fields = match readable("BMO_JOB_CHAT_TOKEN").or_else(|| readable("BMO_JOB_TOKEN")) {
      Some(file) => {
        let mut wrapped = vec![Ordered::text("-c"), Ordered::text(TOKEN_FROM_FILE), Ordered::text(plugin)];

        wrapped.extend(args);

        vec![
          ("type", Ordered::text("stdio")),
          ("command", Ordered::text("/bin/sh")),
          ("args", Ordered::List(wrapped)),
          ("env", Ordered::map([("BMO_JOB_TOKEN_FILE", Ordered::text(file))])),
        ]
      }
      None => {
        let mut fields = vec![
          ("type", Ordered::text("stdio")),
          ("command", Ordered::text(plugin)),
          ("args", Ordered::List(args)),
        ];

        if let Some(token) = by_value("BMO_JOB_CHAT_TOKEN").or_else(|| by_value("BMO_JOB_TOKEN")) {
          fields.push(("env", Ordered::map([("BMO_JOB_TOKEN", Ordered::text(token))])));
        }

        fields
      }
    };

    servers.push(("job".to_owned(), Ordered::map(fields)));
  }

  // The explorer service's harness plugin, first-class beside the artifact and job plugins: its six page
  // and search tools are not reached through the tool host's placed copies. The deployment names the
  // program and the data root its store reads; where it names neither, the server is left out.
  if let (Some(program), Some(root)) = (
    env.get("BMO_EXPLORER_PLUGIN").filter(|program| !program.trim().is_empty()),
    env.get("BMO_DATA_DIR").filter(|root| !root.trim().is_empty()),
  ) {
    let fields = vec![
      ("type", Ordered::text("stdio")),
      ("command", Ordered::text(program.clone())),
      (
        "args",
        Ordered::List(vec![
          Ordered::text("mcp"),
          Ordered::text("--data-root"),
          Ordered::text(root.clone()),
        ]),
      ),
      (
        "env",
        Ordered::map([("HOME", Ordered::text(std::env::var("HOME").unwrap_or_else(|_| "/".to_owned())))]),
      ),
    ];

    servers.push(("explorer".to_owned(), Ordered::map(fields)));
  }

  // The artifact service's harness plugin, which files a document and answers its permanent share link.
  // Its address and its token are the deployment's, read from the environment the chat inherits; a
  // deployment that names neither leaves it out.
  // The deploy tree's own wrapper reads the token's file; a plugin the deployment names is handed the value,
  // since agency cannot know it reads a file.
  if let (Some(url), Some(token_file)) = (
    env.get("BMO_ARTIFACT_URL").filter(|url| !url.trim().is_empty()),
    env.get("BMO_ARTIFACT_TOKEN_FILE").filter(|file| !file.trim().is_empty()),
  ) && let Some(token) = token_or_file(None, Some(token_file.clone()))
  {
    let named = env.get("BMO_ARTIFACT_PLUGIN").filter(|program| !program.trim().is_empty()).cloned();
    let token = match named {
      Some(_) => ("BMO_ARTIFACT_SERVER__TOKEN", Ordered::text(token)),
      None => ("BMO_ARTIFACT_TOKEN_FILE", Ordered::text(token_file.as_str())),
    };
    let program = named.unwrap_or_else(|| {
      ground
        .roots
        .deploy
        .join("deploy")
        .join("artifact-mcp.sh")
        .to_string_lossy()
        .into_owned()
    });
    let fields = vec![
      ("type", Ordered::text("stdio")),
      ("command", Ordered::text(program)),
      ("args", Ordered::List(vec![Ordered::text(url.as_str())])),
      (
        "env",
        Ordered::map([
          token,
          ("HOME", Ordered::text(std::env::var("HOME").unwrap_or_else(|_| "/".to_owned()))),
        ]),
      ),
    ];

    servers.push(("artifact".to_owned(), Ordered::map(fields)));
  }

  if let (Ok(server), Some(url)) = (std::env::current_exe(), env.get("BMO_AGENCY_URL")) {
    // The MCP server is the repository's CLI, which stands beside the server in the release's bin.
    let program = server
      .parent()
      .map(|bin| bin.join("bmo-agency"))
      .filter(|cli| cli.exists())
      .unwrap_or(server);

    let mut fields = vec![
      ("type", Ordered::text("stdio")),
      ("command", Ordered::text(program.to_string_lossy().into_owned())),
      (
        "args",
        Ordered::List(vec![
          Ordered::text("mcp"),
          Ordered::text("--url"),
          Ordered::text(url),
          Ordered::text("--agent"),
          Ordered::text(agent),
        ]),
      ),
    ];

    // The control token as the file it stands in, which the verb reads; by value only where the launch
    // holds it so.
    if let Some(file) = env.get(&file_variable(CONTROL_TOKEN)).filter(|file| !file.trim().is_empty()) {
      fields.push(("env", Ordered::map([(file_variable(CONTROL_TOKEN), Ordered::text(file.as_str()))])));
    } else if let Some(token) = env.get(CONTROL_TOKEN).filter(|token| !token.is_empty()) {
      fields.push(("env", Ordered::map([(CONTROL_TOKEN, Ordered::text(token.as_str()))])));
    }

    servers.push(("agency".to_owned(), Ordered::map(fields)));
  }

  Ordered::map(servers)
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
  let reasoning = reasoning_of(ground, agent).await?;
  let named = launch.model.map(str::trim).filter(|model| !model.is_empty());
  let mut held_thinking = None;
  let (model, effort, tier) = match named {
    // A model the caller named belongs to the provider it named beside it, which this line does not
    // know; its agent's own level is the whole of what the chat line hands it.
    Some(model) => (model.to_owned(), reasoning.level.clone().unwrap_or_default(), None),
    None => {
      let entry = crate::order::supervisor_entry(&ground.db, agent, CLAUDE).await.map_err(fault)?;
      let effort =
        effort_of(ground, machine, CLAUDE, &entry.model, entry.effort.as_deref(), Some(&reasoning))
          .await?
          .unwrap_or_default();

      held_thinking = entry.thinks();

      (entry.model, effort, None)
    }
  };
  let thinking = held_thinking.or(reasoning.thinking).unwrap_or(true);
  let discipline = match ground.peers.brain.discipline().await {
    Ok(text) => Some(text),
    Err(why) => {
      eprintln!("agency: the brain's discipline could not be read; the chat launches without it: {why}");
      None
    }
  };
  let tool_discipline = match super::token::discipline(ground).await {
    Ok(text) => Some(text),
    Err(why) => {
      eprintln!(
        "agency: the tool host's discipline could not be read; the chat launches without it: {why}"
      );
      None
    }
  };
  let persona = super::persona::base_persona(
    ground.peers.brain.as_ref(),
    agent,
    ground.config.persona_values.as_deref(),
    launch.base_persona,
  )
  .await;
  let role = crate::conversational::role_of(&ground.db, agent).await.ok().flatten();
  let above = match role.as_deref() {
    Some(role) => super::role_prompt(&ground.db, role, launch.cwd).await,
    None => None,
  };
  let documents = documents(&ground.db).await;
  let parts = SystemParts {
    above,
    persona: persona.text.clone(),
    discipline,
    tool_discipline,
    documents,
    servers: harness_servers(ground, agent)
      .entries()
      .into_iter()
      .flatten()
      .map(|(name, _)| name.clone())
      .collect(),
  };
  // Claude Code lists its own tools to the session, every one but the line's denied list.
  let system = parts.whole(&trees, &[CHAT_LINE.tools.listed.as_str()])?;
  let system_hash = format!("{:x}", Sha256::digest(system.as_bytes()));
  let system_path = system_prompt_file(&trees, agent, &system)?;
  let mcp = mcp_config(ground, &trees, agent)?;
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
    line: LINE,
    agent: agent.to_owned(),
    model,
    effort,
    tier,
    reasoning,
    thinking,
    system_path,
    system_hash,
    system_parts: Some(parts),
    room: launch.room.clone(),
    tools_shown: CHAT_LINE.tools.shown.clone(),
    // A program told its tools is allowed every one it is shown, and Claude Code denied the line's list alone.
    tools_allowed: CHAT_LINE.tools.shown.clone(),
    tools_denied: Some(denied()),
    mcp_config: mcp,
    mcp_tools: server_digest(ground).await,
    prompt: prompt.to_owned(),
    persona_unused: persona.unused,
    fold,
    ping: launch.ping,
    resume: launch.resume.clone(),
    // The folder a run works in: the caller's, else the deploy tree's link. The link's path is the same
    // across releases though its target moves, so the resident program's own record of its session — kept
    // under the folder it was stood in — is taken up again when the program is stood after a restart,
    // rather than a fresh session opened beside it (docs/sessions.md, 「A restart hands the session over」).
    cwd: launch
      .cwd
      .map(str::to_owned)
      .unwrap_or_else(|| ground.roots.deploy.to_string_lossy().into_owned()),
    env,
    cards: Default::default(),
    worker: None,
  })
}

impl ChatRun {
  /// The system prompt file of the run on a program that is a harness of its own and whose own tools are
  /// these, under the program's name: the chat's role and persona composed afresh with them named and no
  /// harness of the platform's before them, or the run's own file where it was handed its prompt whole.
  pub fn system_on(&self, ground: &Ground, program: &str, tools: &[&str]) -> Result<PathBuf, Refusal> {
    let Some(parts) = &self.system_parts else {
      return Ok(self.system_path.clone());
    };
    let trees = Trees::of(ground);
    let system = parts.compose(&trees, tools)?;

    system_prompt_file(&trees, &format!("{}.{program}", self.agent), &system)
  }

  /// The run's log told why its system prompt does not end with the brain's base persona, where it does not.
  pub async fn note_persona(&self, ground: &Ground, log: &mut RunLog) {
    if let Some(why) = &self.persona_unused {
      log.write(&ground.db, Line::note(why.clone())).await;
    }
  }
}

/// One turn of the chat line's agent across rooms, composed for the resident program standing in its place
/// (docs/api.md, 「Sessions」): the chat's run around the caller's room block, road and base persona, on
/// the login the walk offers the agent now, on the session the launch resumes or a new one. A session it
/// resumes that its program keeps no record of is refused as lost, which the turn reads as a session gone.
/// The messages the turn answers and the moment it took its words go among its own variables, and the name
/// of the run its log stands under last; that log is told why the brain's persona was not used where it was
/// not.
pub async fn compose_turn(
  ground: &Ground,
  machine: &Machine,
  launch: &ChatLaunch<'_>,
  prompt: &str,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
) -> Result<ComposedTurn, Refusal> {
  let run = chat_run(ground, machine, launch, prompt, false).await?;

  run.note_persona(ground, log).await;

  compose_prepared(ground, machine, &run, prompt, log.run(), answers, began).await
}

/// The folder a chat run's Claude Code program stands in (docs/sessions.md, 「A restart hands the session
/// over」): the caller's where it named one; for the chat's own launches, which name none, the agent's own home
/// under the data root, made where it is not there yet. Claude Code names a session's records after the
/// folder's real path, which the deploy tree's link changes with every release, so a real folder of the agent's
/// own is what lets the session be resumed after a deploy.
pub fn claude_folder(ground: &Ground, run: &ChatRun) -> Result<String, Refusal> {
  if Path::new(&run.cwd) != ground.roots.deploy {
    return Ok(run.cwd.clone());
  }

  let roots = harness_machine::supervisor_prompt::Roots {
    assets: ground.code_root(),
    data: ground.roots.data.clone(),
  };
  let home = harness_machine::supervisor_prompt::folder(&roots, &run.agent);

  std::fs::create_dir_all(&home).map_err(|why| fault(format!("{}: {why}", home.display())))?;

  Ok(home.to_string_lossy().into_owned())
}

/// A run already composed, its turn on Claude Code's chat line: the login the walk offers its agent, the
/// place its program stands in, the turn's variables and its run's name.
pub async fn compose_prepared(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  prompt: &str,
  run_name: &str,
  answers: &[i64],
  began: i64,
) -> Result<ComposedTurn, Refusal> {
  let trees = Trees::of(ground);
  let login = walked_login(ground, &run.agent).await?;
  let home = match login.as_ref().and_then(|login| login.credential.own_home()) {
    Some(home) => home.to_path_buf(),
    None => ClaudeCode::machine_config_dir(machine),
  };
  let cwd = claude_folder(ground, run)?;

  if let Some(id) = &run.resume
    && !ClaudeCode::records_under(&home, Path::new(&cwd), id)
      .iter()
      .any(|record| record.exists())
  {
    return Err(fault(format!("{LOST_SESSION}: {id}")));
  }

  let opened = uuid::Uuid::new_v4().to_string();
  let place = Place::of(&trees.data, &run.agent);
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
    args: resident_args(run, &settings, &opened)?,
    cwd: cwd.clone(),
    env,
  };
  let env = line_environment(&command, run_name, login.as_ref());

  Ok(ComposedTurn {
    system_path: run.system_path.clone(),
    command: ResidentCommand {
      args: command.args,
      cwd: PathBuf::from(&cwd),
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
    run_env: None,
    given: None,
  })
}

/// The run a gate's turn goes out on: the gate's instructions as the program's whole system prompt, no tools
/// and no MCP server, the model and effort its mapping names, and the thinking the gate asks for.
pub fn gate_run(
  ground: &Ground,
  agent: &str,
  gate: &GateAsk,
  prompt: &str,
  cwd: &str,
) -> Result<ChatRun, Refusal> {
  let trees = Trees::of(ground);
  let system = gate
    .system
    .as_deref()
    .filter(|text| !text.trim().is_empty())
    .ok_or_else(|| fault(format!("{agent}: a gate's turn names its system prompt")))?;
  let digest = format!("{:x}", Sha256::digest(system.as_bytes()));
  let system_path = system_prompt_file(&trees, &format!("{agent}-{}", &digest[..16]), system)?;
  let mcp_config = trees.agents.join("gate.mcp.json");

  if !mcp_config.exists() {
    publish_text(&mcp_config, "{\"mcpServers\":{}}")?;
  }

  let effort = gate.effort.clone().unwrap_or_default();
  let thinking = gate.thinking.unwrap_or(true);
  let mut env: BTreeMap<String, String> = ground.env().clone();

  for (name, value) in &CLAUDE_CODE.launch.variables {
    env.insert(name.clone(), value.clone());
  }

  Ok(ChatRun {
    line: LINE,
    agent: agent.to_owned(),
    model: gate.model.clone().unwrap_or_default(),
    effort: effort.clone(),
    tier: None,
    reasoning: Reasoning {
      thinking: gate.thinking,
      level: Some(effort),
    },
    thinking,
    system_path,
    system_hash: digest,
    system_parts: None,
    room: RoomContext {
      chat_id: 0,
      text: String::new(),
      mark: String::new(),
    },
    tools_shown: String::new(),
    tools_allowed: String::new(),
    tools_denied: None,
    mcp_config,
    mcp_tools: None,
    prompt: prompt.to_owned(),
    persona_unused: None,
    fold: false,
    ping: false,
    resume: None,
    cwd: cwd.to_owned(),
    env,
    cards: Default::default(),
    worker: None,
  })
}

/// What one conversational agent's program on the chat line is started with, whichever agent it is
/// (docs/agents.md, 「Conversational agents」): the model and effort, its own composed prompt, the line's
/// one configuration, what it holds of its own tools, and whether the program's own commands are reachable.
pub struct Words<'a> {
  pub model: &'a str,
  pub effort: &'a str,
  /// The whole system prompt, which stands in the program's own place: a supervisor's opens with a
  /// harness, a gate's is its instructions.
  pub system: &'a Path,
  pub mcp: &'a Path,
  pub hands: Hands<'a>,
  pub host_commands: bool,
}

/// What a program on the chat line holds of its own tools (docs/launch-lines.md, 「What a line's program is
/// allowed, by road」).
#[derive(Clone, Copy, Debug)]
pub enum Hands<'a> {
  /// Every tool of its own but these, each call going ahead unasked: a supervisor's, on every road.
  AllBut(&'a str),
  /// Only what it is shown, and of that only what it is allowed, in don't-ask mode: a gate's judgement, shown
  /// nothing, and a runner line's turn on a gateway road.
  Listed { shown: &'a str, allowed: &'a str },
}

/// The line's words for the interactive program: the model and effort, the composed prompt in the program's
/// own place, nothing of this machine but the one configuration handed, the program's own commands off where
/// they are not reachable, and its permission mode with what it is denied, or shown and allowed.
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

  args.push("--no-chrome".to_owned());

  // Each list flag reads every word after it as another item, so a flag of its own always follows it.
  match line.hands {
    Hands::AllBut(denied) => {
      args.extend(["--permission-mode", "bypassPermissions"].map(str::to_owned));

      if !denied.is_empty() {
        args.extend(["--disallowedTools".to_owned(), denied.to_owned()]);
      }
    }
    Hands::Listed { shown, allowed } => {
      args.extend(
        ["--permission-mode", "dontAsk", "--tools", shown, "--allowedTools", allowed].map(str::to_owned),
      );
    }
  }

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
      hands: match &self.tools_denied {
        Some(denied) => Hands::AllBut(denied),
        None => Hands::Listed {
          shown: &self.tools_shown,
          allowed: &self.tools_allowed,
        },
      },
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
      line: self.line,
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
      tools_denied: self.tools_denied.as_deref(),
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
    line: run.line,
    model: &run.model,
    effort: &run.effort,
    thinking: run.thinking,
    host_commands: run.fold,
    system: file(&run.system_path),
    tools_shown: &run.tools_shown,
    tools_allowed: &run.tools_allowed,
    tools_denied: run.tools_denied.as_deref(),
    mcp: file(&run.mcp_config),
    mcp_tools: run.mcp_tools.as_deref(),
    cwd: &run.cwd,
  };

  format!("{:x}", Sha256::digest(serde_json::to_string(&prefix).unwrap_or_default().as_bytes()))
}

/// The road a stand-in step's turn goes out on (docs/cache-fallback.md, 「While a fallback stands, the room
/// is not silent」), with what it holds of the turn beside the step.
pub enum Door {
  /// Codex, on the agent's kept thread for the step's model and effort where it keeps one.
  Codex { thread: Option<String> },
  /// A provider whose turns go through the pane program's inbox, on the dsh session it keeps.
  Inbox { provider: String },
  /// Claude Code on the gateway, under the model whose home, window and wait the run is sized to.
  Gateway {
    held: Box<GatewayModel>,
    placement: Option<String>,
  },
}

/// The chat's turn composed for one stand-in step: the agent's own program on the step's road, on the chat
/// line's system prompt, tools and MCP configuration, and the turn's variables handed to its run beside a
/// command that keeps them out of the program's identity. A stand-in's program is handed the process's own
/// environment and none of Claude Code's launch variables.
pub async fn compose_stand_in(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  door: &Door,
  answers: &[i64],
  began: i64,
) -> Result<ComposedTurn, Refusal> {
  let mut env: BTreeMap<String, String> = run
    .env
    .iter()
    .filter(|(name, _)| {
      !CLAUDE_CODE.launch.variables.contains_key(*name) && name.as_str() != "MAX_THINKING_TOKENS"
    })
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect();

  env.insert(
    "BMO_TURN_MESSAGES".to_owned(),
    answers.iter().map(i64::to_string).collect::<Vec<_>>().join(","),
  );
  env.insert("BMO_TURN_BEGAN".to_owned(), began.to_string());

  let opened = uuid::Uuid::new_v4().to_string();
  let effort = Some(step.effort.clone()).filter(|effort| !effort.is_empty());
  // The run as the road describes it, which a loop reads its turn by.
  let described = |delivery: &str, keep: bool, resume: Option<&str>, env: &BTreeMap<String, String>| {
    let mut given = serde_json::to_value(run.stand_in(&step.model, &step.effort, delivery, keep))
      .unwrap_or(serde_json::Value::Null);

    given["prompt"] = serde_json::json!(run.prompt);
    given["resumeSessionId"] = serde_json::json!(resume);
    given["env"] = serde_json::json!(env);
    given
  };
  let composed = |command: ResidentCommand, session: String, effort: Option<String>| ComposedTurn {
    system_path: run.system_path.clone(),
    command,
    session,
    prompt: run.prompt.clone(),
    model: step.model.clone(),
    effort,
    placement: None,
    window: None,
    account: None,
    began,
    run_env: None,
    given: None,
  };

  match door {
    Door::Codex { thread } => {
      let shape = Shape::of_line(
        run.line,
        &step.model,
        &step.effort,
        run.thinking,
        &run.cwd,
        Some(run.system_path.clone()),
        Some(run.mcp_config.clone()),
        &run.tools_shown,
        &run.tools_allowed,
      );
      let command = resident_command_of(&shape, thread.as_deref(), &env)?;

      Ok(ComposedTurn {
        given: Some(described("input", true, thread.as_deref(), &env)),
        run_env: Some(env),
        ..composed(command, thread.clone().unwrap_or(opened), effort)
      })
    }
    Door::Inbox { provider } => {
      // The program in the pane is dsh, so the chat's tools paragraph names dsh's own tools.
      let system = run.system_on(ground, "dsh", &super::dsh::SESSION_TOOLS)?;
      // The chat line's session folds itself at its fold ratio; a thrown-away session's place keeps dsh's
      // own.
      let compact_at = if run.line == LINE {
        super::dsh::compact_share(ground, &run.agent, provider).await
      } else {
        None
      };
      let laid = Composed {
        line: run.line,
        provider,
        system: Some(&system),
        rules: None,
        shown: Some(&run.tools_shown),
        allowed: Some(&run.tools_allowed),
        cwd: &run.cwd,
        model: &step.model,
        effort: &step.effort,
        chosen: run.reasoning.level.is_some(),
        thinking: run.thinking,
        mcp: Some(&run.mcp_config),
        compact_at,
      };
      let command = inbox_command(ground, machine, provider, &run.agent, &laid, &env)?;

      Ok(ComposedTurn {
        given: Some(serde_json::json!({ "prompt": run.prompt })),
        run_env: Some(env),
        ..composed(command, opened, None)
      })
    }
    Door::Gateway { held, placement } => {
      // Claude Code on the gateway names its records after its folder's real path as on its own road.
      let cwd = claude_folder(ground, run)?;
      // A session the road no longer holds is no session to resume: the turn opens one of its own, as a
      // fresh session would. This is how a session left on another road or a deleted one is let go.
      let kept = run.resume.as_ref().is_some_and(|id| {
        ClaudeCode::records_under(&held.home, Path::new(&cwd), id)
          .iter()
          .any(|record| record.exists())
      });
      let sent_run = if run.resume.is_some() && !kept {
        ChatRun {
          resume: None,
          ..run.clone()
        }
      } else {
        run.clone()
      };
      let (claude_effort, backend_effort, thinking) = match held.road {
        Some(road) => (
          harness_providers::provider(road).and_then(|provider| provider.claude_effort(&step.effort)),
          Some(step.effort.as_str()),
          Some(step.thinking),
        ),
        None => (None, None, None),
      };
      // A role's run on the worker line stands on the line whole, as on Claude Code's own road: its settings
      // and their hooks, its own guard among them, and its co-workers resolved on the road's provider.
      let worker = sent_run.worker.as_deref();
      let on = OnGateway {
        agent: &sent_run.agent,
        model: &step.model,
        effort: &step.effort,
        cwd: &cwd,
        settings: worker.map(|worker| read_settings(&worker.settings)).unwrap_or_default(),
        compose_typed: RUN_LINES.composes_typed_lines(sent_run.line),
        shell_guard: worker.is_none(),
        sampling: None,
        // The supervisor's and a gate's runs are asked as the role their work key names, where it names one.
        role: policy::work_keys::WORK_KEYS
          .profile(&sent_run.agent)
          .and_then(|profile| profile.prefix_role.as_deref()),
        card_terms: sent_run.cards.terms.clone(),
        thinking,
        claude_effort,
        backend_effort,
        own_coworkers: worker.is_some() && native_provider(&step.provider) == step.provider,
      };
      let (mut command, _) = gateway_command(
        ground,
        machine,
        held,
        on,
        step.answer_tokens,
        Placing::At(placement.clone()),
        |name, effort, thinking, settings| match worker {
          Some(worker) => {
            let sent = WorkerRun {
              model: name.to_owned(),
              effort: effort.to_owned(),
              thinking,
              ..worker.clone()
            };
            let session = match &sent_run.resume {
              Some(resumed) => Session::Resume(resumed.clone()),
              None => Session::Open(opened.clone()),
            };

            super::claude::worker_args(&sent, settings, Some(&session))
          }
          None => {
            let sent = ChatRun {
              model: name.to_owned(),
              effort: effort.to_owned(),
              thinking,
              ..sent_run.clone()
            };

            resident_args(&sent, settings, &opened)
          }
        },
      )
      .await?;

      command.env.extend(env.into_iter().filter(|(name, _)| {
        name.starts_with("BMO_TURN_") || token::RECORD_VARIABLES.contains(&name.as_str())
      }));

      Ok(ComposedTurn {
        placement: placement.clone(),
        window: Some(Window {
          context_tokens: held.context_tokens,
          home: held.home.clone(),
        }),
        ..composed(command, sent_run.resume.clone().unwrap_or_else(|| opened.clone()), effort)
      })
    }
  }
}

/// The command a provider's pane program stands on for a turn: the program and the harness its declaration
/// names, and the overlay the run's own words compose. The turn's environment is left as it was handed in, so
/// no key a program reads reaches the run the turn is written down as.
pub fn inbox_command(
  ground: &Ground,
  machine: &Machine,
  provider: &str,
  agent: &str,
  run: &Composed<'_>,
  env: &BTreeMap<String, String>,
) -> Result<ResidentCommand, Refusal> {
  let declared = harness_providers::provider(provider)
    .ok_or_else(|| fault(format!("{provider} is not a provider this build carries")))?;
  let resident = declared
    .declared()
    .resident
    .as_ref()
    .ok_or_else(|| fault(format!("{provider} stands no pane program")))?;
  let program = resident
    .program
    .clone()
    .ok_or_else(|| fault(format!("{provider} names no pane program")))?;
  let dsh = resident
    .dsh
    .clone()
    .ok_or_else(|| fault(format!("{provider} names no harness program")))?;
  let place = Place::of(&ground.roots.data, agent);

  resident_command(ground, machine, provider, &program, &dsh, &place.path, agent, run, env)
}
