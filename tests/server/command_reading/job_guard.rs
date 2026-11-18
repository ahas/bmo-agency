//! The job guard as the shell hook asks it: what it refuses of a job's shell command, read with a ground that
//! stands in for the testbed and the store, and what it lets through to the shell guard.

use std::sync::Mutex;

use agency_server::command_reading::job_guard::JobGround;
use agency_server::command_reading::testbed::Classified;
use agency_server::verbs::agent_status::{CLOSED_INPUT, Refuses, shell_hook};
use serde_json::{Value, json};

const HOME: &str = "/home/tester";

// The testbed's answer, the accounts' directories and the job's opening, as a test sets them, and what the
// guard asked of the store and recorded.
#[derive(Default)]
struct Stand {
  heavy: Option<Classified>,
  dirs: Vec<String>,
  opening: Option<i64>,
  asked_dirs: Mutex<usize>,
  recorded: Mutex<Vec<(i64, String)>>,
}

impl JobGround for Stand {
  async fn classify(&self, _line: &str) -> Option<Classified> {
    Some(self.heavy.clone().unwrap_or_default())
  }

  async fn config_dirs(&self) -> Vec<String> {
    *self.asked_dirs.lock().unwrap() += 1;

    self.dirs.clone()
  }

  async fn opening(&self) -> Option<i64> {
    self.opening
  }

  async fn record(&self, opening: i64, what: &str) {
    self.recorded.lock().unwrap().push((opening, what.to_owned()));
  }
}

const JOB: Refuses = Refuses {
  claude: false,
  job: true,
};

fn call(command: &str, timeout: Option<u64>) -> String {
  let mut input = json!({ "command": command });

  if let Some(timeout) = timeout {
    input["timeout"] = json!(timeout);
  }

  json!({ "tool_name": "Bash", "tool_input": input }).to_string()
}

// The hook's answer read: the reason where it refused, or the command it sends on.
enum Answered {
  Refused(String),
  Sent(String),
}

async fn answered(command: &str, refuses: Refuses, stand: &Stand) -> Answered {
  let answer: Value =
    serde_json::from_str(&shell_hook(&call(command, None), refuses, HOME, stand).await.unwrap()).unwrap();
  let out = &answer["hookSpecificOutput"];

  match out["permissionDecision"].as_str() {
    Some("deny") => Answered::Refused(out["permissionDecisionReason"].as_str().unwrap().to_owned()),
    _ => Answered::Sent(out["updatedInput"]["command"].as_str().unwrap().to_owned()),
  }
}

fn refused(answer: Answered) -> String {
  match answer {
    Answered::Refused(reason) => reason,
    Answered::Sent(command) => panic!("let through: {command}"),
  }
}

#[tokio::test]
async fn a_job_s_sleeping_wait_on_a_full_match_is_refused_and_another_line_s_is_only_closed() {
  let stand = Stand::default();
  let waits = "until ! pgrep -f 'cargo test' > /dev/null; do sleep 10; done";

  assert!(refused(answered(waits, JOB, &stand).await).contains("pgrep -f"));
  assert!(refused(answered("ls", JOB, &stand).await).contains("standard input"));

  match answered(waits, Refuses::default(), &stand).await {
    Answered::Sent(command) => assert_eq!(command, format!("{CLOSED_INPUT}\n{waits}")),
    Answered::Refused(reason) => panic!("a line with no job guard refused: {reason}"),
  }

  match answered("ls -la .", JOB, &stand).await {
    Answered::Sent(command) => assert_eq!(command, format!("{CLOSED_INPUT}\nls -la .")),
    Answered::Refused(reason) => panic!("refused: {reason}"),
  }
}

#[tokio::test]
async fn a_heavy_command_is_refused_with_the_testbed_s_line() {
  let stand = Stand {
    heavy: Some(Classified {
      heavy: true,
      language: Some("cargo".into()),
      command: Some(vec!["cargo".into(), "test".into()]),
      instead: Some("bmo-testbed run -- cargo test".into()),
    }),
    ..Stand::default()
  };
  let reason = refused(answered("cargo test", JOB, &stand).await);

  assert!(reason.contains("cargo test (cargo) is a heavy command"), "{reason}");
  assert!(reason.contains("bmo-testbed run -- cargo test"), "{reason}");
}

#[tokio::test]
async fn a_live_claude_session_is_refused_until_the_job_opens_it_and_then_recorded() {
  let closed = Stand::default();
  let reason = refused(answered("claude --model x", JOB, &closed).await);

  assert!(reason.contains("bmo live-claude --purpose"), "{reason}");
  assert_eq!(*closed.asked_dirs.lock().unwrap(), 0, "a command naming no records asks no store");

  let opened = Stand {
    opening: Some(7),
    ..Stand::default()
  };

  assert!(matches!(answered("claude --model x", JOB, &opened).await, Answered::Sent(_)));
  assert_eq!(*opened.recorded.lock().unwrap(), vec![(7, "shell: claude --model x".to_owned())]);
  assert!(
    refused(answered("claude -p hi", JOB, &opened).await).contains("print mode"),
    "print mode stays refused whatever the job opened"
  );
}

#[tokio::test]
async fn a_write_into_an_account_s_records_is_read_against_the_store_s_directories() {
  let stand = Stand {
    dirs: vec!["/srv/accounts/second".into()],
    ..Stand::default()
  };
  let reason = refused(answered("cp a.jsonl /srv/accounts/second/projects/x/", JOB, &stand).await);

  assert!(reason.contains("refused before this job has said what for"), "{reason}");
  assert_eq!(*stand.asked_dirs.lock().unwrap(), 1);
}
