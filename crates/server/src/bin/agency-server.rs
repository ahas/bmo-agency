//! Agency's server: the store settled with the deployment's seeds and the ground stood on it, the places
//! stood in this machine's herdr, the clocks started, the cache siren's quota source on its loopback port,
//! and the control routes on the control port under the server's token, for as long as the process runs
//! (docs/api.md).

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use agency_server::clock::every;
use agency_server::config::Config;
use agency_server::control::claude_accounts::Judging;
use agency_server::places::{Ground, Places};
use agency_server::verbs::reload::{Redeploy, TreeRedeploy};
use agency_server::verbs::{CONTROL_PORT, CONTROL_TOKEN, DEFAULT_CONTROL_PORT};
use agency_server::watches::machine;
use agency_server::{Service, archive_place, control, now_ms};
use clap::Parser;
use harness_providers::Machine;
use policy::keepalive::KEEPALIVE;
use policy::machine_room::MACHINE_ROOM;

/// The variable naming the quota source's loopback port, and the port it takes where none is named.
const QUOTA_PORT: &str = "AGENCY_QUOTA_PORT";
const DEFAULT_QUOTA_PORT: u16 = 9200;

// How often the queue's executions are reconciled with what the machine runs.
const RECONCILE_PASS: Duration = Duration::from_secs(60);

/// Where the server stands.
#[derive(Parser)]
#[command(
  name = "agency-server",
  about = "Agency's server: sessions, places, the co-worker pool and the control routes."
)]
struct Args {
  /// The data root the server stands on.
  #[arg(long, env = "BMO_DATA_DIR")]
  data_root: Option<PathBuf>,
  /// The control port every caller speaks to.
  #[arg(long, env = CONTROL_PORT, default_value_t = DEFAULT_CONTROL_PORT)]
  control_port: u16,
  /// The token every caller carries; no front stands without it.
  #[arg(long, env = CONTROL_TOKEN, hide_env_values = true)]
  token: Option<String>,
  /// The cache siren's quota source, a loopback listener of its own beside the control port.
  #[arg(long, env = QUOTA_PORT, default_value_t = DEFAULT_QUOTA_PORT)]
  quota_port: u16,
}

#[tokio::main]
async fn main() -> ExitCode {
  let args = Args::parse();

  match serve(args).await {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("agency: {why}");
      ExitCode::FAILURE
    }
  }
}

async fn serve(args: Args) -> Result<(), String> {
  // The moment this process started, in seconds as the ledger keeps them: what is open from before it was
  // left by the process before.
  let started_at = agency_server::now_ms();
  let token = args
    .token
    .filter(|token| !token.is_empty())
    .ok_or(format!("{CONTROL_TOKEN} is not set"))?;
  let data_root = args.data_root.unwrap_or_else(agency_server::verbs::data_root);
  let place = archive_place(&data_root).ok_or_else(|| {
    format!(
      "no store is named for {}: set BMO_ARCHIVE_URL, or name it in env/agency.env or env/archive-url",
      data_root.display()
    )
  })?;
  let db = store::connect(&place, store::pool(16))
    .await
    .map_err(|why| format!("the store: {why}"))?;

  let machine = Machine::of_process(&data_root);

  store::seed::settle(&db, &agency_server::seeds::deployment(&machine))
    .await
    .map_err(|fault| format!("the store: {fault}"))?;

  let config = Config::read(|name| std::env::var(name).ok())?;
  let service = Service::new(db.clone(), config);
  let herdr = herdr::socket_path(machine.var("XDG_CONFIG_HOME"), machine.var("HOME"))
    .map(|socket| herdr::Herdr::here(socket, herdr::silences_path(machine.var("BMO_HERDR_SILENCES"))));
  let watch = herdr.clone().map(herdr::picture::Watch::start);
  let places = Places::new(Ground::new(&service, &machine, herdr), Arc::default());
  let ground = places.ground.clone();

  places.clone().stand(watch.clone());
  // The gate's kinds read before the routes stand, so an agent's switches list its gates from the
  // gate's own declaration and not from a copy agency would keep.
  agency_server::gate_kinds::refresh(&ground).await;
  // A turn the last process left in flight is handed back where its program's record holds its end, and
  // waited on where the program goes on with it, before anything else reads what that process left.
  if let Err(why) = agency_server::turn_pickup::at_start(&ground).await {
    eprintln!("agency: the turns left in flight: {}", why.detail);
  }

  // Research runs the last process left under way are taken up again where their places stand, and the
  // rest closed as cut and told to their rooms.
  match agency_server::research::sweep_cut(&ground, started_at).await {
    Ok(swept) => {
      if swept.taken_up > 0 {
        println!("agency: {} research runs taken up again after the restart", swept.taken_up);
      }

      if swept.cut > 0 {
        println!("agency: {} research runs cut by the restart closed and told", swept.cut);
      }
    }
    Err(why) => eprintln!("agency: the research runs left under way: {}", why.detail),
  }

  // Execution rows the last process left open are closed on what they left, and a job whose process
  // went away with the last one is ended before the routes stand. The same pass runs on its clock.
  if let Err(why) = agency_server::queue::reconcile::pass(&ground.db, &ground.roots.data).await {
    eprintln!("agency: the executions: {}", why.detail);
  }

  if let Err(why) = agency_server::control::work::reap_stale(&ground).await {
    eprintln!("agency: the jobs left running: {}", why.error);
  }

  // A run whose turn ended without closing it is closed as cut; the same pass runs on the executions' clock.
  match agency_server::run_sweep::pass(&ground).await {
    Ok(0) => {}
    Ok(closed) => println!("agency: {closed} runs left open by turns that had ended closed as cut"),
    Err(why) => eprintln!("agency: the runs left open: {}", why.detail),
  }

  clocks(&ground);
  tokio::spawn(agency_server::order_run::clocks::keep_bound(
    ground.db.clone(),
    agency_server::queue::running::Running::new(ground.db.clone(), &ground.machine(), watch.clone()),
  ));

  // The agents' documents are written afresh on every start, so the runners' directories carry the text
  // this release reads.
  let redeploy = TreeRedeploy {
    bun: ground.roots.bun.clone(),
    tree: ground.code_root(),
  };

  tokio::task::spawn_blocking(move || redeploy.redeploy());

  let quota = ground.clone();

  tokio::spawn(async move {
    if let Err(why) = control::quota::stand(args.quota_port, quota).await {
      eprintln!("agency: the quota source: {why}");
    }
  });

  let front = control::Front::new(token, ground)?.with_places(places, watch);

  // The words said to the workers still waiting, offered again on the words' own clock.
  if let Some(words) = front.words() {
    words.stand();
  }

  let listener = control::bind(args.control_port)
    .await
    .map_err(|why| format!("the control port: {why}"))?;

  println!(
    "agency: control routes on http://{}",
    listener.local_addr().map_err(|why| why.to_string())?
  );
  control::serve(listener, front).await.map_err(|why| format!("the control port: {why}"))
}

