//! A research line's rounds (docs/launch-lines.md, 「A research run's rounds」; docs/api.md, 「Research
//! drafts」): the policy's questions and a line's row over it, the final question's place, and the prompt
//! each question is asked with.

use agency_server::research_draft::{self, PROCESS_KEY, QUESTION_KEY, Source};
use policy::research::{PastCeiling, RESEARCH};
use serde_json::json;

use crate::scratch::Scratch;

async fn set(db: &sea_orm::DatabaseConnection, key: &str, value: &str) {
  agency_server::queue::settings::set(db, key, value).await.unwrap();
}

async fn process(db: &sea_orm::DatabaseConnection, line: &str) -> research_draft::Process {
  research_draft::process_of(db, line).await.unwrap().unwrap()
}

#[tokio::test]
async fn a_line_reads_the_policy_s_rounds_and_the_console_s_row_stands_over_them() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let search = process(db, "search").await;

  assert_eq!(search.rounds, RESEARCH.draft.line("search").unwrap().rounds);
  assert_eq!(search.ceiling_seconds, RESEARCH.draft.line("search").unwrap().ceiling_seconds);
  assert_eq!(search.past_ceiling, PastCeiling::Final);
  assert_eq!(search.source, Source::Default);
  assert!(research_draft::process_of(db, "no-such-line").await.unwrap().is_none());

  set(
    db,
    &format!("{PROCESS_KEY}search"),
    &json!({ "rounds": ["what", "final"], "ceilingSeconds": 60, "pastCeiling": "last" }).to_string(),
  )
  .await;
  let changed = process(db, "search").await;

  assert_eq!(changed.rounds, ["what", "final"]);
  assert_eq!(changed.ceiling_seconds, 60);
  assert_eq!(changed.past_ceiling, PastCeiling::Last);
  assert_eq!(changed.source, Source::Set);
}

#[tokio::test]
async fn a_question_is_the_console_s_text_over_the_policy_s_and_a_line_may_name_its_own() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let what = RESEARCH.draft.question("what").unwrap().to_owned();

  assert_eq!(research_draft::question_of(db, "what").await.unwrap().as_deref(), Some(what.as_str()));
  assert!(research_draft::question_of(db, "no-such-question").await.unwrap().is_none());

  set(db, &format!("{QUESTION_KEY}what"), "the changed rule").await;
  set(db, &format!("{QUESTION_KEY}mine"), "a question of the console's own").await;

  assert_eq!(
    research_draft::question_of(db, "what").await.unwrap().as_deref(),
    Some("the changed rule")
  );

  let questions = research_draft::questions(db).await.unwrap();

  assert!(questions.iter().any(|(name, text, source)| {
    name == "what" && text == "the changed rule" && *source == Source::Set
  }));
  assert!(questions.iter().any(|(name, _, source)| name == "mine" && *source == Source::Set));
}

#[tokio::test]
async fn the_rounds_must_end_in_the_final_question_hold_it_once_and_name_questions_that_stand() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let mut process = process(db, "search").await;

  process.rounds = vec!["what".to_owned(), "facts".to_owned()];
  assert!(research_draft::refusal(db, &process).await.unwrap().is_some());

  process.rounds = vec!["final".to_owned(), "final".to_owned()];
  assert!(research_draft::refusal(db, &process).await.unwrap().is_some());

  process.rounds = vec!["what".to_owned(), "final".to_owned()];
  process.ceiling_seconds = 0;
  assert!(research_draft::refusal(db, &process).await.unwrap().is_some());

  process.ceiling_seconds = 60;
  process.rounds = vec!["no-such-question".to_owned(), "final".to_owned()];
  assert!(research_draft::refusal(db, &process).await.unwrap().is_some());

  process.rounds = vec!["what".to_owned(), "final".to_owned()];
  assert!(research_draft::refusal(db, &process).await.unwrap().is_none());
}

#[test]
fn a_question_is_asked_over_the_task_and_the_answer_written_last() {
  let alone = research_draft::prompt("what must be found", "the task", &[]);

  assert!(alone.starts_with("what must be found\n\n## The task\n\nthe task"));
  assert!(!alone.contains("## What you wrote last"));

  let later = research_draft::prompt("which sources", "the task", &["first".into(), "second".into()]);

  assert!(later.ends_with("## What you wrote last\n\nsecond"));
  assert!(!later.contains("first"));
}
