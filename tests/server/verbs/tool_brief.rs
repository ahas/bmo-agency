//! The tool brief (docs/agent-control.md, 「The tool brief」): the tool a call names, the words its situation is read
//! by, the text a describe printed, and no brief where nothing could be read, the tool left to be briefed at its
//! next call.

use agency_server::verbs::tool_brief::{brief, described_text, situation_of, tool_named};
use serde_json::json;

#[test]
fn the_tool_a_call_names_is_its_namespace_and_name_and_through_the_toolbook_the_tool_it_names() {
  assert_eq!(tool_named(&json!({ "tool_name": "mcp__bmo__send" })).as_deref(), Some("bmo/send"));
  assert_eq!(
    tool_named(&json!({ "tool_name": "mcp__toolbook__call", "tool_input": { "name": "voice/generate" } }))
      .as_deref(),
    Some("voice/generate")
  );
  assert_eq!(tool_named(&json!({ "tool_name": "Bash" })), None);
  assert_eq!(tool_named(&json!({ "tool_name": "mcp__toolbook__list", "tool_input": {} })), None);
}

#[test]
fn a_situation_is_the_tool_and_its_arguments_short_text() {
  let situation = situation_of(
    "bmo/node_exec",
    &json!({ "tool_input": { "node": "node-a", "command": "x".repeat(80) } }),
  );

  assert_eq!(situation, "bmo node exec node node-a command");
}

#[test]
fn a_describe_s_text_is_read_in_each_shape_it_prints() {
  assert_eq!(
    described_text("\"bmo/send: Send a message.\\nManual:\\nUse it so.\""),
    "bmo/send: Send a message.\nManual:\nUse it so."
  );
  assert_eq!(
    described_text(
      &json!({ "content": [{ "type": "text", "text": "a" }, { "type": "text", "text": "b" }] }).to_string()
    ),
    "a\nb"
  );
  assert_eq!(described_text(&json!({ "description": "d", "manual": "m" }).to_string()), "d\n\nm");
  assert_eq!(described_text("plain words"), "plain words");
}

#[tokio::test]
async fn nothing_read_hands_nothing_and_leaves_the_tool_to_its_next_call() {
  let place = tempfile::tempdir().unwrap();
  // No `bmo` stands on this test's path that answers, so neither read gives anything.
  let call = json!({ "tool_name": "mcp__bmo__send", "session_id": "session-a", "tool_input": {} });

  assert_eq!(brief(place.path(), &call).await, None);
  assert!(!place.path().join("tool-briefs").join("session-a.txt").exists());
}

#[test]
fn a_servers_skill_is_read_from_its_deploy_tree_without_its_front_matter() {
  let root = tempfile::tempdir().unwrap();
  let skill = root.path().join("bmo-explorer/deploy/plugin/skills/bmo-explorer");

  std::fs::create_dir_all(&skill).unwrap();
  std::fs::create_dir_all(root.path().join("bmo-explorer/deploy/node_modules/x/skills/y")).unwrap();
  std::fs::write(
    root.path().join("bmo-explorer/deploy/node_modules/x/skills/y/SKILL.md"),
    "---\nname: y\n---\nNot this one.\n",
  )
  .unwrap();
  std::fs::write(
    skill.join("SKILL.md"),
    "---\nname: bmo-explorer\ndescription: x\n---\n\n# Explorer\n\nWalk first.\n",
  )
  .unwrap();

  let read = agency_server::verbs::tool_brief::skill_of(root.path(), "explorer").unwrap();

  assert!(read.starts_with("# Explorer"), "{read}");
  assert!(read.contains("Walk first."));
  assert!(!read.contains("name: bmo-explorer"));
  assert_eq!(agency_server::verbs::tool_brief::skill_of(root.path(), "nobody"), None);
  assert_eq!(agency_server::verbs::tool_brief::skill_of(root.path(), "../etc"), None);
}
