//! The job worker's line: an account naming a base prompt hands the worker a whole system prompt of its
//! own with the worker's rules after it, and an account naming none leaves the program's own prompt with
//! the rules appended (docs/base-prompts.md, 「What each agent is sent」).

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

#[tokio::test]
async fn a_directory_account_with_no_choice_appends_the_rules_to_the_programs_own_prompt() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
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

  assert!(run.system_prompt.is_none());
  assert!(!args.iter().any(|arg| arg == "--system-prompt-file"));
  assert_eq!(handed_after(&args, "--append-system-prompt-file"), run.rules);
}
