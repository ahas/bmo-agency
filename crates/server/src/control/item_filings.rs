//! An item a supervisor files (docs/api.md, 「assignments and jobs」): raised in the job service under the room of
//! the turn the agent carries now, and marked as the owner's ask where that turn is privileged, so neither is the
//! model's to write; the repository it names goes where a launch reads it, the item's `assigneeName`.

use std::collections::BTreeMap;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use bmo_agency_client::{ItemFiled, ItemFilingAsk};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::job_end::{address, token};
use crate::turn_marks;

/// The label an item the owner asked for himself carries, which the job service's unattended labels open.
pub const ASKED_BY: &str = "asked-by";
/// Its value.
pub const OWNER: &str = "owner";

/// A name the ask gives, trimmed; none where it is left out or blank.
fn given(name: Option<&String>) -> Option<&str> {
  name.map(|name| name.trim()).filter(|name| !name.is_empty())
}

/// Where the item goes: the repository the ask names, which a launch reads as the item's `assigneeName`, else
/// the assignee it names; none for host work. An ask naming an assignee other than its repository is refused.
pub fn assignee_of(ask: &ItemFilingAsk) -> Result<Option<&str>, String> {
  match (given(ask.repository.as_ref()), given(ask.assignee_name.as_ref())) {
    (Some(repository), Some(assignee)) if assignee != repository => {
      Err(format!("assigneeName {assignee} is not the repository {repository} the item names"))
    }
    (Some(repository), _) => Ok(Some(repository)),
    (None, assignee) => Ok(assignee),
  }
}

/// The item the job service is asked to raise: the ask as it came, its room the turn's, its `asked-by` label
/// agency's alone, and its `assigneeName` the repository it names.
pub fn item_of(ask: &ItemFilingAsk, mark: turn_marks::Mark) -> Result<Value, String> {
  let assignee = assignee_of(ask)?;
  let mut labels: BTreeMap<String, String> = ask
    .labels
    .iter()
    .filter(|(key, _)| key.as_str() != ASKED_BY)
    .map(|(k, v)| (k.clone(), v.clone()))
    .collect();

  if mark.privileged {
    labels.insert(ASKED_BY.to_owned(), OWNER.to_owned());
  }

  let mut item = json!({
    "title": ask.title,
    "brief": ask.brief,
    "queueName": ask.queue_name,
    "scope": mark.chat_id.to_string(),
    "labels": labels,
  });

  for (field, value) in [
    ("priority", ask.priority.map(Value::from)),
    ("assigneeName", assignee.map(Value::from)),
    ("requesterName", ask.requester_name.clone().map(Value::from)),
  ] {
    if let Some(value) = value {
      item[field] = value;
    }
  }

  Ok(item)
}

/// One item filed for the turn an agent carries now.
pub struct FileItem;

impl Route for FileItem {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/item-filings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let ask: ItemFilingAsk = serde_json::from_value(call.body().await?).map_err(|why| {
      Refused::new(StatusCode::BAD_REQUEST, format!("an item's filing: {why}")).with_code("bad-request")
    })?;
    let Some(mark) = turn_marks::of(&ask.agent_name) else {
      return Err(
        Refused::new(
          StatusCode::CONFLICT,
          format!("{} is carrying no turn to file an item for", ask.agent_name),
        )
        .with_code("no-turn"),
      );
    };
    let item = item_of(&ask, mark).map_err(|why| {
      Refused::new(StatusCode::BAD_REQUEST, format!("an item's filing: {why}")).with_code("bad-request")
    })?;

    if let Some(repository) = given(ask.repository.as_ref())
      && crate::repositories::named(&call.ground().db, repository).await?.is_none()
    {
      return Err(
        Refused::new(StatusCode::UNPROCESSABLE_ENTITY, format!("no repository is named {repository}"))
          .with_code("unknown-repository"),
      );
    }

    let vars = call.ground().vars();
    let (Some(url), Some(bearer)) = (
      address(vars, "BMO_JOB_URL"),
      token(vars, "BMO_JOB_CHAT_TOKEN", "BMO_JOB_CHAT_TOKEN_FILE")
        .or_else(|| token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE")),
    ) else {
      return Err(Refused::failed("no job service is named").with_code("upstream-failed"));
    };
    let upstream = |why: String| Refused::new(StatusCode::BAD_GATEWAY, why).with_code("upstream-failed");
    let answered = reqwest::Client::new()
      .post(format!("{url}/items"))
      .bearer_auth(&bearer)
      .json(&item)
      .send()
      .await
      .map_err(|why| upstream(format!("the job service: {why}")))?;
    let status = answered.status();
    let body: Value = answered.json().await.unwrap_or(Value::Null);

    if !status.is_success() {
      let said = body["error"].as_str().map_or_else(|| status.to_string(), str::to_owned);

      return Err(upstream(format!("the job service refused the item: {said}")));
    }

    let id = body["id"].as_i64().unwrap_or_default();
    let steps: Vec<&str> =
      ask.steps.iter().map(|step| step.trim()).filter(|step| !step.is_empty()).collect();
    // The steps the item's job runs in, saved as its work order (docs/job-review.md, 「Stages」).
    let work_order_refused = if steps.is_empty() {
      None
    } else {
      let order = json!({
        "purpose": ask.title,
        "directive": ask.brief,
        "steps": steps,
        "state": "saved",
      });
      let saved = reqwest::Client::new()
        .put(format!("{url}/items/{id}/work-order"))
        .bearer_auth(&bearer)
        .json(&order)
        .send()
        .await;

      match saved {
        Ok(saved) if saved.status().is_success() => None,
        Ok(saved) => {
          let status = saved.status();
          let said: Value = saved.json().await.unwrap_or(Value::Null);

          Some(said["error"].as_str().map_or_else(|| status.to_string(), str::to_owned))
        }
        Err(why) => Some(why.to_string()),
      }
    };
    let filed = ItemFiled {
      id,
      state: body["state"].as_str().unwrap_or_default().to_owned(),
      scope: body["scope"].as_str().map(str::to_owned),
      labels: body["labels"].clone(),
      work_order_refused,
    };

    Ok(answer(StatusCode::CREATED, serde_json::to_value(filed).unwrap_or_default()))
  }
}

crate::moved!(FileItem);
