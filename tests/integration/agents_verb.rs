//! The agents verb of `bmo-agency` (docs/agent-control.md): the orders, the accounts, the registered
//! models, a key's road and login, the job workers and an agent's own settings, each changed over a
//! scratch store with its outside effects — a worker's workbench settled, the server told, what is
//! running, standing input — answered by a stand-in that writes down what it was asked.

use crate::common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use agency_server::peers::Asking;
use agency_server::peers::node::{NodeKind, NodeRow, NodeService};
use agency_server::places::Ground;
use agency_server::verbs::agents::{AgentsArgs, AgentsHands, Said, Stand, agents};
use common::Scratch;
use futures_util::future::BoxFuture;
use harness_providers::Machine;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::verb_words::VERB_WORDS;

/// The outside of the verb, as a test sees it.
#[derive(Default)]
struct Hands {
  asked: Mutex<Vec<String>>,
  running: Option<Vec<String>>,
  input: String,
}

impl Hands {
  fn asked(&self) -> Vec<String> {
    std::mem::take(&mut self.asked.lock().unwrap())
  }
}

impl AgentsHands for Hands {
  fn settle<'a>(&'a self, worker: &'a str) -> BoxFuture<'a, ()> {
    self.asked.lock().unwrap().push(format!("settle {worker}"));
    Box::pin(async {})
  }

  fn tell_workers(&self) -> BoxFuture<'_, ()> {
    self.asked.lock().unwrap().push("tell workers".to_owned());
    Box::pin(async {})
  }

  fn running(&self) -> BoxFuture<'_, Option<Vec<String>>> {
    let running = self.running.clone();

    Box::pin(async move { running })
  }

  fn standing_input(&self) -> String {
    self.input.clone()
  }
}

/// A node service listing the nodes the test set, each with the SSH login it names, or none.
#[derive(Default)]
struct Listed(Mutex<Vec<(NodeRow, Option<Vec<String>>)>>);

impl Listed {
  fn node(&self, name: &str, label: &str, logs_in: bool) {
    let row = NodeRow {
      node: name.into(),
      label: Some(label.into()),
      kind: if logs_in { NodeKind::Ssh } else { NodeKind::Jake },
      state: "connected".into(),
      detail: None,
    };

    self.0.lock().unwrap().push((row, logs_in.then(|| vec![format!("someone@{name}")])));
  }
}

impl NodeService for Listed {
  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    let rows = self.0.lock().unwrap().iter().map(|(row, _)| row.clone()).collect();

    Box::pin(async move { Ok(rows) })
  }

  fn ssh_login<'a>(&'a self, node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    let login = self
      .0
      .lock()
      .unwrap()
      .iter()
      .find(|(row, _)| row.node == node)
      .and_then(|(_, login)| login.clone());

    Box::pin(async move { Ok(login) })
  }
}

/// A scratch store, a data root and a home of the test's own, and the verb run over them.
struct Bench {
  scratch: Scratch,
  data: tempfile::TempDir,
  machine: Machine,
  ground: Ground,
  nodes: Arc<Listed>,
  hands: Hands,
}

