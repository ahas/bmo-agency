//! Agency's verbs from a shell (docs/api.md, and each verb's own document): the agents and their settings,
//! an agent's status line and hooks, a job's live Claude runs and native co-worker calls, a batch on a
//! pinned login, the work nodes, the workers' sessions and dev servers, the model catalogue, the documents
//! reloaded, the supervisor's verdict on a run past its clock, a supervisor's room followed, and the agents'
//! documents composed. Each prints what it came to
//! and ends with its own code; a refusal is said on the error stream.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use agency_server::mcp::AgencyServer;
use agency_server::verbs::agents::{self, AgentsArgs};
use agency_server::verbs::claude_batch::{self, ClaudeBatch};
use agency_server::verbs::dev_servers::{self, DevAction};
use agency_server::verbs::models::{self, ModelsArgs};
use agency_server::verbs::overtime::{self, OvertimeAsked};
use agency_server::verbs::process_memory::{self, MemoryArgs};
use agency_server::verbs::reload::{self, TreeRedeploy};
use agency_server::verbs::run_list::{self, ListArgs};
use agency_server::verbs::run_stop::{self, StopArgs};
use agency_server::verbs::supervisor_reset::{self, ResetArgs};
use agency_server::verbs::work_node::{self, WorkNodeVerb};
use agency_server::verbs::{
  Answer, VerbError, agent_status, claude_token, code_tree, coworker_pool, deploy_link, live_claude,
  node_pane, supervisor_follow, turns,
};
use bmo_agency_client::AgencyClient;
use clap::{Parser, Subcommand};
use harness_providers::Machine;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

/// The nouns `bmo` dispatches to this command, one a line as `nouns` prints them, each marked `home` after
/// a tab where its commands act on the operations machine (bmo-cli's `docs/api.md`, 「The nouns a unit
/// declares」; `docs/api.md`, 「The command」).
pub const NOUNS: [&str; 23] = [
  "agent\thome",
  "agents\thome",
  "agent status\thome",
  "agent-status\thome",
  "model",
  "models",
  "coworker pool",
  "coworker hook\thome",
  "reload",
  "node work\thome",
  "work-node\thome",
  "node claude-token\thome",
  "node pane",
  "node-pane",
  "dev-server",
  "dev-pane",
  "supervisor follow",
  "supervisor reset\thome",
  "claude batch",
  "live-claude",
  "run list\thome",
  "run stop\thome",
  "machine memory",
];

#[derive(Parser)]
#[command(
  name = "bmo-agency",
  about = "Agency's verbs: agents, sessions, workers, nodes and the pool."
)]
struct Cli {
  #[command(subcommand)]
  verb: Verb,
}

