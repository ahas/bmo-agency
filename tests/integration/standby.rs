//! A worker standing with nothing to do (docs/agent-control.md, 「A worker standing with nothing to do」):
//! the program a worker's place is given while no turn has asked for anything, and a pinned worker's pane
//! holding another provider's program replaced at once.

use crate::common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use agency_server::lines::place::{Place, Standby};
use agency_server::places::{AgentRow, Ground, standby};
use agent_host::identity::process_birth;
use common::Scratch;
use common::herdr::HerdrModel;
use harness_providers::{Machine, provider};
use policy::providers::deepseek::DEEPSEEK;
use policy::providers::registered::REGISTERED;
use serde_json::{Value, json};

/// A machine of the test's own: a home whose data root is the machine's own, the agents root in it,
/// this checkout as the tree the documents are read out of, a tool host and a herdr of its own.
struct Bench {
  scratch: Scratch,
  home: tempfile::TempDir,
  ground: Ground,
  _herdr: HerdrModel,
}

impl Bench {
  async fn new() -> Self {
    let scratch = Scratch::new().await;
    let home = tempfile::tempdir().unwrap();
    let herdr = HerdrModel::stand(home.path().join("herdr.sock"));
    let host = home.path().join("mcp-host.sock");

    tool_host(&host);

    let tree = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let at = |path: PathBuf| path.to_string_lossy().into_owned();
    let machine = Machine::with_env(
      [
        ("HOME".to_owned(), at(home.path().to_path_buf())),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("BMO_DEPLOY_DIR".to_owned(), at(tree)),
        ("BMO_AGENTS_DIR".to_owned(), at(home.path().join("agents"))),
        ("BMO_MCP_HOST_SOCKET".to_owned(), at(host)),
        ("BMO_MCP_HOST_ISSUER_TOKEN".to_owned(), "standby-issuer".to_owned()),
        ("CLAUDE_CONFIG_DIR".to_owned(), at(home.path().join(".claude"))),
      ],
      home.path().join(".local/share/bmo/data"),
    );
    let service = common::service(&scratch, &[("AGENCY_SOURCE_DIR", at(home.path().join("source")))]);
    let ground = Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr.socket.clone())));

    // The gateway's key and DeepSeek's, private under the data root's env directory, so a worker pinned to
    // either stands on its pin: a pin that cannot answer would stand the next road of its walk instead.
    let env = machine.data_root().join("env");

    std::fs::create_dir_all(&env).unwrap();

    for name in [&REGISTERED.presence.key_file, &DEEPSEEK.presence.key_file] {
      let file = env.join(name);

      std::fs::write(&file, "a key\n").unwrap();
      std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let members = machine.data_root().join("llm-gateway");

    std::fs::create_dir_all(&members).unwrap();
    std::fs::write(
      members.join("members.json"),
      r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","wait_seconds":300,"context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
    )
    .unwrap();

    for sql in [
      r#"DELETE FROM "agents""#,
      r#"DELETE FROM "coworker_pool_roles""#,
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('job-worker', 'Frontier')"#,
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'Frontier', 'claude-opus-5', 'high')"#,
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('deepseek', 'Frontier', 'deepseek-flash', 'high')"#,
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens", "wait_seconds", "thinking", "walked", "keeps_research", "added_at")
         VALUES ('member', 'Member', 'member-model', 32768, 4096, 60, 1, 0, 0, 1)"#,
      r#"INSERT INTO "registered_model_addresses" ("registration", "node", "position", "url", "key_file", "key_owned")
         VALUES ('member', 'n1', 1, 'http://127.0.0.1:1/v1/chat/completions', NULL, 0)"#,
    ] {
      scratch.run(sql, vec![]).await;
    }

    Self {
      scratch,
      home,
      ground,
      _herdr: herdr,
    }
  }

  // A worker of a checkout of its own, pinned to the provider and model named.
  async fn worker(&self, name: &str, pin: Option<(&str, Option<&str>)>) -> AgentRow {
    let cwd = self.home.path().join(name);

    std::fs::create_dir_all(&cwd).unwrap();
    self
      .scratch
      .run(
        r#"INSERT INTO "agents" ("name", "kind", "cwd", "pinned_provider", "pinned_model") VALUES ($1, 'worker', $2, $3, $4)"#,
        vec![
          name.into(),
          cwd.to_string_lossy().into_owned().into(),
          pin.map(|(provider, _)| provider.to_owned()).into(),
          pin.and_then(|(_, model)| model).map(str::to_owned).into(),
        ],
      )
      .await;

    AgentRow::named(&self.ground.db, name).await.unwrap().unwrap()
  }

  fn place(&self, worker: &str) -> Place {
    Place::of(&self.ground.roots.data, worker)
  }

  // A place holding a program, and that program standing in it as a process the pass can end.
  fn standing(&self, worker: &str, args: &[&str], env: &[(&str, &str)]) -> (String, std::process::Child) {
    let place = self.place(worker);
    let identity = format!("standing-{}", args[0]);
    let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id() as i32;

    std::fs::create_dir_all(&place.path).unwrap();
    std::fs::write(
      place.path.join("program.json"),
      json!({
        "identity": identity,
        "command": { "args": args, "cwd": self.home.path(), "env": env.iter().copied().collect::<std::collections::BTreeMap<_, _>>() },
        "restart": null,
      })
      .to_string(),
    )
    .unwrap();
    std::fs::write(
      place.path.join("running.json"),
      json!({
        "identity": identity, "pid": pid, "birth": process_birth(pid), "loop": { "pid": 1, "birth": "" },
        "starts": 1, "startedAt": agency_server::now_ms() - 1_000, "paneId": null,
      })
      .to_string(),
    )
    .unwrap();

    (identity, child)
  }

  async fn notes(&self, worker: &str) -> Vec<String> {
    store::all(
      &self.ground.db,
      r#"SELECT "kind", "text" FROM "agent_log" WHERE "agent" = $1 ORDER BY "id""#,
      vec![worker.into()],
    )
    .await
    .unwrap()
    .iter()
    .map(|row| {
      format!(
        "{}: {}",
        row.try_get::<String>("", "kind").unwrap(),
        row.try_get::<String>("", "text").unwrap()
      )
    })
    .collect()
  }

  fn member_home(&self) -> PathBuf {
    self.ground.roots.data.join("claude-roads").join("registered").join("member")
  }
}

