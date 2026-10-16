//! A program's paths and variables read as the node's.

use node_link::rewrite::*;
use serde_json::Value;

fn roots() -> Vec<(String, String)> {
  vec![
    ("/h/a/.local/share/bmo/data".into(), "/n/w/data".into()),
    ("/h/a/.local/share/bmo/deploy".into(), "/n/w/deploy".into()),
    ("/h/a".into(), "/n".into()),
  ]
}

#[test]
fn every_root_in_a_string_reads_as_the_nodes() {
  assert_eq!(
    rewrite_text("--settings=/h/a/.local/share/bmo/data/p/settings.json", &roots()),
    "--settings=/n/w/data/p/settings.json"
  );
  assert_eq!(
    rewrite_text("/h/a/bin:/usr/bin:/h/a/.local/share/bmo/deploy/bin", &roots()),
    "/n/bin:/usr/bin:/n/w/deploy/bin"
  );
  assert_eq!(rewrite_text("/h/ab/x", &roots()), "/h/ab/x");
  assert_eq!(rewrite_text("echo /h/a", &roots()), "echo /n");
}

#[test]
fn a_program_reads_as_the_nodes_with_its_files_gathered_and_no_token_of_this_machine() {
  let mut program: Value = serde_json::json!({
    "command": {
      "args": ["claude", "--settings", "/h/a/.local/share/bmo/data/agent-places/w/settings.json"],
      "cwd": "/n/repo",
      "env": { "PATH": "/h/a/bin", "BMO_AGENT_PLACE": "/h/a/.local/share/bmo/data/agent-places/w", "BMO_CONTROL_TOKEN": "secret", "CLAUDE_CODE_X": "1" }
    },
    "opened": "/h/a/.claude/projects/x/s.jsonl"
  });
  let keep = vec!["BMO_AGENT_PLACE".to_owned(), "CLAUDE_CODE_*".to_owned()];
  let add = vec![("BMO_HOME_BRIDGE".to_owned(), "/n/w/data/bridge.sock".to_owned())];
  let env = NodeEnv {
    keep: &keep,
    path: Some("/n/.bun/bin:/usr/bin"),
    add: &add,
  };
  let mut named = Vec::new();

  rewrite_json(&mut program, &roots(), &env, &["/h/a/.local/share/bmo/data".to_owned()], &mut named);

  assert_eq!(program["command"]["args"][2], "/n/w/data/agent-places/w/settings.json");
  assert_eq!(
    program["command"]["env"],
    serde_json::json!({ "PATH": "/n/.bun/bin:/usr/bin", "BMO_AGENT_PLACE": "/n/w/data/agent-places/w", "CLAUDE_CODE_X": "1", "BMO_HOME_BRIDGE": "/n/w/data/bridge.sock" })
  );
  assert_eq!(program["opened"], "/n/.claude/projects/x/s.jsonl");
  assert_eq!(
    named,
    vec![
      "/h/a/.local/share/bmo/data/agent-places/w/settings.json".to_owned(),
      "/h/a/.local/share/bmo/data/agent-places/w".to_owned()
    ]
  );
}