// Read once off the command line, so a verb's arguments are held by value however large.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Verb {
  /// Every noun this command carries, one a line, each marked `home` after a tab where its commands act on
  /// the operations machine. It reads nothing of the machine, so `bmo` finds it from any directory.
  Nouns,
  /// Moves the old archive's exported rows into the store (docs/store.md, TODO #1923).
  Import {
    /// The export directory the item's export hand wrote.
    #[arg(long, value_name = "dir")]
    export: PathBuf,
    /// The PostgreSQL database written.
    #[arg(long, value_name = "url")]
    database: String,
    /// Reads, checks and counts everything, and writes nothing.
    #[arg(long)]
    dry_run: bool,
  },
  /// The agents and their settings: listed with no flag, or the provider and account orders, the accounts,
  /// the registered models, a key's road and login, a job worker's rows and an agent's own thinking, effort
  /// and gates read and changed.
  #[command(alias = "agents")]
  Agent(AgentsArgs),
  /// The shell guard of a conversational agent's dsh program, asked by its shell's start-up with the command
  /// about to run on standard input: silent and 0 where it may run, its refusal on the error stream and 126
  /// where it may not (docs/agent-control.md, the shell guard in dsh's shell start-up).
  #[command(hide = true)]
  GuardShell,
  /// An agent's resident program's status line and hooks, with what it has to say on standard input; and
  /// its shell guard, which refuses a shell command that starts Claude Code.
  AgentStatus {
    /// status, stop, failure, submit, start or shell.
    kind: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    place: Option<String>,
    #[arg(long)]
    hold_tasks: bool,
    #[arg(long)]
    hold_wording: bool,
    #[arg(long)]
    compose_typed: bool,
  },
  /// The live Claude runs a job opens, each with what it measures and why; with --list, what it opened.
  LiveClaude {
    #[arg(long, allow_hyphen_values = true)]
    purpose: Option<String>,
    #[arg(long)]
    list: bool,
  },
  /// A job's native co-worker calls held to the common pool.
  Coworker {
    #[command(subcommand)]
    what: CoworkerVerb,
  },
  /// A batch put to Claude Code on a pinned login.
  Claude {
    #[command(subcommand)]
    what: ClaudeVerb,
  },
  /// The work nodes: a Claude account's token for them, a node's pane and its workplace.
  Node {
    #[command(subcommand)]
    what: NodeVerb,
  },
  /// What a node's workspace in herdr runs.
  #[command(hide = true)]
  NodePane { name: String },
  /// The workplaces on the work nodes, as a node's own unit runs them.
  #[command(hide = true)]
  WorkNode {
    #[command(subcommand)]
    step: Option<WorkNodeVerb>,
  },
  /// A worker's dev server: started, started again or stopped, listed, or its pane run.
  DevServer {
    #[command(subcommand)]
    what: DevVerb,
  },
  /// A worker's dev-server pane, as its herdr tab runs it.
  #[command(hide = true)]
  DevPane {
    worker: String,
    #[arg(long)]
    version: Option<String>,
  },
  /// The model catalogue: listed, an entry added, changed, removed or chosen for its engine.
  #[command(alias = "models")]
  Model(ModelsArgs),
  /// Every place's program rewritten to hand agency's tokens as the files they stand in.
  HandTokens,
  /// The documents a session relies on, told that they moved.
  Reload {
    #[arg(long = "path", allow_hyphen_values = true)]
    paths: Vec<String>,
    /// Due at once, rather than after the hour a chat is told once in.
    #[arg(long)]
    now: bool,
  },
  /// The supervisor's verdict on a run past its clock: the asks still open, or keep or cut on one.
  Overtime {
    #[arg(long)]
    list: bool,
    #[arg(long)]
    keep: Option<i64>,
    #[arg(long)]
    cut: Option<i64>,
    /// How long a keep holds: `20m`, `1h` or a number of minutes.
    #[arg(long = "for", allow_hyphen_values = true)]
    length: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    why: Option<String>,
  },
  /// A supervisor from the shell.
  Supervisor {
    #[command(subcommand)]
    what: SupervisorVerb,
  },
  /// The runs on this machine: what is running and what is waiting, and a run stopped by its target.
  Run {
    #[command(subcommand)]
    what: RunVerb,
  },
  /// How much memory each program on this machine holds.
  Machine {
    #[command(subcommand)]
    what: MachineVerb,
  },
  /// The agents' documents composed and written under the agents directory, with the caller's gate agents
  /// named in a JSON file.
  Prompts,
  /// One turn put into an agent's standing program through the door its provider declares, from a request
  /// file, and what it came to printed: what a check carries a turn through.
  ResidentTurn {
    request: PathBuf,
    #[arg(long, env = "BMO_DATA_DIR")]
    data_root: Option<PathBuf>,
  },
  /// One run of a provider's program in its agent's host, from a request file, and what it came to
  /// printed: what a check carries a run in a host through.
  HostRun {
    request: PathBuf,
    #[arg(long, env = "BMO_DATA_DIR")]
    data_root: Option<PathBuf>,
  },
  /// The harness plugin's MCP server on standard input and output, calling the agency at one address.
  Mcp {
    /// The base address the agency's routes stand at; the control port where none is named.
    #[arg(long, env = "AGENCY_URL")]
    url: Option<String>,
    /// The agency's bearer token.
    #[arg(long, env = "AGENCY_CONTROL_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// The agent whose turns the server files items for, which its line names.
    #[arg(long)]
    agent: Option<String>,
  },
  /// Prints the OpenAPI document.
  Openapi,
}

