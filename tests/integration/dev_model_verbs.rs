//! A worker's dev server and the model catalogue as `bmo-agency` carries them: the dev pane's own decisions,
//! and the catalogue verb run as a shell runs it over a scratch store, its engine's installer a stand-in.

use crate::binary;
use crate::common;

use std::path::Path;

use agency_server::verbs::dev_servers::{
  DevState, DevStep, dev_environment, dev_restart_delay_ms, dev_step, outside_client, rotate_dev_log,
};
use common::Scratch;
use serde_json::{Value, json};

fn wanted(cwd: &str, command: &str) -> Option<String> {
  Some(json!([cwd, command]).to_string())
}

fn state(want: Option<String>, running: Option<String>, start_at: i64) -> DevState {
  DevState {
    wanted: want,
    running,
    start_at,
    ..DevState::default()
  }
}

// "starts the named command once its wait is over, and leaves a matching one alone"
// "stops a server whose command or directory is no longer the row's"
// "with the command taken away, stops what runs and only then ends"
#[test]
fn what_the_dev_server_pane_does_next() {
  let cmd = wanted("/repo", "bun run dev");

  assert_eq!(dev_step(&state(cmd.clone(), None, 0), 1_000), DevStep::Start);
  assert_eq!(dev_step(&state(cmd.clone(), None, 5_000), 1_000), DevStep::Wait);
  assert_eq!(dev_step(&state(cmd.clone(), cmd.clone(), 0), 1_000), DevStep::Wait);
  assert_eq!(
    dev_step(&state(wanted("/repo", "bun run dev --port 2"), cmd.clone(), 0), 1_000),
    DevStep::Stop
  );
  assert_eq!(dev_step(&state(wanted("/other", "bun run dev"), cmd.clone(), 0), 1_000), DevStep::Stop);
  assert_eq!(dev_step(&state(None, cmd.clone(), 0), 1_000), DevStep::Stop);
  assert_eq!(dev_step(&state(None, None, 0), 1_000), DevStep::Exit);
}

// "restarts on request: a matching server is stopped, and a stopped one starts without waiting"
// "ends a server past its lifetime with the pane, unless a restart was just asked for"
// "ends a server unused past its allowance with the pane, and leaves one used within it or one not running"
#[test]
fn a_restart_a_lifetime_and_an_allowance_of_use() {
  let cmd = wanted("/repo", "bun run dev");
  let restart = |want: Option<String>, running: Option<String>, start_at: i64| DevState {
    restart: true,
    ..state(want, running, start_at)
  };

  assert_eq!(dev_step(&restart(cmd.clone(), cmd.clone(), 0), 1_000), DevStep::Stop);
  assert_eq!(dev_step(&restart(cmd.clone(), None, 60_000), 1_000), DevStep::Start);
  assert_eq!(dev_step(&restart(None, None, 0), 1_000), DevStep::Exit);

  let aged = DevState {
    started_at: Some(0),
    max_age_ms: Some(60_000),
    ..state(cmd.clone(), cmd.clone(), 0)
  };

  assert_eq!(dev_step(&aged, 30_000), DevStep::Wait);
  assert_eq!(dev_step(&aged, 61_000), DevStep::Expire);
  assert_eq!(
    dev_step(
      &DevState {
        restart: true,
        ..aged
      },
      61_000
    ),
    DevStep::Stop
  );

  let quiet = DevState {
    used_at: Some(10_000),
    idle_ms: Some(60_000),
    ..state(cmd.clone(), cmd.clone(), 0)
  };

  assert_eq!(dev_step(&quiet, 69_000), DevStep::Wait);
  assert_eq!(dev_step(&quiet, 71_000), DevStep::Idle);
  assert_eq!(
    dev_step(
      &DevState {
        restart: true,
        ..quiet.clone()
      },
      71_000
    ),
    DevStep::Stop
  );
  assert_eq!(
    dev_step(
      &DevState {
        running: None,
        ..quiet
      },
      71_000
    ),
    DevStep::Start
  );
}

// "waits longer after each quick ending, up to a minute"
#[test]
fn waits_longer_after_each_quick_ending_up_to_a_minute() {
  assert_eq!(dev_restart_delay_ms(0), 3_000);
  assert_eq!(dev_restart_delay_ms(1), 3_000);
  assert_eq!(dev_restart_delay_ms(2), 6_000);
  assert_eq!(dev_restart_delay_ms(3), 12_000);
  assert_eq!(dev_restart_delay_ms(20), 60_000);
}

