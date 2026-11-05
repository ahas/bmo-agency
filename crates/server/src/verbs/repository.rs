//! `bmo-agency repository remove <name>`: a repository's registration taken away through the server's own
//! route (docs/repositories.md, 「What a repository row holds」). The route refuses a removal while a job holds
//! one of the repository's checkouts, and this verb prints what it answered either way; nothing on disk — the
//! base checkout, a worktree, a deploy tree — is touched.

use reqwest::Method;

use super::{Answer, CONTROL_TOKEN, VerbError, control_call, control_token};

/// What the route answered a removal: the line naming what went, or the route's refusal.
pub fn removed(name: &str, status: u16, answer: Option<serde_json::Value>) -> Result<Answer, VerbError> {
  if (200..300).contains(&status) {
    return Ok(Answer::from(vec![format!("repository remove: removed {name}.")]));
  }

  let why = answer
    .as_ref()
    .and_then(|answer| answer.get("error"))
    .and_then(serde_json::Value::as_str)
    .map_or_else(|| format!("agency's server answered {status}"), str::to_owned);

  Err(VerbError::Refused(format!("repository remove: {why}")))
}

/// The repository `name` removed by the server's `DELETE /repositories/{repositoryName}` route.
pub async fn run(name: &str) -> Result<Answer, VerbError> {
  let name = name.trim();

  if name.is_empty() {
    return Err(VerbError::Refused("repository remove: name the repository to remove.".to_owned()));
  }

  let Some(token) = control_token() else {
    return Err(VerbError::Refused(format!(
      "repository remove: {CONTROL_TOKEN} is not set in this process."
    )));
  };
  let path = format!("/repositories/{name}");
  let (status, answer) = control_call(&token, Method::DELETE, &path, None).await.map_err(|why| {
    VerbError::Refused(format!("repository remove: agency's server did not answer: {why}"))
  })?;

  removed(name, status, answer)
}