#[derive(Subcommand)]
enum CoworkerVerb {
  /// Slots of a callable role reserved under the CLI this is run from, or every one given back.
  Pool {
    /// `reserve` or `release`.
    action: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    role: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    count: Option<String>,
  },
  /// The worker's native co-worker hooks, with the hook input on standard input.
  Hook,
}

#[derive(Subcommand)]
enum ClaudeVerb {
  /// One batch on a pinned login, its fixed part as the system prompt and the batch on standard input.
  Batch(ClaudeBatch),
}

#[derive(Subcommand)]
enum NodeVerb {
  /// One Claude account's token for the work nodes made and kept on this machine.
  ClaudeToken {
    /// The account's name or id.
    account: String,
    #[arg(long)]
    paste: bool,
  },
  /// What a node's workspace in herdr runs, until the node leaves the list.
  Pane { name: String },
  /// The workplaces on the work nodes: listed with no step, or one node's set up, checked, synced, its
  /// heavy runs switched, or let go.
  Work {
    #[command(subcommand)]
    step: Option<WorkNodeVerb>,
  },
}

#[derive(Subcommand)]
enum DevVerb {
  /// The worker's dev server started, waited on until it is up.
  Start { worker: String },
  /// The worker's dev server started again, waited on until the new one is up.
  Restart { worker: String },
  /// The worker's dev server stopped, waited on until it is down.
  Stop { worker: String },
  /// Every worker's dev server and every server a job left listening; with --sweep the ones due to end
  /// are ended.
  List {
    #[arg(long)]
    sweep: bool,
  },
  /// A worker's dev-server pane, run in its herdr workspace until it closes.
  Pane {
    worker: String,
    #[arg(long)]
    version: Option<String>,
  },
}

#[derive(Subcommand)]
enum SupervisorVerb {
  /// A supervisor's session in a room printed as it was kept and then each turn as it is appended.
  Follow {
    /// The room by its number, which may begin with a minus.
    #[arg(allow_negative_numbers = true)]
    room: String,
    #[arg(long)]
    agent: String,
    /// The word naming the build the pane was started from.
    #[arg(long)]
    version: Option<String>,
  },
  /// A supervisor started afresh: what it said, what it remembers, or both.
  Reset(ResetArgs),
}

#[derive(Subcommand)]
enum RunVerb {
  /// What is running and what is waiting on this machine now, with what each row is stopped by.
  List(ListArgs),
  /// A run stopped by its target, or what is running listed with every target first.
  Stop(StopArgs),
}

#[derive(Subcommand)]
enum MachineVerb {
  /// How much memory each program on this machine holds.
  Memory(MemoryArgs),
}

// The words this command is run with, the two-word noun `agent status` read as the `agent-status` it names:
// `bmo` hands a declared noun's words through unchanged, and both spellings are nouns of this command.
fn words() -> Vec<OsString> {
  let mut words: Vec<OsString> = std::env::args_os().collect();

  if words.get(1).is_some_and(|word| word == "agent") && words.get(2).is_some_and(|word| word == "status") {
    words.remove(1);
    words[1] = "agent-status".into();
  }

  words
}