// "holds none of the service's variables, and names the workbench"
// "names a support worker's number, so a server that binds ports moves them (docs/worker-slots.md)"
#[test]
fn the_dev_server_s_environment() {
  let pairs = |items: &[(&str, &str)]| {
    items
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect::<Vec<_>>()
  };
  let env = pairs(&[
    ("PATH", "/bin"),
    ("HOME", "/home/tester"),
    ("BMO_BOT_TOKEN", "secret"),
    ("BMO_CONTROL_TOKEN", "secret"),
    ("BMO_RUN_NAME", "bmo-dev-trainer"),
  ]);

  assert_eq!(
    dev_environment(env, Path::new("/data/workbench/trainer"), None),
    pairs(&[
      ("PATH", "/bin"),
      ("HOME", "/home/tester"),
      ("BMO_WORKBENCH_DIR", "/data/workbench/trainer"),
      ("BMO_DEV_USED_FILE", "/data/workbench/trainer/dev-server.used"),
    ])
  );
  assert_eq!(
    dev_environment(
      pairs(&[("PATH", "/bin"), ("BMO_WORKER_SLOT", "9")]),
      Path::new("/data/workbench/worker-w2"),
      Some(2)
    ),
    pairs(&[
      ("PATH", "/bin"),
      ("BMO_WORKBENCH_DIR", "/data/workbench/worker-w2"),
      ("BMO_DEV_USED_FILE", "/data/workbench/worker-w2/dev-server.used"),
      ("BMO_WORKER_SLOT", "2"),
    ])
  );
}

// "is moved aside past its bound, one generation kept, and left alone below it"
#[test]
fn the_dev_server_s_log_is_moved_aside_past_its_bound() {
  let dir = tempfile::tempdir().unwrap();
  let log = dir.path().join("dev-server.log");

  assert!(!rotate_dev_log(&log, 10));
  std::fs::write(&log, "short").unwrap();
  assert!(!rotate_dev_log(&log, 10));
  std::fs::write(&log, "a line longer than ten bytes").unwrap();
  assert!(rotate_dev_log(&log, 10));
  assert!(!log.exists());
  assert_eq!(
    std::fs::read_to_string(dir.path().join("dev-server.log.1")).unwrap(),
    "a line longer than ten bytes"
  );
}

// "a client outside the group connected to a port the group listens on is use; the group talking to itself is not"
#[test]
fn who_uses_the_dev_server() {
  let group = [
    "p100",
    "f20",
    "n127.0.0.1:3231",
    "f21",
    "n127.0.0.1:13232",
    // The console's server reading the stand-in bot: both ends are the group's.
    "f22",
    "n127.0.0.1:13232->127.0.0.1:40001",
    "p101",
    "f9",
    "n127.0.0.1:40001->127.0.0.1:13232",
    // The server's own call out to a service elsewhere.
    "f10",
    "n127.0.0.1:50000->127.0.0.1:5432",
  ];
  let with = |more: &[&str]| group.iter().chain(more).copied().collect::<Vec<_>>().join("\n");

  assert!(!outside_client(&group.join("\n")));
  assert!(outside_client(&with(&["p100", "f30", "n127.0.0.1:3231->127.0.0.1:41000"])));
  assert!(outside_client(
    &["p100", "f20", "n[::1]:3231", "f21", "n[::1]:3231->[::1]:41000"].join("\n")
  ));
  assert!(!outside_client(""));
}

// The binary as a shell runs it, the store at the address `--db` names, with variables of its own.
fn models(
  scratch: &Scratch,
  data_root: &Path,
  args: &[&str],
  env: &[(&str, String)],
) -> (i32, String, String) {
  let address = scratch.address();
  let done = std::process::Command::new(binary::built("agency-server", "bmo-agency"))
    .arg("model")
    .args(args)
    .args(["--db", &address])
    .env("BMO_DATA_DIR", data_root)
    .envs(env.iter().map(|(name, value)| (*name, value.as_str())))
    .stdin(std::process::Stdio::null())
    .output()
    .unwrap();

  (
    done.status.code().unwrap_or(-1),
    String::from_utf8_lossy(&done.stdout).into_owned(),
    String::from_utf8_lossy(&done.stderr).into_owned(),
  )
}