impl Bench {
  async fn new() -> Self {
    let data = tempfile::tempdir().unwrap();
    let machine = Machine::of_process(data.path());
    let scratch = Scratch::new().await;
    let mut service =
      common::service(&scratch, &[("AGENCY_SOURCE_DIR", data.path().to_string_lossy().into_owned())]);
    let nodes = Arc::new(Listed::default());

    service.peers.node = nodes.clone();

    let ground = Ground::new(&service, &machine, None);

    scratch.run(r#"DELETE FROM "claude_accounts""#, vec![]).await;

    Self {
      scratch,
      data,
      machine,
      ground,
      nodes,
      hands: Hands::default(),
    }
  }

  fn root(&self) -> &Path {
    self.data.path()
  }

  async fn run(&self, args: &[&str]) -> Said {
    let parsed =
      <AgentsArgs as clap::Parser>::try_parse_from(std::iter::once("agents").chain(args.iter().copied()))
        .unwrap_or_else(|why| panic!("{args:?}: {why}"));
    let home = self.root().join("home");
    let stand = Stand {
      ground: &self.ground,
      machine: &self.machine,
      hands: &self.hands,
      home: &home,
    };

    agents(&stand, &parsed).await.unwrap()
  }

  async fn account(&self, id: &str, name: &str, place: i64) {
    let dir = self.root().join(id);

    std::fs::create_dir_all(&dir).unwrap();
    self
      .scratch
      .run(
        r#"INSERT INTO "claude_accounts" ("id", "name", "place", "credential_kind", "credential_path", "added_at")
           VALUES ($1, $2, $3, 'directory', $4, 1)"#,
        vec![id.into(), name.into(), place.into(), dir.display().to_string().into()],
      )
      .await;
  }

  async fn worker(&self, name: &str) {
    self
      .scratch
      .run(
        r#"INSERT INTO "agents" ("name", "kind", "role", "label", "cwd", "builtin") VALUES ($1, 'worker', 'worker', $1, $2, 0)"#,
        vec![name.into(), self.root().display().to_string().into()],
      )
      .await;
  }

  async fn text(&self, sql: &str, values: Vec<sea_orm::Value>) -> Option<String> {
    use sea_orm::ConnectionTrait;

    let statement =
      sea_orm::Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres, sql, values);

    self
      .scratch
      .db()
      .query_one(statement)
      .await
      .unwrap()
      .and_then(|row| row.try_get_by_index::<Option<String>>(0).unwrap())
  }
}

fn printed(said: &Said) -> String {
  said.out.join("\n")
}

fn ended(said: &Said) -> (u8, &str) {
  said.ended.as_ref().map_or((0, ""), |(code, why)| (*code, why.as_str()))
}

// "prints the account order beside the provider order"
// "the shell that reads and changes the order"
#[tokio::test]
async fn the_listing_prints_both_orders_above_the_agents() {
  let bench = Bench::new().await;

  bench.account("account", "first", 1).await;
  bench.account("account-2", "second", 2).await;
  bench.worker("tryout").await;

  let said = bench.run(&[]).await;

  assert_eq!(ended(&said), (0, ""));
  assert!(printed(&said).starts_with("provider order: claude-code (on)"));
  assert!(printed(&said).contains("registered models: none"));
  assert!(printed(&said).contains("account order: account (first, on) → account-2 (second, on) [default]"));
  assert!(
    printed(&said).contains("account in use: account (first); chosen: none, walks the account order")
  );
  assert!(said.out.iter().any(|line| line.starts_with("tryout               worker  ")));
}

// "moves the order, by id or by the name the owner gave"
// "turns one off and on again, and refuses the last one that is on"
// "puts the order back to the default, and says nothing of an account nobody registered"
#[tokio::test]
async fn the_account_order_moves_switches_and_goes_back_to_its_default() {
  let bench = Bench::new().await;

  bench.account("account", "first", 1).await;
  bench.account("account-2", "second", 2).await;

  let moved = bench.run(&["--account-order", "second,account"]).await;

  assert_eq!(ended(&moved), (0, ""));
  assert_eq!(
    moved.out[0],
    r#"[{"id":"account-2","name":"second","enabled":true},{"id":"account","name":"first","enabled":true}]"#
  );
  assert!(
    printed(&bench.run(&[]).await)
      .contains("account order: account-2 (second, on) → account (first, on)\n")
  );

  assert_eq!(ended(&bench.run(&["--account-off", "second"]).await), (0, ""));

  let last = bench.run(&["--account-off", "account"]).await;

  assert_eq!(ended(&last), (1, CONTROL_WORDS.accounts.last_off.as_str()));
  assert_eq!(ended(&bench.run(&["--account-on", "account-2"]).await), (0, ""));

  let reset = bench.run(&["--account-reset"]).await;

  assert_eq!(ended(&reset), (0, ""));
  assert!(
    printed(&bench.run(&[]).await)
      .contains("account order: account (first, on) → account-2 (second, on) [default]")
  );

  let unknown = bench.run(&["--account-off", "nobody"]).await;

  assert_eq!(
    ended(&unknown),
    (1, fill_marks(&VERB_WORDS.agents.no_account, &[("named", "nobody")]).as_str())
  );
}