fn main() -> ExitCode {
  let cli = Cli::parse_from(words());

  // The nouns are answered before the machine is read at all: `bmo` asks `nouns` from any directory, with
  // no settings and no standard input.
  if let Verb::Nouns = &cli.verb {
    NOUNS.iter().for_each(|noun| println!("{noun}"));

    return ExitCode::SUCCESS;
  }

  let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
    Ok(runtime) => runtime,
    Err(why) => {
      eprintln!("bmo-agency: {why}");
      return ExitCode::FAILURE;
    }
  };

  // The import reads its own arguments and never the deployment's store address.
  if let Verb::Import {
    export,
    database,
    dry_run,
  } = &cli.verb
  {
    return runtime.block_on(async {
      match store::import::run(database, export, *dry_run).await {
        Ok(report) => {
          for moved in &report.tables {
            println!(
              "{} -> {}: {} exported, {} written",
              moved.source, moved.target, moved.exported, moved.written
            );
          }
          println!(
            "{} at snapshot {} read {}: {} rows sampled, {} failures",
            if report.dry_run { "dry run" } else { "import" },
            report.snapshot,
            report.read_at,
            report.sample.checked,
            report.sample.failures.len()
          );
          for failure in &report.sample.failures {
            eprintln!("  {failure}");
          }
          if report.sample.failures.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(why) => {
          eprintln!("bmo-agency: {why}");
          ExitCode::FAILURE
        }
      }
    });
  }

  runtime.block_on(async {
    let ended = run(cli.verb).await;

    agency_server::verbs::relay_events().await;
    ended
  })
}

// What a verb came to, printed: its lines on standard output and its code, or its refusal on the error
// stream.
fn ended(answered: Result<Answer, VerbError>) -> ExitCode {
  match answered {
    Ok(answer) => {
      if !answer.lines.is_empty() {
        println!("{}", answer.lines.join("\n"));
      }

      ExitCode::from(answer.code)
    }
    Err(VerbError::Ended { code, said }) => {
      eprintln!("{said}");
      ExitCode::from(code)
    }
    Err(VerbError::Partly { lines, refusal }) => {
      if !lines.is_empty() {
        println!("{}", lines.join("\n"));
      }

      eprintln!("{refusal}");
      ExitCode::FAILURE
    }
    Err(why) => {
      eprintln!("{why}");
      ExitCode::FAILURE
    }
  }
}

fn standing_input() -> String {
  std::io::read_to_string(std::io::stdin()).unwrap_or_default()
}

