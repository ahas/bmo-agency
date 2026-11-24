//! A Claude account signed in from the console (docs/cache-fallback.md, 「Signing an account in from the
//! console」): the sign-in's address read out of what the CLI prints, its hyperlink escapes and all; a refusal
//! said in the CLI's last words with the code taken out; and a new account's directory named by its name. A
//! person's sign-in on their node, as far as it is read without a node: the pane it runs in, the address read off
//! a screen that wrapped it, and the CLI's words after the code judging whether the login took. A person's new
//! login named past every account's names, and a name another account holds refused under its field.

use std::path::Path;

use agency_server::control::claude_logins::{
  NodeOutcome, last_words, name_refusal, new_home, next_login_name, node_launch, node_outcome, said_since,
  sign_in_address,
};
use policy::control_words::CONTROL_WORDS;

// What `claude auth login` prints with no browser to open: the address as a terminal hyperlink, its escapes
// around it, then the prompt for the code.
fn printed(address: &str) -> String {
  format!(
    "Opening browser to sign in…\nIf the browser didn't open, visit: \u{1b}]8;;{address}\u{7}{address}\u{1b}]8;;\u{7}\nPaste code here if prompted > "
  )
}

#[test]
fn the_address_is_read_out_of_the_hyperlink_the_cli_prints() {
  let address = "https://example.com/oauth/authorize?code=true&client_id=x&state=y";
  assert_eq!(sign_in_address(&printed(address)).as_deref(), Some(address));
  assert_eq!(sign_in_address("Opening browser to sign in…\n"), None);
  assert_eq!(sign_in_address("visit: http://example.com/plain"), None);
}

#[test]
fn a_refusal_says_the_last_words_with_the_code_taken_out() {
  let said = format!(
    "{}abc123-secret\n\u{1b}[31mOAuth error: invalid code abc123-secret\u{1b}[0m\n\n",
    printed("https://example.com/a")
  );
  let words = last_words(&said, Some("abc123-secret"));
  assert_eq!(words, "OAuth error: invalid code …");
  assert_eq!(last_words("", None), CONTROL_WORDS.logins.silent);
}

#[test]
fn a_new_account_s_directory_is_named_by_its_name_under_the_env_directory() {
  let root = Path::new("/data");
  assert_eq!(new_home(root, "claude-03"), Path::new("/data/env/claude-03"));
  assert_eq!(new_home(root, " Work Login! "), Path::new("/data/env/work-login"));
  assert_eq!(new_home(root, "../.."), Path::new("/data/env/"));
}

#[test]
fn a_person_s_sign_in_pane_runs_the_cli_s_own_words_under_the_node_s_workplace() {
  let launch =
    node_launch("/home/someone/bmo", "/home/someone", "/home/someone/bmo/data/env/theirs", "C.UTF-8");
  let policy = &policy::providers::claude_code::CLAUDE_CODE.console_login.command;

  // The CLI's words are the shell's arguments after its script, never spliced into it.
  assert_eq!(&launch.command[..2], ["/bin/sh", "-c"]);
  assert_eq!(&launch.command[4..], policy.as_slice());
  assert!(launch.command[2].contains(r#""$@""#));
  assert!(
    launch.command[2].contains("unset CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN")
  );
  assert_eq!(launch.env["CLAUDE_CONFIG_DIR"], "/home/someone/bmo/data/env/theirs");
  assert_eq!(launch.env["BROWSER"], "true");
  assert_eq!(launch.env["HOME"], "/home/someone");
  assert!(launch.env["PATH"].starts_with("/home/someone/bmo/bun/bin:"));
  assert_eq!(launch.cwd, "/home/someone");
}

#[test]
fn the_login_is_judged_by_the_cli_s_words_after_the_code() {
  let before = "Paste code here if prompted > ";

  assert_eq!(
    said_since(before, &format!("{before}abc\nLogin successful.\n")),
    "abc\nLogin successful.\n"
  );
  assert_eq!(
    node_outcome(&said_since(before, &format!("{before}abc\nlogin successful."))),
    NodeOutcome::Took
  );
  assert_eq!(node_outcome("abc\nOAuth error: invalid code"), NodeOutcome::Refused);
  assert_eq!(node_outcome("abc\n"), NodeOutcome::Pending);
}

#[test]
fn a_new_login_is_named_past_every_account_s_numbered_name() {
  assert_eq!(next_login_name(&[]), "claude-01");
  // The owner's own `claude-01` and `claude-02` stand beside a person who holds none of their own.
  assert_eq!(next_login_name(&["claude-01", "claude-02"]), "claude-03");
  assert_eq!(
    next_login_name(&["main", "claude-9", "claude-x", "claude-", " claude-11 "]),
    "claude-12"
  );
}

#[test]
fn a_name_another_account_holds_is_refused_under_the_name_field() {
  let taken = name_refusal("claude-01", &["claude-01"]).expect("a name held is refused");
  assert_eq!(
    (taken.status.as_u16(), taken.code(), taken.field.as_deref()),
    (409, "name-taken".to_owned(), Some("name"))
  );

  let long = name_refusal(&"x".repeat(500), &["claude-01"]).expect("a name too long is refused");
  assert_eq!((long.status.as_u16(), long.field.as_deref()), (400, Some("name")));

  assert!(name_refusal("claude-02", &["claude-01"]).is_none());
}