// A tool host on the socket given, issuing the same token to every record and taking every token back.
fn tool_host(socket: &Path) {
  let listener = tokio::net::UnixListener::bind(socket).unwrap();
  let app = axum::Router::new()
    .route(
      "/tool-sessions",
      axum::routing::post(|| async {
        (axum::http::StatusCode::CREATED, axum::Json(json!({ "token": "standby-token" })))
      }),
    )
    .fallback(|| async { axum::http::StatusCode::NO_CONTENT });

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
}

// Whether a child the pass was to end has ended, waited for a while.
fn ended(child: &mut std::process::Child) -> bool {
  for _ in 0..100 {
    if child.try_wait().unwrap().is_some() {
      return true;
    }

    std::thread::sleep(std::time::Duration::from_millis(20));
  }

  false
}

#[tokio::test]
async fn a_worker_stands_its_jobs_program_with_nothing_put_in_on_a_session_of_its_own() {
  let bench = Bench::new().await;
  let row = bench.worker("idle-worker", None).await;

  standby::stand_idle(&bench.ground, &row).await.unwrap();

  let place = bench.place("idle-worker");
  let program = place.program().unwrap();
  let standby = Standby::read(&place.path).unwrap();

  // The job's own line in the worker's checkout, opening a session of its own, and nothing said.
  assert_eq!(program.command.cwd, PathBuf::from(row.cwd.as_ref().unwrap()));
  assert_eq!(program.command.args[0], "claude");
  assert!(program.command.args.contains(&"--session-id".to_owned()));
  assert!(program.command.args.contains(&standby.session_id));
  assert_eq!(program.opened, Some(Some(standby.record.clone())));
  assert_eq!((standby.model.as_str(), standby.account.as_str()), ("claude-opus-5", "default"));
  assert!(standby.prefix_digest.is_some());

  // A place that holds a program keeps it.
  standby::stand_idle(&bench.ground, &row).await.unwrap();
  assert_eq!(place.program().unwrap().identity, program.identity);
}

#[tokio::test]
async fn a_worker_pinned_to_a_registration_stands_claude_code_on_the_gateway_under_its_home() {
  let bench = Bench::new().await;
  let row = bench.worker("pinned-worker", Some(("registered", Some("member")))).await;

  standby::stand_idle(&bench.ground, &row).await.unwrap();

  let place = bench.place("pinned-worker");
  let program = place.program().unwrap();
  let machine = bench.ground.machine();

  assert_eq!(program.command.args[0], "claude");
  assert!(program.command.args.contains(&"--plugin-dir".to_owned()));
  assert!(program.command.args.contains(&"--system-prompt-file".to_owned()));
  assert!(!program.command.args.contains(&"--append-system-prompt-file".to_owned()));
  assert!(!program.command.args.contains(&"--session-id".to_owned()));
  assert_eq!(
    program.command.env.get("CLAUDE_CONFIG_DIR"),
    Some(&bench.member_home().to_string_lossy().into_owned())
  );
  assert_eq!(program.command.env.get("ANTHROPIC_MODEL").map(String::as_str), Some("member"));
  assert!(provider("registered").unwrap().owns_program(
    &machine,
    &program.command.args,
    &program.command.env
  ));
  assert!(Standby::read(&place.path).is_none());

  // The home's first-run questions are answered, and the worker's checkout trusted in it.
  let config: Value =
    serde_json::from_str(&std::fs::read_to_string(bench.member_home().join(".claude.json")).unwrap())
      .unwrap();

  assert_eq!(config["hasCompletedOnboarding"], true);
  assert_eq!(config["projects"][row.cwd.as_ref().unwrap()]["hasTrustDialogAccepted"], true);
}

