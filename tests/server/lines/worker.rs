//! The job worker's line: an account naming a base prompt hands the worker a whole system prompt of its
//! own with the worker's rules after it, and an account naming none hands the platform's harness opening
//! with the same rules after it (docs/base-prompts.md, 「What each agent is sent」).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::claude_accounts::{Account, Credential, EndpointModels};
use agency_server::lines::Trees;
use agency_server::lines::claude::{
  COMPACT_SHARE, Command, Runs, command_env, line_environment, worker_args,
};
use agency_server::lines::worker::worker_run;
use agency_server::places::Ground;
use harness_providers::Machine;
use sea_orm::Value;

use crate::scratch::Scratch;

// No line sets the share of its model's window a session compacts at: a chat, an editor, a gate and a
// job worker alike compact where Claude Code does by itself (docs/sessions.md, 「Session compaction」).
#[test]
fn no_line_sets_the_compact_share() {
  let process = BTreeMap::from([("PATH".to_owned(), "/bin".to_owned())]);
  let command = Command {
    args: vec![],
    cwd: "/w".into(),
    env: command_env(&process, None, &Runs::Here, true).unwrap(),
  };

  assert!(!line_environment(&command, "bmo-any-line", None).contains_key(COMPACT_SHARE));
}

// A ground with a store of its own, its home and data root under one directory the test drops.
async fn ground(scratch: &Scratch, home: &tempfile::TempDir) -> Ground {
  let machine = Machine::with_env(
    [("HOME".to_owned(), home.path().to_string_lossy().into_owned())],
    home.path().join("data"),
  );
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());

  Ground::new(&service, &machine, None)
}

// An account of a kind, on no base prompt unless it is given one.
fn account(id: &str, credential: Credential, base_prompt: Option<&str>) -> Account {
  Account {
    id: id.into(),
    name: id.into(),
    place: 0,
    enabled: true,
    credential,
    needs_login_at: None,
    user_id: None,
    base_prompt: base_prompt.map(str::to_owned),
  }
}

// The file the words after the flag hand over, as the program is handed it.
fn handed_after(args: &[String], flag: &str) -> PathBuf {
  let at = args
    .iter()
    .position(|arg| arg == flag)
    .unwrap_or_else(|| panic!("{flag} is not among {args:?}"));

  PathBuf::from(&args[at + 1])
}

#[tokio::test]
async fn an_endpoint_accounts_base_stands_where_the_programs_prompt_would_and_the_rules_follow() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let dsh = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/base/dsh.md")).unwrap();

  scratch
    .run(
      r#"INSERT INTO "base_prompts" ("name", "label", "text") VALUES ('dsh', 'DeepSeek Harness based', $1)"#,
      vec![Value::from(dsh.trim())],
    )
    .await;

  let ground = ground(&scratch, &home).await;
  let trees = Trees::of(&ground);
  let endpoint = account(
    "deep",
    Credential::Endpoint {
      home: home.path().join("deep"),
      url: "https://api.deepseek.com/anthropic".into(),
      key_file: home.path().join("deep.key"),
      models: EndpointModels {
        opus: Some("deepseek-v4-pro".into()),
        sonnet: Some("deepseek-flash".into()),
        haiku: Some("deepseek-flash".into()),
      },
    },
    Some("dsh"),
  );
  let run = worker_run(&ground, &trees, Some("worker"), Some(&endpoint), "claude-opus-5", "/work")
    .await
    .unwrap();

  let args = worker_args(&run, &run.settings, None).unwrap();
  let whole = handed_after(&args, "--system-prompt-file");

  assert!(!args.iter().any(|arg| arg == "--append-system-prompt-file"));
  assert_eq!(Some(&whole), run.system_prompt.as_ref());

  // The dsh base, its model named by the account's map for the run's family, then the worker's rules.
  let filled = dsh.trim().replace("{{model}}", "deepseek-v4-pro").replace("{{cwd}}", "/work");
  let rules = std::fs::read_to_string(&run.rules).unwrap();

  assert!(filled.starts_with("You are an AI agent powered by DeepSeek Harness."));
  assert_eq!(std::fs::read_to_string(&whole).unwrap(), format!("{filled}\n\n{rules}"));
}

