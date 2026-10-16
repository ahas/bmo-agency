//! The logins' watch: a moment written as the TypeScript side writes a number, and the notices that name
//! the account, how long is left and the one command the owner runs.

use std::path::Path;

use agency_server::control::claude_accounts::Account;
use agency_server::keepalive::logins::{NodeToken, moment_text, node_notice, notice};
use policy::fill_marks;
use policy::providers::claude_code::CLAUDE_CODE;

fn account(kind: &str) -> Account {
  Account {
    id: "spare".into(),
    name: "spare login".into(),
    place: 1,
    enabled: true,
    credential_kind: kind.into(),
    credential_path: "/logins/spare".into(),
    needs_login_at: None,
    user_id: None,
  }
}

#[test]
fn a_moment_is_written_as_the_typescript_side_writes_a_number() {
  assert_eq!(moment_text(1_712_345_678_901.0), "1712345678901");
  assert_eq!(moment_text(1_712_345_678_901.125), "1712345678901.125");
}

#[test]
fn the_warning_names_the_account_how_long_is_left_and_the_one_command() {
  let words = &CLAUDE_CODE.login;
  let ending = notice(&account("token"), false, Some(90.0 * 60_000.0));
  let left = fill_marks(&words.ending_in, &[("left", &fill_marks(&words.hours, &[("n", "2")]))]);

  assert!(ending.starts_with(&fill_marks(&words.head, &[("name", "spare login"), ("when", &left)])));
  assert!(ending.contains(&fill_marks(&words.token_command, &[("path", "/logins/spare")])));

  let soon = notice(&account("directory"), false, Some(10_000.0));

  assert!(soon.contains(&fill_marks(&words.minutes, &[("n", "1")])));
  assert!(soon.contains(&fill_marks(&words.directory_command, &[("path", "/logins/spare")])));
  assert!(notice(&account("directory"), true, None).contains(&words.gone_after));
}

#[test]
fn the_token_for_the_nodes_is_told_with_the_command_that_makes_it_on_this_machine() {
  let words = &CLAUDE_CODE.node_login;
  let gone = node_notice(&account("directory"), Path::new("/deploy"), NodeToken::Gone, None);

  assert!(gone.starts_with(&fill_marks(&words.head, &[("name", "spare login"), ("when", &words.gone)])));
  assert!(gone.contains(&fill_marks(&words.command, &[("deploy", "/deploy"), ("name", "spare login")])));
  assert!(gone.ends_with(&words.gone_after));

  let ending =
    node_notice(&account("directory"), Path::new("/deploy"), NodeToken::Ending, Some(3.0 * 3_600_000.0));

  assert!(ending.contains(&fill_marks(&CLAUDE_CODE.login.hours, &[("n", "3")])));
  assert!(ending.ends_with(&CLAUDE_CODE.login.ending_after));
  assert!(
    node_notice(&account("directory"), Path::new("/d"), NodeToken::Missing, None).contains(&words.missing)
  );
}