#[tokio::test]
async fn a_pinned_workers_pane_holding_another_providers_program_has_its_pins_program_stood_in_its_stead() {
  let bench = Bench::new().await;
  let row = bench.worker("stray-worker", Some(("registered", Some("member")))).await;
  let (_, mut old) = bench.standing("stray-worker", &["claude", "--model", "claude-opus-5"], &[]);

  standby::stand_idle(&bench.ground, &row).await.unwrap();

  let program = bench.place("stray-worker").program().unwrap();

  assert_eq!(
    program.command.env.get("CLAUDE_CONFIG_DIR"),
    Some(&bench.member_home().to_string_lossy().into_owned())
  );
  assert!(ended(&mut old));
  assert_eq!(
    bench.notes("stray-worker").await,
    [
      "note: Took down the Claude Code program standing in the pane and stood the program of the pinned provider Registered models on the model member"
    ]
  );
}

#[tokio::test]
async fn a_member_pins_own_program_is_stood_as_claude_code_and_a_current_one_is_left() {
  let bench = Bench::new().await;
  bench.worker("member-worker", Some(("deepseek", Some("deepseek-flash")))).await;

  let place = bench.place("member-worker").path.to_string_lossy().into_owned();
  let (_, mut old) = bench.standing(
    "member-worker",
    &[
      "bmo-tui",
      "dsh",
      "--program",
      "dsh",
      "--place",
      &place,
      "--model",
      r#"["deepseek","deepseek-flash"]"#,
    ],
    &[],
  );

  standby::restand_all(&bench.ground, None).await;

  let program = bench.place("member-worker").program().unwrap();

  assert_ne!(program.identity, "standing-bmo-tui");
  assert_eq!(program.command.args.first().map(String::as_str), Some("claude"));
  assert_eq!(program.command.env.get("ANTHROPIC_MODEL").map(String::as_str), Some("deepseek-flash"));
  assert_eq!(
    program.command.env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
    Some(
      bench
        .ground
        .roots
        .data
        .join("claude-roads/deepseek/deepseek-flash")
        .to_string_lossy()
        .as_ref()
    )
  );
  assert!(ended(&mut old));

  // Composed again on the same words, the program is the same one, and nothing is stood twice.
  standby::restand_all(&bench.ground, None).await;

  assert_eq!(bench.place("member-worker").program().unwrap().identity, program.identity);
  assert_eq!(bench.notes("member-worker").await.len(), 1);
}

#[tokio::test]
async fn a_worker_pinned_to_claude_code_has_a_members_program_replaced_by_claude_codes() {
  let bench = Bench::new().await;
  bench.worker("back-worker", Some(("claude-code", None))).await;

  let home = bench.member_home().to_string_lossy().into_owned();
  let (_, mut old) = bench.standing(
    "back-worker",
    &["claude", "--model", "member"],
    &[("CLAUDE_CONFIG_DIR", &home), ("ANTHROPIC_BASE_URL", "http://127.0.0.1:18030")],
  );

  standby::restand_all(&bench.ground, None).await;

  let place = bench.place("back-worker");

  assert_eq!(place.program().unwrap().command.args[0], "claude");
  assert!(!place.program().unwrap().command.env.contains_key("ANTHROPIC_BASE_URL"));
  assert!(Standby::read(&place.path).is_some());
  assert!(ended(&mut old));
  assert_eq!(
    bench.notes("back-worker").await,
    [
      "note: Took down the Registered models program standing in the pane and stood the program of the pinned provider Claude Code"
    ]
  );
}

#[tokio::test]
async fn the_pins_own_program_on_another_model_and_a_pane_program_at_work_on_a_persons_line_are_left_alone()
{
  let bench = Bench::new().await;
  let own = bench.worker("own-worker", Some(("registered", Some("member")))).await;
  let typing = bench.worker("typing-worker", Some(("claude-code", None))).await;
  let home = bench.member_home().to_string_lossy().into_owned();
  let (kept, mut kept_child) = bench.standing(
    "own-worker",
    &["claude", "--model", "another-model"],
    &[("CLAUDE_CONFIG_DIR", &home), ("ANTHROPIC_BASE_URL", "http://127.0.0.1:18030")],
  );
  let typing_place = bench.place("typing-worker").path.to_string_lossy().into_owned();
  let (busy, mut busy_child) = bench.standing(
    "typing-worker",
    &[
      "bmo-tui",
      "dsh",
      "--program",
      "dsh",
      "--place",
      &typing_place,
      "--model",
      r#"["openrouter","vendor/model"]"#,
    ],
    &[],
  );

  std::fs::write(
    bench.place("typing-worker").path.join("loop.jsonl"),
    format!(
      "{}\n",
      json!({ "kind": "start", "id": "typed-1", "at": agency_server::now_ms(), "typed": true })
    ),
  )
  .unwrap();

  standby::stand_idle(&bench.ground, &own).await.unwrap();
  standby::stand_idle(&bench.ground, &typing).await.unwrap();

  assert_eq!(bench.place("own-worker").program().unwrap().identity, kept);
  assert_eq!(bench.place("typing-worker").program().unwrap().identity, busy);
  assert!(bench.notes("typing-worker").await.is_empty());

  kept_child.kill().unwrap();
  busy_child.kill().unwrap();
}
