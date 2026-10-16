//! Agency's verbs from a shell (docs/api.md, and each verb's own document): the agents and their settings,
//! an agent's status line and hooks, a job's live Claude runs and native co-worker calls, a batch on a
//! pinned login, the work nodes, the workers' sessions and dev servers, the model catalogue, the documents
//! reloaded, a companion's room followed, and the agents' documents composed. Each prints what it came to
//! and ends with its own code; a refusal is said on the error stream.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use agency_server::verbs::agents::{self, AgentsArgs};
use agency_server::verbs::claude_batch::{self, ClaudeBatch};
use agency_server::verbs::dev_servers::{self, DevAction};
use agency_server::verbs::fold_worker::{self, FoldAsked};
use agency_server::verbs::models::{self, ModelsArgs};
use agency_server::verbs::reload::{self, TreeRedeploy};
use agency_server::verbs::work_node::{self, WorkNodeVerb};
use agency_server::verbs::{
  Answer, VerbError, agent_status, claude_token, code_tree, companion_follow, coworker_pool, deploy_link,
  live_claude, node_pane, turns,
};
use clap::{Parser, Subcommand};
use harness_providers::Machine;

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
  /// The agents and their settings: listed with no flag, or the provider and account orders, the accounts,
  /// the registered models, a key's road and login, a job worker's rows and an agent's own thinking, effort
  /// and gates read and changed.
  #[command(alias = "agents")]
  Agent(AgentsArgs),
  /// An agent's resident program's status line and hooks, with what it has to say on standard input; and
  /// its shell guard, which refuses a shell command that starts Claude Code.
  #[command(disable_help_flag = true)]
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
  /// The job workers' sessions.
  Worker {
    #[command(subcommand)]
    what: WorkerVerb,
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
  /// The documents a session relies on, told that they moved.
  Reload {
    #[arg(long = "path", allow_hyphen_values = true)]
    paths: Vec<String>,
    /// Due at once, rather than after the hour a chat is told once in.
    #[arg(long)]
    now: bool,
  },
  /// A companion from the shell.
  Companion {
    #[command(subcommand)]
    what: CompanionVerb,
  },
  /// The agents' documents composed and written under the agents directory, with the caller's gate agents
  /// named in a JSON file.
  Prompts { gates: Option<PathBuf> },
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
}

#[derive(Subcommand)]
enum CoworkerVerb {
  /// Seats reserved under the CLI this is run from, or every one given back.
  Pool {
    /// `reserve` or `release`.
    action: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    role: Option<String>,
    /// The tier to hold the seats on, in place of the one the role resolves to.
    #[arg(long, allow_hyphen_values = true)]
    tier: Option<String>,
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
enum WorkerVerb {
  /// Folds the job workers' sessions whose prefix moved, or with --all every one.
  Fold {
    #[arg(long)]
    worker: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    dry_run: bool,
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
enum CompanionVerb {
  /// A companion's session in a room printed as it was kept and then each turn as it is appended.
  Follow {
    room: String,
    #[arg(long)]
    agent: String,
    /// The word naming the build the pane was started from.
    #[arg(long)]
    version: Option<String>,
  },
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
    Ok(runtime) => runtime,
    Err(why) => {
      eprintln!("bmo-agency: {why}");
      return ExitCode::FAILURE;
    }
  };

  runtime.block_on(run(cli.verb))
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
    Verb::Worker {
      what: WorkerVerb::Fold {
        worker,
        model,
        all,
        dry_run,
      },
    } => {
      let asked = FoldAsked {
        worker,
        model,
        all,
        dry_run,
      };

      ended(fold(&data_root, &asked).await)
    }
    Verb::DevServer { what } => ended(match what {
      DevVerb::Start { worker } => dev_servers::dev_server(&data_root, DevAction::Start, &worker).await,
      DevVerb::Restart { worker } => dev_servers::dev_server(&data_root, DevAction::Restart, &worker).await,
      DevVerb::Stop { worker } => dev_servers::dev_server(&data_root, DevAction::Stop, &worker).await,
      DevVerb::List { sweep } => dev_servers::dev_servers(&data_root, sweep).await,
      DevVerb::Pane { worker, .. } => dev_servers::dev_pane(&data_root, &worker).await,
    }),
    Verb::DevPane { worker, .. } => ended(dev_servers::dev_pane(&data_root, &worker).await),
    Verb::Model(args) => ended(models::models(&data_root, args).await),
    Verb::Reload { paths, now } => ended(reloaded(&data_root, &paths, now).await),
    Verb::Companion {
      what: CompanionVerb::Follow { room, agent, .. },
    } => ended(companion_follow::follow(&data_root, &agent, &room).await.map(|()| Answer::default())),
    Verb::Prompts { gates } => ended(prompts(&data_root, gates).await),
    Verb::ResidentTurn {
      request,
      data_root: named,
    } => ended(turns::resident_turn(&named.unwrap_or(data_root), &request).await),
    Verb::HostRun {
      request,
      data_root: named,
    } => ended(turns::host_run(&named.unwrap_or(data_root), &request).await),
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
  let (action, role, tier, count) = match what {
    CoworkerVerb::Hook => return Ok(coworker_pool::hook(data_root, &standing_input()).await),
    CoworkerVerb::Pool {
      action,
      role,
      tier,
      count,
    } => (action, role, tier, count),
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
      // A run on a single model has no co-worker apart from itself, so there is nothing to hold.
      if !coworker_pool::caller_lends_coworkers(&Machine::of_process(data_root)) {
        return Err(failed(
          "this run stands on a single model, which has no co-workers apart from the run itself; do the work yourself",
        ));
      }

      let count = count.and_then(|count| count.trim().parse::<i64>().ok()).filter(|count| *count >= 1);
      let (Some(role), Some(count)) = (role, count) else {
        return Err(failed("reserve needs --role <role> and --count <n>"));
      };

      if let Some(tier) = tier.as_deref().filter(|tier| !policy::tiers::TIERS.holds(tier)) {
        return Err(failed(&format!("unknown tier: {tier}")));
      }

      coworker_pool::reserve(&db, &caller, &role, tier.as_deref(), count)
        .await
        .map(Answer::from)
    }
    Some("release") => coworker_pool::release(&db, &caller).await.map(Answer::from),
    _ => Err(failed(coworker_pool::USAGE)),
  }
}

async fn token(data_root: &std::path::Path, account: &str, paste: bool) -> Result<Answer, VerbError> {
  let db = agency_server::verbs::store(data_root).await?;

  claude_token::claude_token(&db, data_root, account, paste).await
}

async fn fold(data_root: &std::path::Path, asked: &FoldAsked) -> Result<Answer, VerbError> {
  let ground = fold_worker::ground_of(data_root).await?;

  fold_worker::fold_worker(&ground, data_root, asked, &mut |said| println!("{said}")).await
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

async fn prompts(data_root: &std::path::Path, gates: Option<PathBuf>) -> Result<Answer, VerbError> {
  let ground = agency_server::verbs::ground(data_root).await?;
  let mut command = std::process::Command::new(&ground.roots.bun);

  command.arg(reload::DEPLOY_AGENTS).current_dir(code_tree());

  if let Some(gates) = gates {
    command.arg(gates);
  }

  let status = command
    .status()
    .map_err(|why| VerbError::Refused(format!("the agents' documents could not be composed: {why}")))?;

  Ok(Answer {
    lines: vec![],
    code: status.code().and_then(|code| u8::try_from(code).ok()).unwrap_or(1),
  })
}
