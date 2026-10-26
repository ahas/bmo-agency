//! The rounds a research line asks after its first answer (docs/launch-lines.md, 「A research run's
//! rounds」): the policy's questions and the lines that ask them, a line's own row standing over the
//! policy's, and the prompt each question is asked with. A research run keeps no receiver, so agency
//! asks its rounds itself; the console reads and changes them through the control routes in
//! `control::research_drafts`.

use policy::research::{Draft, PastCeiling, RESEARCH};
use serde::Deserialize;

use crate::Refusal;
use crate::queue::settings;

/// The prefix a line's own row of rounds is held under.
pub const PROCESS_KEY: &str = "research-draft:";

/// The prefix a question's changed text is held under.
pub const QUESTION_KEY: &str = "research-question:";

/// Where a row came from: the policy, or the console's own row standing over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
  Default,
  Set,
}

impl Source {
  pub fn word(self) -> &'static str {
    match self {
      Self::Default => "default",
      Self::Set => "set",
    }
  }
}

/// One research line's rounds as they stand now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Process {
  pub line: String,
  pub rounds: Vec<String>,
  pub ceiling_seconds: i64,
  pub past_ceiling: PastCeiling,
  pub source: Source,
}

/// A change the console sends: each field left out keeps what the row holds.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
  #[serde(default)]
  pub rounds: Option<Vec<String>>,
  #[serde(default)]
  pub ceiling_seconds: Option<i64>,
  #[serde(default)]
  pub past_ceiling: Option<String>,
}

/// The policy's draft, as its reader gives it.
pub fn draft() -> &'static Draft {
  &RESEARCH.draft
}

/// One line's rounds as they stand: the policy's row, with the console's over it where it set one.
pub async fn process_of(
  db: &impl sea_orm::ConnectionTrait,
  line: &str,
) -> Result<Option<Process>, Refusal> {
  let Some(default) = draft().line(line) else { return Ok(None) };
  let held = settings::get(db, &format!("{PROCESS_KEY}{line}")).await?;
  let changed = held.as_deref().and_then(|value| serde_json::from_str::<Change>(value).ok());

  let Some(changed) = changed else {
    return Ok(Some(Process {
      line: line.to_owned(),
      rounds: default.rounds.clone(),
      ceiling_seconds: default.ceiling_seconds,
      past_ceiling: default.past_ceiling,
      source: if held.is_some() { Source::Set } else { Source::Default },
    }));
  };

  Ok(Some(Process {
    line: line.to_owned(),
    rounds: changed.rounds.unwrap_or_else(|| default.rounds.clone()),
    ceiling_seconds: changed.ceiling_seconds.unwrap_or(default.ceiling_seconds),
    past_ceiling: changed
      .past_ceiling
      .as_deref()
      .and_then(PastCeiling::read)
      .unwrap_or(default.past_ceiling),
    source: Source::Set,
  }))
}

/// Every line's rounds, the policy's lines in their order, each with the console's row over it.
pub async fn processes(db: &impl sea_orm::ConnectionTrait) -> Result<Vec<Process>, Refusal> {
  let mut processes = Vec::new();

  for line in draft().lines.keys() {
    if let Some(process) = process_of(db, line).await? {
      processes.push(process);
    }
  }

  Ok(processes)
}

/// A question's text as it stands: the console's row where it set one, else the policy's.
pub async fn question_of(
  db: &impl sea_orm::ConnectionTrait,
  name: &str,
) -> Result<Option<String>, Refusal> {
  if let Some(text) = settings::get(db, &format!("{QUESTION_KEY}{name}")).await? {
    return Ok(Some(text));
  }

  Ok(draft().question(name).map(str::to_owned))
}

/// Every question the catalogue holds, the policy's in their order and the console's own after them.
pub async fn questions(
  db: &impl sea_orm::ConnectionTrait,
) -> Result<Vec<(String, String, Source)>, Refusal> {
  let mut questions: Vec<(String, String, Source)> = Vec::new();

  for (name, text) in &draft().questions {
    let held = settings::get(db, &format!("{QUESTION_KEY}{name}")).await?;

    questions.push(match held {
      Some(text) => (name.clone(), text, Source::Set),
      None => (name.clone(), text.clone(), Source::Default),
    });
  }

  let mut own = settings::named(db, QUESTION_KEY).await?;
  own.sort();

  for name in own {
    if draft().question(&name).is_none()
      && let Some(text) = settings::get(db, &format!("{QUESTION_KEY}{name}")).await?
    {
      questions.push((name, text, Source::Set));
    }
  }

  Ok(questions)
}

/// Why a line's rounds do not stand, or nothing where they do: a list that ends in the final question and
/// holds it nowhere else, a ceiling above nothing, and a past-ceiling word the policy names.
pub async fn refusal(
  db: &impl sea_orm::ConnectionTrait,
  process: &Process,
) -> Result<Option<String>, Refusal> {
  if process.rounds.is_empty() || process.rounds.last().map(String::as_str) != Some(Draft::FINAL) {
    return Ok(Some("the rounds must end in the final question".to_owned()));
  }

  if process.rounds.iter().filter(|name| name.as_str() == Draft::FINAL).count() != 1 {
    return Ok(Some("the final question stands once, at the end".to_owned()));
  }

  if process.ceiling_seconds <= 0 {
    return Ok(Some("the ceiling must be a positive number of seconds".to_owned()));
  }

  for name in &process.rounds {
    if question_of(db, name).await?.is_none() {
      return Ok(Some(format!("no question is called {name}")));
    }
  }

  Ok(None)
}

/// The prompt one question of a research process is asked with: the question, the task as the run was
/// given it, and the answer the run wrote last. The session holds the earlier answers, so none of them is
/// carried again.
pub fn prompt(question: &str, task: &str, so_far: &[String]) -> String {
  let mut text = format!("{question}\n\n## The task\n\n{task}");

  if let Some(last) = so_far.last() {
    text.push_str("\n\n## What you wrote last\n\n");
    text.push_str(last);
  }

  text
}
