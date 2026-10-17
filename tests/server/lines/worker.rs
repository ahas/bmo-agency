use std::collections::BTreeMap;

use agency_server::lines::claude::{COMPACT_SHARE, Command, Runs, command_env, line_environment};

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

  assert!(!line_environment(&command, "bmo-any-line").contains_key(COMPACT_SHARE));
}
