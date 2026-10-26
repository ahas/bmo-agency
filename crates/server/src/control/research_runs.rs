//! A research asked of agency and the settings every research run is held to (docs/api.md, 「research runs and
//! settings」; docs/research-runs.md): the ask answered at once with the road it went out on, or refused with the
//! road resolution's sentence; the settings the owner keeps, with the search engines' state beside them.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use bmo_agency_client::{ResearchRunAsk, ResearchSettingsChange};

use super::{Call, Refused, Route, answer};
use crate::research::{self, Refused as Launch};

fn bad(why: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, why).with_code("bad-request")
}

fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
  serde_json::to_value(value).unwrap_or_default()
}

/// A research run on a research key, answered at once.
pub struct CreateResearchRun;

impl Route for CreateResearchRun {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/research-runs";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked: ResearchRunAsk =
      serde_json::from_value(call.body().await?).map_err(|why| bad(format!("a research ask: {why}")))?;

    match research::launch(call.shared_ground(), asked).await {
      Ok(launched) => Ok(answer(StatusCode::ACCEPTED, json(&launched))),
      Err(Launch::Ask(why)) => Err(bad(why)),
      Err(Launch::Closed(why)) => {
        Err(Refused::new(StatusCode::SERVICE_UNAVAILABLE, why).with_code("no-login-open"))
      }
      Err(Launch::Failed(why)) => Err(Refused::failed(why)),
    }
  }
}

crate::moved!(CreateResearchRun);

/// The research settings, with the search engines' state.
pub struct ResearchSettings;

impl Route for ResearchSettings {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/research-settings";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let (settings, set) = research::settings(call.store()).await?;
    let engines = research::engines(call.ground()).await;

    Ok(answer(StatusCode::OK, json(&research::view(&settings, set, engines))))
  }
}

crate::moved!(ResearchSettings);

/// The research settings changed, a field left out keeping what it holds.
pub struct SetResearchSettings;

impl Route for SetResearchSettings {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/research-settings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let change: ResearchSettingsChange =
      serde_json::from_value(call.body().await?).map_err(|why| bad(format!("a settings change: {why}")))?;
    let (current, _) = research::settings(call.store()).await?;
    let settings = research::changed(current, &change).map_err(bad)?;

    research::keep(call.store(), &settings).await?;

    let engines = research::engines(call.ground()).await;

    Ok(answer(StatusCode::OK, json(&research::view(&settings, true, engines))))
  }
}

crate::moved!(SetResearchSettings);