// "registers an account, renames it, points it elsewhere and removes it"
#[tokio::test]
async fn an_account_is_registered_renamed_pointed_elsewhere_and_removed() {
  let bench = Bench::new().await;
  let token = bench.root().join("token");

  std::fs::write(&token, "not read by anything under test\n").unwrap();
  bench.account("account", "first", 1).await;

  let token_path = token.display().to_string();
  let added = bench
    .run(&["--account-add", "token login", "--kind", "token", "--path", &token_path])
    .await;

  assert_eq!(ended(&added), (0, ""));
  assert_eq!(added.out, [format!("account token-login (token login) token at {token_path}")]);

  assert_eq!(ended(&bench.run(&["--account-set", "token login", "--name", "renamed"]).await), (0, ""));

  let moved = bench.run(&["--account-set", "token-login", "--path", &token_path]).await;

  assert_eq!(moved.out, [format!("account token-login (renamed) token at {token_path}")]);

  let first = bench.root().join("account").display().to_string();

  for args in [
    vec!["--account-add", "first", "--path", first.as_str()],
    vec!["--account-add", "relative", "--path", "second-login"],
    vec!["--account-set", "nobody", "--name", "somebody"],
  ] {
    assert_eq!(ended(&bench.run(&args).await).0, 1, "{args:?}");
  }

  assert_eq!(ended(&bench.run(&["--account-add", "no path"]).await).0, 2);
  assert_eq!(ended(&bench.run(&["--account-set", "token-login"]).await).0, 2);
  assert_eq!(
    ended(
      &bench
        .run(&["--account-add", "bad kind", "--kind", "keychain", "--path", first.as_str()])
        .await
    ),
    (2, VERB_WORDS.agents.credential_kind.as_str())
  );

  let removed = bench.run(&["--account-remove", "renamed"]).await;

  assert_eq!(removed.out, ["account token-login (renamed) removed"]);

  let last = bench.run(&["--account-remove", "account"]).await;

  assert_eq!(ended(&last), (1, CONTROL_WORDS.accounts.last_gone.as_str()));
}

// "pins by the name the owner gave, prints it in the listing, and gives it back to the order"
// "chooses the account runs go out on now, says so in the listing, and lets it go"
// "the shell that pins a key to a login"
#[tokio::test]
async fn a_key_is_pinned_to_a_login_and_one_login_is_chosen_to_run_on() {
  let bench = Bench::new().await;

  bench.account("account", "first", 1).await;
  bench.account("account-2", "second", 2).await;
  bench
    .scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "role") VALUES ('digest', 'editor', 'editor')"#,
      vec![],
    )
    .await;

  let pinned = bench.run(&["--route", "digest", "--account", "second"]).await;

  assert_eq!(pinned.out.last().unwrap(), "route digest: runs on account-2 (second)");
  assert!(said_line(&bench.run(&[]).await, "digest").contains(" on account-2"));
  assert_eq!(
    ended(&bench.run(&["--route", "digest", "--account", "nobody"]).await),
    (1, fill_marks(&VERB_WORDS.agents.no_account, &[("named", "nobody")]).as_str())
  );
  assert_eq!(
    bench.run(&["--route", "digest", "--account", "auto"]).await.out.last().unwrap(),
    "route digest: walks the account order"
  );
  assert_eq!(bench.text(r#"SELECT "key" FROM "fallback_routes""#, vec![]).await, None);

  let chosen = bench.run(&["--account-use", "second"]).await;

  assert_eq!(chosen.out[1], "account in use: account-2 (second); chosen: account-2");
  assert!(printed(&bench.run(&["--account-use", "auto"]).await).contains("chosen: none"));
  assert_eq!(ended(&bench.run(&["--account-off", "second"]).await), (0, ""));
  assert_eq!(
    ended(&bench.run(&["--account-use", "second"]).await),
    (1, CONTROL_WORDS.accounts.chosen_off.as_str())
  );
}

fn said_line<'a>(said: &'a Said, agent: &str) -> &'a str {
  said
    .out
    .iter()
    .find(|line| line.starts_with(&format!("{agent} ")))
    .map_or("", String::as_str)
}

