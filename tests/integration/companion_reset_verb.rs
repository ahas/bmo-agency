//! `bmo-agency companion reset` (docs/companions.md, 「Starting it afresh」): the lines the verb prints of
//! what the route answered, in a preview and in a run.

use agency_server::verbs::companion_reset::reset_lines;
use serde_json::json;

fn answer() -> serde_json::Value {
  json!({
    "agent": "companion",
    "label": "The companion",
    "ranges": ["conversation", "person"],
    "alone": [{ "chatId": 42, "title": "The room" }, { "chatId": 43, "title": null }],
    "shared": [],
    "rows": [
      { "table": "runs", "range": "conversation", "rows": 2 },
      { "table": "memories", "range": "person", "rows": 0 },
    ],
    "files": ["/data/companion/record.jsonl"],
    "brain": { "took": { "memories": 3, "notes": 1 }, "couldNot": [] },
    "failures": [],
  })
}

#[test]
fn a_preview_names_the_companion_its_ranges_and_what_it_would_take() {
  let lines = reset_lines(&answer(), true);
  let text = lines.join("\n");

  assert!(text.contains("The companion (companion) · preview · conversation, person"), "{text}");
  assert!(text.contains("answers alone: The room (42), room 43"), "{text}");
  assert!(text.contains("shares with others: none"), "{text}");
  assert!(text.contains("rows: 2"), "{text}");
  assert!(text.contains("  runs · conversation · 2"), "{text}");
  assert!(!text.contains("memories · person · 0"), "{text}");
  assert!(text.contains("files: 1"), "{text}");
  assert!(text.contains("  /data/companion/record.jsonl"), "{text}");
  assert!(text.contains("brain: 4 taken, 0 could not"), "{text}");
}

#[test]
fn a_run_says_so_and_names_what_was_left() {
  let mut taken = answer();

  taken["ranges"] = json!(["person"]);
  taken["failures"] = json!(["brain: one note could not be read"]);

  let text = reset_lines(&taken, false).join("\n");

  assert!(text.contains("The companion (companion) · run · person"), "{text}");
  assert!(!text.contains("answers alone"), "{text}");
  assert!(text.contains("left: brain: one note could not be read"), "{text}");
}
