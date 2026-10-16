//! What a failed Claude turn was: a full window, a login gone, a run that died, and how each travels.

use agency_server::job_run::claude::failure::*;

#[test]
fn a_full_window_is_a_limit_with_the_reset_it_names() {
  assert_eq!(
    of_unanswered("Claude AI usage limit reached|1791430000", "job-worker"),
    Failure::Limit {
      resets_at: Some(1_791_430_000),
      account: None,
      silenced: false,
    }
  );
  assert_eq!(
    of_unanswered("You've hit your session limit · resets 3pm", "job-worker"),
    Failure::Limit {
      resets_at: None,
      account: None,
      silenced: false,
    }
  );
}

#[test]
fn a_login_gone_is_read_off_the_last_line_and_named_by_the_key() {
  let failed = of_unanswered("banner\n\nInvalid API key · Please run /login\n", "job-worker");

  assert_eq!(
    failed,
    Failure::Account {
      failure: AccountFailure::Login,
      detail: "job-worker: banner\nInvalid API key · Please run /login".to_owned(),
    }
  );
  assert_eq!(failed.account_failure(), Some(AccountFailure::Login));

  // A long-lived token expired or revoked, in the words the CLI says it.
  for said in [
    "OAuth token has expired",
    "Failed to authenticate: OAuth token revoked.",
    "Invalid bearer token",
  ] {
    assert_eq!(
      of_unanswered(said, "job-worker").account_failure(),
      Some(AccountFailure::Login),
      "{said}"
    );
  }
}

#[test]
fn anything_else_died_with_the_tail_of_what_it_said() {
  let failed =
    of_unanswered("\x1b[31m────\x1b[0m\nNo conversation found with session ID x\n", "job-worker");

  assert_eq!(
    failed,
    Failure::Died {
      ending: RunEnding::Exit { code: 1 },
      said: Some("No conversation found with session ID x".to_owned()),
      message: "claude exited with code 1: No conversation found with session ID x".to_owned(),
    }
  );
  assert_eq!(failed.account_failure(), None);
}

#[test]
fn a_long_text_keeps_its_tail_from_a_whole_line() {
  let long = format!("{}\n{}", "a".repeat(10), "b".repeat(FAILURE_TEXT_LIMIT));
  let kept = failure_text(&format!("{long}\nlast"));

  assert!(kept.starts_with("…\n"));
  assert!(kept.ends_with("last"));
}

#[test]
fn a_failure_travels_by_its_kind() {
  assert_eq!(
    serde_json::to_value(Failure::Limit {
      resets_at: Some(5),
      account: None,
      silenced: false,
    })
    .unwrap(),
    serde_json::json!({ "kind": "limit", "resetsAt": 5 })
  );
  assert_eq!(
    serde_json::to_value(of_unanswered("boom", "k")).unwrap(),
    serde_json::json!({ "kind": "died", "ending": { "kind": "exit", "code": 1 }, "said": "boom", "message": "claude exited with code 1: boom" })
  );
}

#[test]
fn a_server_error_is_an_outage_and_every_failure_names_its_cause_in_english() {
  assert_eq!(
    of_errored("API Error: 529 overloaded", "k").account_failure(),
    Some(AccountFailure::Outage)
  );

  let untrusted = Failure::Untrusted {
    folder: "/work".into(),
    config: "/home/tester/.claude.json".into(),
  };

  assert!(untrusted.cause().starts_with("JobFolderUntrusted: Claude Code did not get past"));
  assert!(
    untrusted.message().contains("/work") && untrusted.message().contains("/home/tester/.claude.json")
  );
  assert!(Failure::Exhausted { attempted: vec![] }.message().is_ascii());
}
