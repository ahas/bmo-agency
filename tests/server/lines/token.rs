use std::collections::BTreeMap;

use agency_server::lines::token::*;
use serde_json::json;

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())).collect()
}

#[test]
fn a_run_with_no_research_setting_is_opened_no_research_tool() {
  assert_eq!(
    research_record(&env(&[("BMO_RESEARCH_TOOLS", "")])),
    (None, vec!["web.branches", "web.images", "web.rulebooks"])
  );
}

#[test]
fn a_branch_line_ping_is_opened_the_picture_viewer_alone() {
  let (research, withheld) = research_record(&env(&[("BMO_RESEARCH_IMAGES", "1")]));

  assert_eq!(research, Some(json!({ "branches": 0, "chat": 0, "rulebooks": [] })));
  assert_eq!(withheld, vec!["web.branches", "web.rulebooks"]);
}

#[test]
fn a_research_run_carries_what_its_runner_told_it() {
  let (research, withheld) = research_record(&env(&[
    ("BMO_RESEARCH_TOOLS", "1"),
    ("BMO_RESEARCH_RULEBOOKS", "a,,b"),
    ("BMO_RESEARCH_CLOSES_AT", "1700"),
    ("BMO_RESEARCH_SOURCE", "elsewhere"),
    ("BMO_RESEARCH_OWNER", "research:7"),
    ("BMO_RESEARCH_BRANCHES", "3"),
    ("BMO_RESEARCH_CHAT", "-42x"),
    ("BMO_WEB_HOST_INTERVAL_MS", "1.5"),
  ]));

  assert_eq!(
    research,
    Some(
      json!({ "closesAt": 1700, "owner": "research:7", "branches": 3, "chat": -42, "rulebooks": ["a", "b"] })
    )
  );
  assert_eq!(withheld, vec!["web.images"]);
}