async fn run(verb: Verb) -> ExitCode {
  let data_root = agency_server::verbs::data_root();

  match verb {
    Verb::Nouns => unreachable!("the nouns are printed before the machine is read"),
    Verb::Import { .. } => unreachable!("the import is answered before the machine is read"),
    Verb::Agent(args) => match agents::run(&data_root, args).await {
      Ok(said) => {
        if !said.out.is_empty() {
          println!("{}", said.out.join("\n"));
        }

        match said.ended {
          Some((code, why)) => {
            eprintln!("{why}");
            ExitCode::from(code)
          }
          None => ExitCode::SUCCESS,
        }
      }
      Err(why) => ended(Err(why)),
    },
    Verb::GuardShell => match agent_status::shell_refusal(&standing_input(), &data_root) {
      Some(refused) => {
        eprintln!("{refused}");
        ExitCode::from(126)
      }
      None => ExitCode::SUCCESS,
    },
    Verb::AgentStatus {
      kind,
      place,
      hold_tasks,
      hold_wording,
      compose_typed,
    } => {
      let asked = agent_status::Asked {
        kind: kind.unwrap_or_default(),
        place: place.unwrap_or_default(),
        hold_tasks,
        hold_wording,
        compose_typed,
      };
      // A standing input that cannot be read is an empty one: the hook never stands in the program's way.
      let input = standing_input();
      let env: BTreeMap<String, String> = std::env::vars().collect();
      let machine = Machine::of_process(&data_root);
      let ground = agent_status::Ground {
        env: &env,
        code_tree: &code_tree(),
        machine_config_dir: &agency_server::control::claude_accounts::machine_config_dir(&machine),
        now_ms: agency_server::now_ms(),
      };

      ended(Ok(agent_status::agent_status(&asked, &input, &ground).await.into()))
    }
    Verb::LiveClaude { purpose, list } => ended(live_claude_run(&data_root, purpose, list).await),
    Verb::Coworker { what } => ended(coworker(&data_root, what).await),
    Verb::Claude {
      what: ClaudeVerb::Batch(asked),
    } => ended(claude_batch::run(asked, &data_root).await.map(Answer::from)),
    Verb::Node { what } => match what {
      NodeVerb::ClaudeToken { account, paste } => ended(token(&data_root, &account, paste).await),
      NodeVerb::Pane { name } => ended(node_pane::node_pane(&data_root, &name).await),
      NodeVerb::Work { step } => ended(work_node::work_node(&data_root, step).await),
    },
    Verb::NodePane { name } => ended(node_pane::node_pane(&data_root, &name).await),
    Verb::WorkNode { step } => ended(work_node::work_node(&data_root, step).await),
    Verb::DevServer { what } => ended(match what {
      DevVerb::Start { worker } => dev_servers::dev_server(&data_root, DevAction::Start, &worker).await,
      DevVerb::Restart { worker } => dev_servers::dev_server(&data_root, DevAction::Restart, &worker).await,
      DevVerb::Stop { worker } => dev_servers::dev_server(&data_root, DevAction::Stop, &worker).await,
      DevVerb::List { sweep } => dev_servers::dev_servers(&data_root, sweep).await,
      DevVerb::Pane { worker, .. } => dev_servers::dev_pane(&data_root, &worker).await,
    }),
    Verb::DevPane { worker, .. } => ended(dev_servers::dev_pane(&data_root, &worker).await),
    Verb::Model(args) => ended(models::models(&data_root, args).await),
    Verb::HandTokens => ended(agency_server::verbs::hand_tokens::hand_tokens(&data_root)),
    Verb::Reload { paths, now } => ended(reloaded(&data_root, &paths, now).await),
    Verb::Overtime {
      list,
      keep,
      cut,
      length,
      why,
    } => {
      let asked = OvertimeAsked {
        list,
        keep,
        cut,
        length,
        why,
      };

      ended(verdict(&data_root, &asked).await)
    }
    Verb::Supervisor { what } => ended(match what {
      SupervisorVerb::Follow { room, agent, .. } => {
        supervisor_follow::follow(&data_root, &agent, &room).await.map(|()| Answer::default())
      }
      SupervisorVerb::Reset(args) => supervisor_reset::reset(args).await,
    }),
    Verb::Run { what } => ended(match what {
      RunVerb::List(asked) => run_list::run_list(&data_root, &asked).await,
      RunVerb::Stop(asked) => run_stop::run_stop(&data_root, &asked).await,
    }),
    Verb::Machine { what } => ended(match what {
      MachineVerb::Memory(asked) => Ok(process_memory::machine_memory(&asked)),
    }),
    Verb::Prompts => ended(prompts(&data_root).await),
    Verb::ResidentTurn {
      request,
      data_root: named,
    } => ended(turns::resident_turn(&named.unwrap_or(data_root), &request).await),
    Verb::HostRun {
      request,
      data_root: named,
    } => ended(turns::host_run(&named.unwrap_or(data_root), &request).await),
    Verb::Mcp { url, token, agent } => mcp(url, token, agent).await,
    Verb::Openapi => {
      print!("{}", agency_server::openapi::openapi_json());
      ExitCode::SUCCESS
    }
  }
}

async fn mcp(url: Option<String>, token: Option<String>, agent: Option<String>) -> ExitCode {
  let port = std::env::var("AGENCY_CONTROL_PORT")
    .ok()
    .and_then(|port| port.trim().parse::<u16>().ok())
    .unwrap_or(agency_server::verbs::DEFAULT_CONTROL_PORT);
  let url = url.unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
  let token = token
    .filter(|token| !token.is_empty())
    .or_else(agency_server::verbs::control_token)
    .unwrap_or_default();
  let client = match AgencyClient::new(&url, token) {
    Ok(client) => client,
    Err(why) => {
      eprintln!("bmo-agency mcp: {why}");
      return ExitCode::FAILURE;
    }
  };
  let server = AgencyServer::new(client, agent.as_deref());
  match server.serve(stdio()).await {
    Ok(service) => {
      let _ = service.waiting().await;
      ExitCode::SUCCESS
    }
    Err(why) => {
      eprintln!("bmo-agency mcp: {why}");
      ExitCode::FAILURE
    }
  }
}