// The server's own clocks: the run log pruned, the pool's admissions and leftovers, the pool's warming, the
// accounts' usage windows read, the queue's reconciliation, the logins' watch on the keepalive pass, the held
// launch's watch, the node service's pressures listened to, and what finished items left in the workbenches
// swept.
fn clocks(ground: &Arc<Ground>) {
  tokio::spawn(agency_server::run_log::prune::keep(ground.db.clone()));

  let gated = ground.clone();

  tokio::spawn(async move {
    every("gate kinds", Duration::from_secs(60), Duration::from_secs(60), move || {
      let ground = gated.clone();

      async move {
        agency_server::gate_kinds::refresh(&ground).await;

        Ok(())
      }
    })
    .await
  });
  tokio::spawn(agency_server::order_run::clocks::keep_pool(ground.db.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_leftovers(ground.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_warm(ground.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_usage_windows(ground.clone()));

  let watched = ground.clone();

  tokio::spawn(async move {
    let period = Duration::from_secs(policy::pull_requests::PULL_REQUESTS.fallback_seconds);

    every("pull requests' fallback", period, period, move || {
      let ground = watched.clone();

      async move { agency_server::watches::pull_requests::fallback(&ground.db, ground.vars()).await }
    })
    .await
  });

  let idle = ground.clone();

  tokio::spawn(async move {
    let period = Duration::from_secs(policy::queue_idle::QUEUE_IDLE.check_seconds);

    every("idle queue's alarm", period, period, move || {
      let ground = idle.clone();

      async move { agency_server::watches::queue_idle::pass(&ground.db, ground.vars()).await }
    })
    .await
  });

  let reconciled = ground.clone();

  tokio::spawn(async move {
    every("queue reconciliation", RECONCILE_PASS, RECONCILE_PASS, move || {
      let ground = reconciled.clone();

      async move {
        agency_server::queue::reconcile::pass(&ground.db, &ground.roots.data)
          .await
          .map_err(|refusal| refusal.detail)?;
        agency_server::control::work::reap_stale(&ground)
          .await
          .map(|_| ())
          .map_err(|refusal| refusal.error)?;
        agency_server::run_sweep::pass(&ground)
          .await
          .map(|_| ())
          .map_err(|refusal| refusal.detail)
      }
    })
    .await
  });

  let watched = ground.clone();
  let first = Duration::from_secs(KEEPALIVE.start_delay_seconds);
  let period = Duration::from_secs(KEEPALIVE.tick_seconds);

  tokio::spawn(async move {
    every("logins' watch", first, period, move || {
      let ground = watched.clone();

      async move {
        agency_server::keepalive::logins::watch(
          Judging::of(&ground),
          &ground.events,
          &ground.roots,
          now_ms(),
        )
        .await
        .map_err(|refusal| refusal.detail)
      }
    })
    .await
  });

  let held = ground.clone();
  let period = Duration::from_secs(MACHINE_ROOM.clocks.held_watch_seconds);

  tokio::spawn(async move {
    every("held launch's watch", period, period, move || {
      let ground = held.clone();

      async move {
        machine::watch_held(&ground.db, ground.peers.node.as_ref(), &ground.events)
          .await
          .map(|_| ())
          .map_err(|refusal| refusal.detail)
      }
    })
    .await
  });

  tokio::spawn(agency_server::watches::pressure::relay(ground.clone()));

  let swept = ground.clone();
  let sweep = Arc::new(agency_server::places::item_sweep::Sweep::default());
  let period = Duration::from_secs(MACHINE_ROOM.clocks.item_sweep_seconds);

  tokio::spawn(async move {
    every("item scratch sweep", Duration::ZERO, period, move || {
      let (ground, sweep) = (swept.clone(), sweep.clone());

      async move {
        let cluster = agency_server::places::item_sweep::StoreCluster(&ground.db);

        sweep.sweep(&ground, &cluster).await.map(|_| ()).map_err(|refusal| refusal.detail)
      }
    })
    .await
  });
}