async fn rvn(scratch: &Scratch, data_root: &Path) {
  let sha = "ab".repeat(32);
  let (code, _, err) = models(
    scratch,
    data_root,
    &[
      "add",
      "rvn",
      "--file",
      "RVN-Q4_K_M-multilingual-mtp.gguf",
      "--bytes",
      "1234",
      "--sha256",
      &sha,
      "--engine",
      "local-llm",
      "--source",
      "https://example.com/rvn.gguf",
    ],
    &[],
  );

  assert_eq!(code, 0, "{err}");
}

// "prints one entry whole, records a node its file lies on, and refuses an entry it does not hold"
#[tokio::test]
async fn the_catalogue_verb_an_installer_asks() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();

  rvn(&scratch, data.path()).await;

  let (code, out, _) = models(&scratch, data.path(), &["--entry", "rvn", "--json"], &[]);
  let read: Value = serde_json::from_str(&out).unwrap();

  assert_eq!(code, 0);
  assert_eq!(
    (read["name"].as_str(), read["file"].as_str()),
    (Some("rvn"), Some("RVN-Q4_K_M-multilingual-mtp.gguf"))
  );
  assert_eq!(read["engine"], "local-llm");
  assert_eq!(models(&scratch, data.path(), &["--placed", "rvn", "node-a"], &[]).0, 0);
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "model_catalogue" WHERE "nodes" = '["node-a"]'"#, vec![])
      .await,
    1
  );

  let (code, _, err) = models(&scratch, data.path(), &["--entry", "gone", "--json"], &[]);

  assert_eq!(code, 2);
  assert!(err.starts_with("models: ") && err.contains("gone"), "{err}");

  let (code, out, _) = models(&scratch, data.path(), &[], &[]);

  assert_eq!(code, 0);
  assert!(out.starts_with("rvn: rvn\n"), "{out}");
  assert!(out.contains("1,234 B · sha256 abababababab"), "{out}");
}

// "runs the engine's installer with the entry and ends done or failed with its last line"
#[tokio::test]
async fn runs_the_engine_s_installer_with_the_entry_and_ends_done_or_failed_with_its_last_line() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let source = data.path().join("source");
  let bin = data.path().join("bin");
  let asked = data.path().join("asked");

  std::fs::create_dir_all(source.join("gate/scripts")).unwrap();
  std::fs::create_dir_all(&bin).unwrap();
  std::fs::write(source.join("gate/scripts/local-llm-install.ts"), "// stand-in\n").unwrap();
  // A stand-in bun that writes the arguments it was given and ends well for rvn alone.
  common::executable(
    &bin.join("bun"),
    &format!(
      "#!/bin/sh\nshift\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = rvn ]; then echo ok; else echo 'head: setup stopped — model-missing'; exit 1; fi\n",
      asked.display()
    ),
  );
  rvn(&scratch, data.path()).await;
  scratch
    .run(
      r#"INSERT INTO "model_catalogue" ("name", "label", "file", "bytes", "sha256", "engine", "nodes", "added_at")
         VALUES ('trained', 'trained', 'trained.gguf', 1, $1, 'local-llm', '[]', 2)"#,
      vec!["cd".repeat(32).into()],
    )
    .await;

  let env = [
    ("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default())),
    ("AGENCY_SOURCE_DIR", source.display().to_string()),
  ];
  let begin = |entry: &str| {
    format!(
      r#"INSERT INTO "model_uses" ("entry", "engine", "chosen_by", "started_at", "state") VALUES ('{entry}', 'local-llm', 'tester', 1, 'running') RETURNING "id""#
    )
  };
  let first = scratch.count(&begin("rvn"), vec![]).await.to_string();
  let (code, out, err) = models(&scratch, data.path(), &["run-use", &first], &env);

  assert_eq!((code, out.as_str()), (0, "local-llm ← rvn: done\n"), "{err}");

  let second = scratch.count(&begin("trained"), vec![]).await.to_string();
  let (code, out, _) = models(&scratch, data.path(), &["run-use", &second], &env);

  assert_eq!(
    (code, out.as_str()),
    (1, "local-llm ← trained: failed · head: setup stopped — model-missing\n")
  );
  assert_eq!(std::fs::read_to_string(&asked).unwrap(), "--model rvn\n--model trained\n");
  assert_eq!(models(&scratch, data.path(), &["run-use", &second], &env).0, 2);
}