async fn live_claude_run(
  data_root: &std::path::Path,
  purpose: Option<String>,
  list: bool,
) -> Result<Answer, VerbError> {
  let run = live_claude::job_run().ok_or_else(live_claude::not_a_job)?;
  let db = agency_server::verbs::store(data_root).await?;

  if list {
    return live_claude::list(&db, &run).await.map(Answer::from);
  }

  live_claude::open(&db, &run, purpose.as_deref().unwrap_or_default(), agency_server::now_ms())
    .await
    .map(Answer::from)
}

async fn coworker(data_root: &std::path::Path, what: CoworkerVerb) -> Result<Answer, VerbError> {
  let (action, role, count) = match what {
    CoworkerVerb::Hook => return Ok(coworker_pool::hook(data_root, &standing_input()).await),
    CoworkerVerb::Pool {
      action,
      role,
      count,
    } => (action, role, count),
  };
  let failed = |why: &str| VerbError::Ended {
    code: 1,
    said: format!("coworker-pool: {why}"),
  };
  let parent = i64::from(std::os::unix::process::parent_id());
  let Some(caller) = coworker_pool::hook_caller(parent) else {
    return Err(failed("the CLI this command was run from could not be identified"));
  };
  let db = agency_server::verbs::store(data_root).await?;

  match action.as_deref() {
    Some("reserve") => {
      // A run on a provider that lends no co-workers has none apart from itself, so there is nothing to
      // hold.
      let machine = Machine::of_process(data_root);

      if !coworker_pool::caller_lends_coworkers(&machine) {
        return Err(failed(
          "this run stands on a single model, which has no co-workers apart from the run itself; do the work yourself",
        ));
      }

      let count = count.and_then(|count| count.trim().parse::<i64>().ok()).filter(|count| *count >= 1);
      let (Some(role), Some(count)) = (role, count) else {
        return Err(failed("reserve needs --role <role> and --count <n>"));
      };

      coworker_pool::reserve(&db, &caller, &role, count).await.map(Answer::from)
    }
    Some("release") => coworker_pool::release(&db, &caller).await.map(Answer::from),
    _ => Err(failed(coworker_pool::USAGE)),
  }
}

async fn token(data_root: &std::path::Path, account: &str, paste: bool) -> Result<Answer, VerbError> {
  let db = agency_server::verbs::store(data_root).await?;

  claude_token::claude_token(&db, data_root, account, paste).await
}

async fn reloaded(data_root: &std::path::Path, paths: &[String], now: bool) -> Result<Answer, VerbError> {
  if paths.is_empty() {
    return Err(VerbError::Refused(reload::RELOAD_USAGE.to_owned()));
  }

  let ground = agency_server::verbs::ground(data_root).await?;
  let tree = code_tree();
  let here = std::env::current_dir().unwrap_or_default();
  let places = reload::Places {
    brain: data_root.join("brain"),
    deploy: deploy_link(),
    checkouts: vec![ground.source.clone()],
    here,
    tree: tree.clone(),
  };
  let redeploy = TreeRedeploy {
    bun: ground.roots.bun.clone(),
    tree,
  };

  reload::reload(&ground.db, &places, paths, now, &redeploy, agency_server::now_ms() / 1000)
    .await
    .map(Answer::from)
}

async fn verdict(data_root: &std::path::Path, asked: &OvertimeAsked) -> Result<Answer, VerbError> {
  let ground = agency_server::verbs::ground(data_root).await?;

  overtime::overtime(&ground, asked).await
}

async fn prompts(data_root: &std::path::Path) -> Result<Answer, VerbError> {
  let ground = agency_server::verbs::ground(data_root).await?;
  let mut command = std::process::Command::new(&ground.roots.bun);

  command.arg(reload::DEPLOY_AGENTS).current_dir(code_tree());

  let status = command
    .status()
    .map_err(|why| VerbError::Refused(format!("the agents' documents could not be composed: {why}")))?;

  Ok(Answer {
    lines: vec![],
    code: status.code().and_then(|code| u8::try_from(code).ok()).unwrap_or(1),
  })
}
