//! The one grammar a collection of rows is worked through from the shell (docs/cli.md, 「One grammar for every
//! noun」): a change names only its fields, each value read as JSON where it reads as JSON; a list prints a row a
//! line; a refusal says the route's words and the step that gets past it.

use agency_server::verbs::VerbError;
use agency_server::verbs::crud::{ROLE_AGENTS, Step, answered, fields};
use serde_json::json;

#[test]
fn a_field_reads_as_json_where_it_does_and_as_text_otherwise() {
  let read =
    fields(&["enabled=false".to_owned(), "model=q38fn-dq8".to_owned(), "answerTokens=8000".to_owned()])
      .unwrap();

  assert_eq!(
    serde_json::Value::Object(read),
    json!({ "enabled": false, "model": "q38fn-dq8", "answerTokens": 8000 })
  );
  assert!(fields(&["model".to_owned()]).unwrap_err().contains("field=value"));
  assert!(fields(&["=x".to_owned()]).unwrap_err().contains("names no field"));
}

#[test]
fn a_list_prints_a_row_a_line_with_what_tells_rows_apart() {
  let page = json!({ "items": [
    { "name": "coder-a", "role": "coder", "provider": "registered", "model": "q38fn-dq8", "enabled": true },
    { "name": "coder-b", "role": "coder", "provider": "claude-code", "model": "claude-opus-5-5", "enabled": false }
  ], "more": false });
  let lines = answered(ROLE_AGENTS, &Step::List, 200, Some(page)).unwrap().lines;

  assert_eq!(
    lines,
    [
      "coder-a · coder · registered · q38fn-dq8",
      "coder-b · coder · claude-code · claude-opus-5-5 · off"
    ]
  );
  assert_eq!(
    answered(ROLE_AGENTS, &Step::List, 200, Some(json!({ "items": [] }))).unwrap().lines,
    ["(none)"]
  );
}

#[test]
fn a_refusal_says_the_route_s_words_and_the_step_past_it() {
  let refused = |status, step: Step| match answered(
    ROLE_AGENTS,
    &step,
    status,
    Some(json!({ "error": "No agent is named coder-z." })),
  ) {
    Err(VerbError::Refused(said)) => said,
    other => panic!("not refused: {other:?}"),
  };

  assert_eq!(
    refused(404, Step::Show("coder-z".to_owned())),
    "role-agent: No agent is named coder-z. `role-agent list` lists the names it holds."
  );
  assert!(
    refused(409, Step::Add("coder-a".to_owned(), vec![]))
      .ends_with("change the one standing with `role-agent set`.")
  );
}
