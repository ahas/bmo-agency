//! Agency's own switches (docs/api.md, 「settings」): the research supervisor and the downgrade return, read and
//! changed. No switch warms or stops everything at once: a supervisor's warming is its own row's
//! (docs/keepalive-schedule.md).

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::lines::token::{RESEARCH_SUPERVISOR, research_supervisor};

// The switches as a caller reads them.
async fn view(ground: &crate::places::Ground) -> Result<Value, Refused> {
  let research = research_supervisor(ground).await.map_err(Refused::failed)?;

  Ok(json!({ "researchSupervisor": research, "downgradeReturn": Value::Null }))
}

/// Agency's own switches.
pub struct SettingsRead;

impl Route for SettingsRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/settings";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    Ok(answer(StatusCode::OK, view(&call.shared_ground()).await?))
  }
}

crate::moved!(SettingsRead);

/// Agency's own switches changed as the body names them: the research supervisor, a supervisor the store
/// holds, or null for none.
pub struct SettingsWrite;

impl Route for SettingsWrite {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/settings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    match body.get("researchSupervisor") {
      None => {}
      Some(Value::Null) => {
        store::run(
          call.store(),
          r#"DELETE FROM "settings" WHERE "key" = $1"#,
          vec![RESEARCH_SUPERVISOR.into()],
        )
        .await?;
      }
      Some(Value::String(name)) if crate::supervisors::named(call.store(), name).await?.is_some() => {
        super::set_setting(call.store(), RESEARCH_SUPERVISOR, name).await?;
      }
      Some(_) => {
        return Err(
          Refused::new(
            StatusCode::BAD_REQUEST,
            "The research supervisor is a supervisor the store holds, or null.",
          )
          .with_field("researchSupervisor"),
        );
      }
    }

    Ok(answer(StatusCode::OK, view(&call.shared_ground()).await?))
  }
}

crate::moved!(SettingsWrite);