// A worker that chooses no base on an account that names none opens on the platform's harness opening, read
// out of the tree the line is composed from, with its rules after it, handed whole in the program's own place;
// its co-workers' prompts open on the same opening and close on the rules every worker shares.
#[tokio::test]
async fn a_directory_account_with_no_choice_hands_the_harness_and_the_rules_whole() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
         VALUES ('tracer', 'Traces a road', 'Trace the road.', '', '', 1, 1, 0) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:tracer', 'Light')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'Light', 'claude-sonnet-5', 'low')
         ON CONFLICT ("provider", "tier") DO UPDATE SET "model" = excluded."model", "effort" = excluded."effort""#,
      vec![],
    )
    .await;

  let ground = ground(&scratch, &home).await;
  let trees = Trees::of(&ground);
  let directory = account(
    "main",
    Credential::Directory {
      home: home.path().join("main"),
    },
    None,
  );
  let run = worker_run(&ground, &trees, Some("worker"), Some(&directory), "claude-opus-5", "/work")
    .await
    .unwrap();

  let args = worker_args(&run, &run.settings, None).unwrap();
  let harness = std::fs::read_to_string(trees.harness_path()).unwrap();
  let rules = std::fs::read_to_string(&run.rules).unwrap();

  assert!(run.system_prompt.is_none());
  assert!(!args.iter().any(|arg| arg == "--append-system-prompt-file"));
  assert_eq!(handed_after(&args, "--system-prompt-file"), run.harnessed);
  assert_eq!(
    std::fs::read_to_string(&run.harnessed).unwrap(),
    format!("{}\n\n{rules}", harness.trim())
  );

  // The co-workers' definitions carry the opening too, as a subagent's whole prompt.
  let coworkers: serde_json::Value = serde_json::from_str(&run.agents_json).unwrap();
  let prompt = coworkers["tracer"]["prompt"].as_str().unwrap();

  assert!(prompt.starts_with(&format!("{}\n\nTrace the road.\n\n", harness.trim())), "{prompt}");
  assert!(prompt.contains("RULEBOOK prompts/rules/self-preservation.md"), "{prompt}");
}

// A worker's prefix documents stand last in its rules file, in the list's order, each headed by its path
// as written, a relative one read from its checkout and an absolute one where it stands; one that cannot
// be read is left out, and the co-workers' rules carry none (docs/worker-bench.md, 「Prefix documents」).
#[tokio::test]
async fn a_worker_s_prefix_documents_stand_last_in_its_rules_and_its_co_workers_carry_none() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let checkout = home.path().join("checkout");
  let umbrella = home.path().join("STRUCTURE.md");

  std::fs::create_dir_all(checkout.join("docs")).unwrap();
  std::fs::write(checkout.join("docs").join("area.md"), "# The area\n\nIts canonical statement.\n")
    .unwrap();
  std::fs::write(&umbrella, "# The structure\n").unwrap();

  let listed = serde_json::json!([umbrella.to_string_lossy(), "docs/area.md", "docs/gone.md"]).to_string();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "cwd", "prefix_documents") VALUES ('helper', 'worker', $1, $2)"#,
      vec![Value::from(checkout.to_string_lossy().into_owned()), Value::from(listed)],
    )
    .await;

  let ground = ground(&scratch, &home).await;
  let trees = Trees::of(&ground);
  let cwd = checkout.to_string_lossy().into_owned();
  let run = worker_run(&ground, &trees, Some("helper"), None, "claude-opus-5", &cwd).await.unwrap();
  let rules = std::fs::read_to_string(&run.rules).unwrap();
  let expected = format!(
    "DOCUMENT {}:\n# The structure\n\nDOCUMENT docs/area.md:\n# The area\n\nIts canonical statement.",
    umbrella.display()
  );

  assert!(rules.trim_end().ends_with(&expected), "{rules}");
  assert!(!rules.contains("docs/gone.md"), "{rules}");
  assert!(!run.agents_json.contains("Its canonical statement."));
}

// A worker's rules file, the session's cached prefix, carries the report trailer every job is held to, its
// marks filled, after the worker's own rules and before its prefix documents; a co-work branch's carries
// none (docs/launch-lines.md, the job worker's rules file).
#[tokio::test]
async fn a_worker_s_rules_carry_the_report_trailer_before_its_documents_and_a_branch_s_carry_none() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let checkout = home.path().join("checkout");

  std::fs::create_dir_all(checkout.join("docs")).unwrap();
  std::fs::write(checkout.join("docs").join("area.md"), "# The area\n").unwrap();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "cwd", "prefix_documents")
         VALUES ('helper', 'worker', $1, '["docs/area.md"]')"#,
      vec![Value::from(checkout.to_string_lossy().into_owned())],
    )
    .await;

  let ground = ground(&scratch, &home).await;
  let trees = Trees::of(&ground);
  let cwd = checkout.to_string_lossy().into_owned();
  let worker = worker_run(&ground, &trees, Some("helper"), None, "claude-opus-5", &cwd).await.unwrap();
  let rules = std::fs::read_to_string(&worker.rules).unwrap();
  let opening = "Rules a job needs only in some situations stand in the worker's own skills";
  let trailer_at = rules.find(opening).unwrap_or_else(|| panic!("no trailer in {rules}"));

  assert!(trailer_at < rules.find("DOCUMENT docs/area.md:").unwrap(), "{rules}");
  assert!(!rules.contains("{{"), "every mark of the trailer is filled: {rules}");
  // A job's result for a person is written in the language its brief names (docs/worker-language.md).
  assert!(
    rules.contains("A result written for a person is written in the language of the person who asked"),
    "{rules}"
  );
  assert!(
    rules.contains("in the language the item's brief names for the person who asked"),
    "{rules}"
  );
  assert!(rules.contains("plain-writing.md"), "{rules}");

  let branch = worker_run(&ground, &trees, None, None, "claude-opus-5", &cwd).await.unwrap();

  assert!(!std::fs::read_to_string(&branch.rules).unwrap().contains(opening));
}
