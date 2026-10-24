//! The rounds of a research line (docs/api.md, 「Research drafts」; docs/launch-lines.md, 「A research
//! run's rounds」): the lines' questions as the policy gives them and as the console changes them, and
//! the catalogue of questions they name. A research run keeps no receiver, so these are agency's own
//! rows; the console reads them in the receiver's draft-process shape.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::queue::settings;
use crate::research_draft::{self as drafts, Change, PROCESS_KEY, QUESTION_KEY, Source};

fn not_found(what: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, format!("no research draft is called {what}")).with_code("not-found")
}

fn row(process: &drafts::Process) -> Value {
  json!({
    "line": process.line,
    "rounds": process.rounds,
    "ceilingSeconds": process.ceiling_seconds,
    "pastCeiling": process.past_ceiling.word(),
    "source": process.source.word(),
  })
}

fn page(items: Vec<Value>) -> Response {
  answer(StatusCode::OK, json!({ "items": items, "more": false }))
}

fn failed(why: impl Into<String>) -> Refused {
  Refused::failed(why.into())
}

/// Every research line's rounds.
pub struct DraftProcesses;

impl Route for DraftProcesses {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/draft-processes";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let processes = drafts::processes(call.store()).await?;

    Ok(page(processes.iter().map(row).collect()))
  }
}

/// One research line's rounds changed, each field left out keeping what the line holds.
pub struct SetDraftProcess;

impl Route for SetDraftProcess {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/draft-processes/{line}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let line = call.param("line").unwrap_or_default().to_owned();
    let Some(current) = drafts::process_of(call.store(), &line).await? else {
      return Err(not_found(&line));
    };
    let body = call.body().await?;
    let change: Change = serde_json::from_value(body).map_err(|why| failed(why.to_string()))?;
    let past_ceiling = match change.past_ceiling.as_deref() {
      None => current.past_ceiling,
      Some("final") => policy::research::PastCeiling::Final,
      Some("last") => policy::research::PastCeiling::Last,
      Some(other) => {
        return Err(
          Refused::new(StatusCode::BAD_REQUEST, format!("{other} is no past-ceiling word; final or last"))
            .with_code("bad-request"),
        );
      }
    };
    let process = drafts::Process {
      line: line.clone(),
      rounds: change.rounds.unwrap_or(current.rounds),
      ceiling_seconds: change.ceiling_seconds.unwrap_or(current.ceiling_seconds),
      past_ceiling,
      source: Source::Set,
    };

    if let Some(why) = drafts::refusal(call.store(), &process).await? {
      return Err(Refused::new(StatusCode::BAD_REQUEST, why).with_code("bad-request"));
    }

    let held = json!({
      "rounds": process.rounds,
      "ceilingSeconds": process.ceiling_seconds,
      "pastCeiling": process.past_ceiling.word(),
    });

    settings::set(call.store(), &format!("{PROCESS_KEY}{line}"), &held.to_string()).await?;

    Ok(answer(StatusCode::OK, row(&process)))
  }
}

/// A line's own rounds taken back, the policy's standing again.
pub struct ResetDraftProcess;

impl Route for ResetDraftProcess {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/draft-processes/{line}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let line = call.param("line").unwrap_or_default().to_owned();

    if settings::get(call.store(), &format!("{PROCESS_KEY}{line}")).await?.is_none() {
      return Err(not_found(&line));
    }

    settings::forget(call.store(), &format!("{PROCESS_KEY}{line}")).await?;

    Ok(answer(StatusCode::NO_CONTENT, Value::Null))
  }
}

/// Every question the catalogue holds.
pub struct DraftQuestions;

impl Route for DraftQuestions {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/draft-questions";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let questions = drafts::questions(call.store()).await?;

    Ok(page(
      questions
        .into_iter()
        .map(|(name, text, source)| json!({ "name": name, "text": text, "source": source.word() }))
        .collect(),
    ))
  }
}

/// One question's text changed, or added under a name the policy does not hold.
pub struct SetDraftQuestion;

impl Route for SetDraftQuestion {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/draft-questions/{name}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("name").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let text = body
      .get("text")
      .and_then(Value::as_str)
      .map(str::trim)
      .filter(|text| !text.is_empty())
      .ok_or_else(|| {
        Refused::new(StatusCode::BAD_REQUEST, "text (a non-empty string) is required")
          .with_code("bad-request")
      })?
      .to_owned();

    settings::set(call.store(), &format!("{QUESTION_KEY}{name}"), &text).await?;

    Ok(answer(StatusCode::OK, json!({ "name": name, "text": text, "source": "set" })))
  }
}

crate::moved!(DraftProcesses);
crate::moved!(SetDraftProcess);
crate::moved!(ResetDraftProcess);
crate::moved!(DraftQuestions);
crate::moved!(SetDraftQuestion);
