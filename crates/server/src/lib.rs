//! The agency service: sessions, places, lines, the co-worker pool, control routes and verbs (docs/api.md).
//! This is its ground: what every handler stands on, the refusal it answers with, the store's place, the
//! clock helper, the deployment's configuration, the clients of the services agency calls, and the bus
//! its events and hooks go through; and the conversational agents' settings as their rows declare them.

pub mod clock;
pub mod config;
pub mod conversational;
pub mod data_move;
pub mod events;
pub mod hooks;
pub mod peers;
pub mod text;

/// The launch lines (docs/launch-lines.md). The lines and the stand-in walk beside them stand on the
/// places, the door, the queue and the control modules, and are declared whole, through `lines/mod.rs`
/// and `stand_in`, together with those; until then the members that stand on nothing but the ground are
/// built here.
pub mod lines {
  pub mod ordered;
  pub mod prefix;
}

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sea_orm::DatabaseConnection;
use serde_json::json;

use crate::config::Config;
use crate::events::Bus;
use crate::hooks::Hooks;
use crate::peers::{PeerFault, Peers};

/// What every handler stands on: the store, the deployment's configuration, the services agency calls,
/// and the bus its events are published on and its hooks registered with.
#[derive(Clone)]
pub struct Service {
  pub db: DatabaseConnection,
  pub config: Arc<Config>,
  pub peers: Peers,
  pub events: Bus,
  pub hooks: Hooks,
}

impl Service {
  /// The service on its store, calling each peer the configuration addresses, with no subscriber and no
  /// hook registered.
  pub fn new(db: DatabaseConnection, config: Config) -> Self {
    let peers = Peers::of(&config.peers);

    Self {
      db,
      config: Arc::new(config),
      peers,
      events: Bus::default(),
      hooks: Hooks::default(),
    }
  }
}

/// A refusal: the status it is answered with, its kebab-case code, and an English sentence naming why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
  pub status: StatusCode,
  pub code: String,
  pub detail: String,
}

impl Refusal {
  pub fn new(status: StatusCode, code: impl Into<String>, detail: impl Into<String>) -> Self {
    Self {
      status,
      code: code.into(),
      detail: detail.into(),
    }
  }

  pub fn bad(code: impl Into<String>, detail: impl Into<String>) -> Self {
    Self::new(StatusCode::BAD_REQUEST, code, detail)
  }

  pub fn forbidden(code: impl Into<String>, detail: impl Into<String>) -> Self {
    Self::new(StatusCode::FORBIDDEN, code, detail)
  }

  pub fn absent(code: impl Into<String>, detail: impl Into<String>) -> Self {
    Self::new(StatusCode::NOT_FOUND, code, detail)
  }

  /// A failure the service did not expect, said in its own words.
  pub fn failed(detail: impl Into<String>) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, "failed", detail)
  }
}

impl std::fmt::Display for Refusal {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out.write_str(&self.detail)
  }
}

impl std::error::Error for Refusal {}

impl From<store::Fault> for Refusal {
  fn from(fault: store::Fault) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, "store-failed", format!("the store: {fault}"))
  }
}

impl From<sea_orm::DbErr> for Refusal {
  fn from(error: sea_orm::DbErr) -> Self {
    store::Fault::from(error).into()
  }
}

impl From<PeerFault> for Refusal {
  fn from(fault: PeerFault) -> Self {
    let (status, code) = match &fault {
      PeerFault::Refused { .. } => (StatusCode::CONFLICT, "peer-refused"),
      PeerFault::Unreachable { .. } => (StatusCode::BAD_GATEWAY, "peer-unreachable"),
      PeerFault::Failed { .. } | PeerFault::Malformed { .. } => (StatusCode::BAD_GATEWAY, "peer-failed"),
    };

    Self::new(status, code, fault.to_string())
  }
}

impl IntoResponse for Refusal {
  fn into_response(self) -> Response {
    (self.status, Json(json!({ "code": self.code, "error": self.detail }))).into_response()
  }
}

/// The store's place as every reader of it reads it: the address `BMO_ARCHIVE_URL` names, else the one
/// the data root's `env/archive-url` holds, or none where neither names one (docs/store.md,
/// 「Connecting」).
pub fn archive_place(data_root: &Path) -> Option<String> {
  let held = |text: String| Some(text.trim().to_owned()).filter(|text| !text.is_empty());

  std::env::var("BMO_ARCHIVE_URL")
    .ok()
    .and_then(held)
    .or_else(|| std::fs::read_to_string(data_root.join("env/archive-url")).ok().and_then(held))
}

/// Now, in the epoch milliseconds agency's own columns keep.
pub fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}
