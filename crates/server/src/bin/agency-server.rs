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
use agency_server::{Service, archive_place, control, now_ms};
use clap::Parser;
use harness_providers::Machine;
use policy::keepalive::KEEPALIVE;

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
  let token = args
    .token
    .filter(|token| !token.is_empty())
    .ok_or(format!("{CONTROL_TOKEN} is not set"))?;
  let data_root = args.data_root.unwrap_or_else(agency_server::verbs::data_root);
  let place = archive_place(&data_root).ok_or_else(|| {
    format!(
      "no store is named for {}: set BMO_ARCHIVE_URL or write env/archive-url",
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
  clocks(&ground);

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
// queue's reconciliation and the logins' watch on the keepalive pass.
fn clocks(ground: &Arc<Ground>) {
  tokio::spawn(agency_server::run_log::prune::keep(ground.db.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_pool(ground.db.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_leftovers(ground.clone()));
  tokio::spawn(agency_server::order_run::clocks::keep_warm(ground.clone()));

  let reconciled = ground.clone();

  tokio::spawn(async move {
    every("queue reconciliation", RECONCILE_PASS, RECONCILE_PASS, move || {
      let ground = reconciled.clone();

      async move {
        agency_server::queue::reconcile::pass(&ground.db, &ground.roots.data)
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
}