// "registry refuses retired model writes without mutating routes or tier evidence"
// "a gate is put on the registry from the command line, reading the registrations out of the store"
#[tokio::test]
async fn a_key_s_road_is_set_as_the_console_sets_it_and_a_model_of_its_own_is_refused() {
  let bench = Bench::new().await;

  for mode in ["auto", "claude-code", "codex", "openrouter"] {
    let said = bench.run(&["--route", "job-worker", "--provider", mode, "--model", "obsolete"]).await;

    assert_eq!(ended(&said), (2, "Per-key model writes are retired; use shared model-tier settings."));
  }

  assert_eq!(bench.text(r#"SELECT "key" FROM "fallback_routes""#, vec![]).await, None);

  let unread = bench.run(&["--route", "nowhere", "--provider", "auto"]).await;

  assert_eq!(
    ended(&unread),
    (
      2,
      "usage: bmo-agency agent --route <key> --provider <auto|claude-code|registered|codex|openrouter|laya>"
    )
  );

  // Nothing is registered, so the registry is no road for the key, said as the console says it.
  let refused = bench.run(&["--route", "voice-reading", "--provider", "registered"]).await;

  assert_eq!(ended(&refused).0, 1);
  assert_eq!(bench.text(r#"SELECT "key" FROM "fallback_routes""#, vec![]).await, None);

  let walked = bench.run(&["--route", "voice-reading", "--provider", "auto"]).await;

  assert_eq!(ended(&walked), (0, ""));
  assert!(walked.out.last().unwrap().starts_with("route voice-reading changed to auto ("));
}

// "a registration is made, given an address and a key from standing input, listed without its key, and removed"
#[tokio::test]
async fn a_registration_is_made_given_an_address_and_a_key_listed_without_it_and_removed() {
  let mut bench = Bench::new().await;
  let key = "sk-command-secret-9876543210";

  bench.hands.input = format!("{key}\n");

  let made = bench
    .run(&[
      "--register",
      "local",
      "--model",
      "local-model",
      "--context",
      "65,536",
      "--answer",
      "2048",
      "--wait",
      "120",
      "--thinking",
      "on",
    ])
    .await;

  assert_eq!(ended(&made), (0, ""));
  assert!(printed(&made).contains(
    "local (local): local-model, context 65,536, answer 2,048, wait 120s, thinking on, not walked, research elsewhere"
  ));
  assert_eq!(made.out.last().unwrap(), "gateway: no status written");

  let keyed = bench
    .run(&[
      "--register-address",
      "local",
      "a",
      "http://127.0.0.1:9/v1/chat/completions",
      "--key-stdin",
    ])
    .await;

  assert_eq!(ended(&keyed), (0, ""));
  assert!(
    printed(&keyed)
      .contains(&format!("  a http://127.0.0.1:9/v1/chat/completions {}", VERB_WORDS.agents.key_usable))
  );
  assert_eq!(
    std::fs::read_to_string(bench.root().join("env/registered/local-a.key")).unwrap(),
    format!("{key}\n")
  );

  bench
    .scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model") VALUES ('registered', 'Light', 'local')"#,
      vec![],
    )
    .await;

  let listed = bench.run(&["--registered"]).await;

  assert!(printed(&listed).contains("  named by: "));

  for said in [&made, &keyed, &listed] {
    assert!(!printed(said).contains(key));
  }

  assert_eq!(ended(&bench.run(&["--register", "Bad", "--model", "m"]).await).0, 1);
  assert_eq!(
    ended(&bench.run(&["--register", "local", "--thinking", "maybe"]).await),
    (1, fill_marks(&VERB_WORDS.agents.on_off, &[("flag", "--thinking")]).as_str())
  );
  assert_eq!(ended(&bench.run(&["--unregister", "local"]).await).0, 1);

  bench.scratch.run(r#"DELETE FROM "model_tier_mappings""#, vec![]).await;

  assert_eq!(ended(&bench.run(&["--unregister", "local"]).await), (0, ""));
  assert!(!bench.root().join("env/registered/local-a.key").exists());
  assert_eq!(bench.text(r#"SELECT "name" FROM "registered_models""#, vec![]).await, None);
}

// "a worker is pinned to a provider or a registration and let go again, and a pin nothing could run is refused"
#[tokio::test]
async fn a_worker_is_pinned_to_a_provider_and_let_go_and_a_pin_nothing_could_run_is_refused() {
  let bench = Bench::new().await;
  let pin = || bench.text(r#"SELECT "pinned_provider" FROM "agents" WHERE "name" = 'tryout'"#, vec![]);

  bench.worker("tryout").await;

  assert_eq!(ended(&bench.run(&["--set", "tryout", "--provider", "codex"]).await), (0, ""));
  assert_eq!(pin().await.as_deref(), Some("codex"));
  assert_eq!(bench.hands.asked(), ["tell workers"]);

  let refused = bench.run(&["--set", "tryout", "--provider", "elsewhere"]).await;

  assert_eq!(ended(&refused).0, 1);
  assert_eq!(pin().await.as_deref(), Some("codex"));
  assert_eq!(ended(&bench.run(&["--set", "tryout", "--provider", "auto"]).await), (0, ""));
  assert_eq!(pin().await, None);
}

// A worker the owner adds is given a directory of its own, rests and wakes, has its keepalive paused, and
// is taken away, each change settling its workbench.
#[tokio::test]
async fn a_worker_is_added_rested_woken_paused_and_removed() {
  let bench = Bench::new().await;
  let added = bench.run(&["--add", "helper", "--label", "Helper"]).await;
  let home = bench.root().join("worker-homes/helper");

  assert_eq!(
    added.out.last().unwrap(),
    &format!(
      "worker helper (Helper) added at {} — not a git checkout: ordinary jobs only, no co-work",
      home.display()
    )
  );
  assert!(home.is_dir());
  assert_eq!(bench.hands.asked(), ["settle helper"]);

  assert_eq!(
    bench.run(&["--set", "helper", "--rest", "on"]).await.out.last().unwrap(),
    "worker helper rests"
  );
  assert!(said_line(&bench.run(&[]).await, "helper").contains("worker  resting since 0m ago Helper"));
  assert_eq!(
    ended(&bench.run(&["--set", "helper", "--rest", "maybe"]).await),
    (1, fill_marks(&VERB_WORDS.agents.on_off, &[("flag", "--rest")]).as_str())
  );
  assert_eq!(
    ended(&bench.run(&["--set", "nobody", "--rest", "on"]).await),
    (1, fill_marks(&VERB_WORDS.agents.not_restable, &[("worker", "nobody")]).as_str())
  );
  assert_eq!(
    bench.run(&["--set", "helper", "--rest", "off"]).await.out.last().unwrap(),
    "worker helper woken"
  );

  assert_eq!(
    bench.run(&["--pause-keepalive", "helper"]).await.out.last().unwrap(),
    "worker helper: keepalive off until its next job"
  );
  assert!(said_line(&bench.run(&[]).await, "helper").ends_with(" keepalive off until next job"));

  bench.hands.asked();

  assert_eq!(bench.run(&["--remove", "helper"]).await.out.last().unwrap(), "worker helper removed");
  assert_eq!(bench.hands.asked(), ["settle helper"]);
  assert_eq!(
    ended(&bench.run(&["--remove", "helper"]).await),
    (1, fill_marks(&VERB_WORDS.agents.no_worker, &[("worker", "helper")]).as_str())
  );
}

// A base's checkout on a node stands as a support worker placed there: refused where the node takes no
// job, written from the node's home, kept and relabelled as the base's support settles, and taken away
// only while no job holds it (docs/work-nodes.md, 「A checkout on a node」).
#[tokio::test]
async fn a_checkout_on_a_node_is_added_kept_by_the_support_settling_and_removed() {
  let mut bench = Bench::new().await;

  bench.worker("app").await;
  bench.nodes.node("n3", "Node three", true);
  bench.nodes.node("win", "Windows", false);
  bench
    .scratch
    .run(
      r#"INSERT INTO "node_workplaces" ("node", "root", "home", "runs", "state", "created_at", "updated_at")
         VALUES ('n3', '~/bmo', '/home/someone', 0, 'ok', 1, 1)"#,
      vec![],
    )
    .await;

  let refused = bench.run(&["--node-checkout", "app", "--node", "win"]).await;

  assert_eq!(ended(&refused).0, 1);
  assert!(ended(&refused).1.contains("has no SSH route to carry a job"));

  let added = bench.run(&["--node-checkout", "app", "--node", "n3", "--path", "~/app"]).await;

  assert_eq!(added.out.last().unwrap(), "node checkout app-n3 added on n3 at /home/someone/app");
  assert_eq!(bench.hands.asked(), ["tell workers"]);
  assert!(
    said_line(&bench.run(&[]).await, "app-n3")
      .contains(" node n3 cwd /home/someone/app checkout of app on n3")
  );
  assert_eq!(
    ended(&bench.run(&["--node-checkout", "app", "--node", "n3"]).await),
    (
      1,
      fill_marks(&VERB_WORDS.agents.node_checkout.already, &[("base", "app"), ("node", "n3")]).as_str()
    )
  );

  // A label moved on the base is followed by its checkout on the node, which no count reaches.
  let relabelled = bench.run(&["--set", "app", "--label", "App"]).await;

  assert_eq!(relabelled.out.last().unwrap(), "worker app changed");
  assert_eq!(
    bench
      .text(r#"SELECT "label" FROM "agents" WHERE "name" = 'app-n3'"#, vec![])
      .await
      .as_deref(),
    Some("App@Node three")
  );

  bench.hands.running = Some(vec!["app-n3".to_owned()]);

  assert_eq!(
    ended(&bench.run(&["--drop-node-checkout", "app", "--node", "n3"]).await),
    (1, fill_marks(&VERB_WORDS.agents.node_checkout.busy, &[("worker", "app-n3")]).as_str())
  );

  bench.hands.running = Some(vec![]);

  assert_eq!(
    bench.run(&["--drop-node-checkout", "app", "--node", "n3"]).await.out.last().unwrap(),
    "node checkout app-n3 removed; its checkout on n3 stays"
  );
  assert_eq!(bench.text(r#"SELECT "name" FROM "agents" WHERE "name" = 'app-n3'"#, vec![]).await, None);
}

// A worker placed on a node is refused where it has support workers or the node takes no job, needs the
// node's path, and comes back here only with this machine's.
#[tokio::test]
async fn a_worker_is_placed_on_a_node_and_brought_back() {
  let bench = Bench::new().await;

  bench.worker("solo").await;
  bench.nodes.node("n4", "n4", true);

  assert_eq!(
    ended(&bench.run(&["--set", "solo", "--node", "n4"]).await),
    (1, VERB_WORDS.agents.node_placing.path.as_str())
  );
  assert_eq!(
    ended(&bench.run(&["--set", "solo", "--node", "none"]).await),
    (1, VERB_WORDS.agents.back_here_path.as_str())
  );
  assert_eq!(
    ended(&bench.run(&["--set", "solo", "--node", "n9", "--path", "/srv/solo"]).await),
    (1, fill_marks(&VERB_WORDS.agents.node_placing.no_node, &[("node", "n9")]).as_str())
  );
  assert_eq!(
    bench
      .run(&["--set", "solo", "--node", "n4", "--path", "~/solo", "--herdr-label", "Solo"])
      .await
      .out
      .last()
      .unwrap(),
    "worker solo changed"
  );
  assert_eq!(
    bench
      .text(r#"SELECT "node" FROM "agents" WHERE "name" = 'solo'"#, vec![])
      .await
      .as_deref(),
    Some("n4")
  );
  assert_eq!(
    bench
      .text(r#"SELECT "cwd" FROM "agents" WHERE "name" = 'solo'"#, vec![])
      .await
      .as_deref(),
    Some("~/solo")
  );
  assert_eq!(
    bench
      .text(r#"SELECT "herdr_label" FROM "agents" WHERE "name" = 'solo'"#, vec![])
      .await
      .as_deref(),
    Some("Solo")
  );

  let here = bench.root().display().to_string();

  assert_eq!(
    bench
      .run(&["--set", "solo", "--node", "none", "--path", &here, "--herdr-label", ""])
      .await
      .out
      .last()
      .unwrap(),
    "worker solo changed"
  );
  assert_eq!(bench.text(r#"SELECT "node" FROM "agents" WHERE "name" = 'solo'"#, vec![]).await, None);
  assert_eq!(
    bench.text(r#"SELECT "herdr_label" FROM "agents" WHERE "name" = 'solo'"#, vec![]).await,
    None
  );
  assert_eq!(
    ended(&bench.run(&["--set", "solo", "--downgrade-heal", "maybe"]).await),
    (1, VERB_WORDS.agents.heal_on_off.as_str())
  );
  assert_eq!(
    ended(&bench.run(&["--set", "solo"]).await),
    (1, VERB_WORDS.agents.nothing_to_change.as_str())
  );
}

// An agent's own thinking and effort, each flag left out keeping what the row holds, and its gates
// switched one at a time.
#[tokio::test]
async fn an_agent_s_own_thinking_effort_and_gates() {
  let bench = Bench::new().await;

  bench.worker("tryout").await;

  let set = bench.run(&["--reasoning", "tryout", "--thinking", "on", "--effort", "high"]).await;

  assert_eq!(set.out.last().unwrap(), "tryout: thinking on, effort high");
  assert_eq!(bench.hands.asked(), ["tell workers"]);

  let kept = bench.run(&["--reasoning", "tryout", "--effort", "default"]).await;

  assert_eq!(kept.out.last().unwrap(), "tryout: thinking on, effort default");
  assert_eq!(ended(&bench.run(&["--reasoning", "tryout", "--thinking", "maybe"]).await).0, 1);

  bench
    .scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "role") VALUES ('bmo', 'chat', 'chat')"#,
      vec![],
    )
    .await;

  let listed = bench.run(&["--gates", "bmo"]).await;
  let first = listed.out[4].split("  ").nth(1).unwrap().to_owned();

  assert!(listed.out[4].starts_with(&format!("{}  ", VERB_WORDS.agents.pre_gate)));
  assert!(listed.out[4].ends_with("  on"));

  let switched = bench.run(&["--gates", "bmo", "--gate", &first, "off"]).await;

  assert_eq!(switched.out[4], format!("{}  {first}  off", VERB_WORDS.agents.pre_gate));
  assert_eq!(ended(&bench.run(&["--gates", "bmo", "--gate", &first, "maybe"]).await).0, 1);
  assert_eq!(ended(&bench.run(&["--gates", "tryout"]).await).0, 1);
}
