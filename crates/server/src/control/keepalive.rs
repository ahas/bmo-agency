//! Agency's own switches (docs/api.md, 「settings」): the research supervisor, the downgrade return and the
//! console secret the DeepSeek top-up reads its card from, read and changed. No switch warms or stops everything
//! at once: a supervisor's warming is its own row's (docs/keepalive-schedule.md).

use std::path::{Component, Path};

use axum::http::{Method, StatusCode};
use axum::response::Response;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::lines::token::{RESEARCH_SUPERVISOR, research_supervisor};
use crate::queue::settings;

/// The settings key the DeepSeek top-up's card secret stands under.
const DEEPSEEK_CARD_SECRET: &str = "deepseek_card_secret";

// The switches as a caller reads them.
async fn view(ground: &crate::places::Ground) -> Result<Value, Refused> {
  let research = research_supervisor(ground).await.map_err(Refused::failed)?;
  let card_secret = settings::get(&ground.db, DEEPSEEK_CARD_SECRET).await.map_err(Refused::failed)?;

  Ok(json!({
    "researchSupervisor": research,
    "downgradeReturn": Value::Null,
    "deepseekCardSecret": card_secret,
  }))
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

/// A secret's name as the console's store keeps it: one plain file name of the administrator's space,
/// never a path reaching past it.
fn is_secret_name(name: &str) -> bool {
  let parts: Vec<Component> = Path::new(name).components().collect();

  matches!(parts.as_slice(), [Component::Normal(part)] if *part == name)
}

async fn change_research_supervisor(store: &DatabaseConnection, asked: &Value) -> Result<(), Refused> {
  match asked {
    Value::Null => Ok(settings::forget(store, RESEARCH_SUPERVISOR).await?),
    Value::String(name) if crate::supervisors::named(store, name).await?.is_some() => {
      super::set_setting(store, RESEARCH_SUPERVISOR, name).await
    }
    _ => Err(
      Refused::new(
        StatusCode::BAD_REQUEST,
        "The research supervisor is a supervisor the store holds, or null.",
      )
      .with_field("researchSupervisor"),
    ),
  }
}

async fn change_card_secret(store: &DatabaseConnection, asked: &Value) -> Result<(), Refused> {
  match asked {
    Value::Null => Ok(settings::forget(store, DEEPSEEK_CARD_SECRET).await?),
    Value::String(name) if is_secret_name(name) => {
      super::set_setting(store, DEEPSEEK_CARD_SECRET, name).await
    }
    _ => Err(
      Refused::new(
        StatusCode::BAD_REQUEST,
        "The DeepSeek card secret is a console secret's name, one plain file name, or null.",
      )
      .with_field("deepseekCardSecret"),
    ),
  }
}

/// Agency's own switches changed as the body names them: the research supervisor, a supervisor the store
/// holds, or null for none; the DeepSeek top-up's card secret, a console secret's name, or null for none. A
/// switch the body leaves out stands as it is.
pub struct SettingsWrite;

impl Route for SettingsWrite {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/settings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    if let Some(asked) = body.get("researchSupervisor") {
      change_research_supervisor(call.store(), asked).await?;
    }

    if let Some(asked) = body.get("deepseekCardSecret") {
      change_card_secret(call.store(), asked).await?;
    }

    Ok(answer(StatusCode::OK, view(&call.shared_ground()).await?))
  }
}

crate::moved!(SettingsWrite);
