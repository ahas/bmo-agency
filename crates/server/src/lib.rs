//! The agency service: sessions, places, lines, the co-worker pool, control routes and verbs (docs/api.md).
//! This is its ground: what every handler stands on, the refusal it answers with, the store's place, the
//! clock helper, the deployment's configuration, the clients of the services agency calls, the bus its
//! events and hooks go through, and its readiness; the conversational agents' settings as their rows
//! declare them; and the members of the places and the watches that stand on nothing but this.

pub mod chat_stand_in;
pub mod clock;
pub mod command_reading;

/// A companion's runs (docs/companions.md): its guard, its handoff, the point its reset cuts it off at, its
/// runs going, its room sessions, and what its reset tells the units holding the rest of it.
pub mod companion {
  pub mod going;
  pub mod guard;
  pub mod handoff;
  pub mod overtaken;
  pub mod reset;
  pub mod run;
  pub mod sessions;
}
pub mod config;
pub mod control;
pub mod conversational;
pub mod data_move;
pub mod doctor;
pub mod door;
pub mod events;
pub mod gate_kinds;
pub mod herdr_backend;
pub mod hooks;
pub mod host_run;

/// What a job's Claude turn comes to (docs/run-records.md, 「Why a run died」), the walk of the logins that
/// carries it, the reset a worker's sessions are carried out by, and the OpenRouter tap and what it read.
pub mod job_run {
  pub mod claude {
    pub mod failure;
    pub mod reset;
    pub mod turn;
    pub mod walk;
  }
  pub mod openrouter;
  pub mod openrouter_tap;
}

/// The keepalive agency keeps: the logins' watch, the co-worker pool's warming of its reserved places, and
/// the owner's warming switch that stops it.
pub mod keepalive {
  pub mod logins;
  pub mod pool;
  pub mod switch;
}
pub mod lines;
pub mod mcp;
pub mod openapi;
pub mod order_run;
pub mod peers;
pub mod places;
pub mod queue;
pub mod readiness;
pub mod research_draft;
pub mod rest;
pub mod rooms;
pub mod run_clock;
pub mod run_ledger;
pub mod run_log;
pub mod seeds;
pub mod session_output;
pub mod session_questions;
pub mod stand_in;
pub mod text;
pub mod usage_windows;

pub mod verbs;

/// The machine's watches agency keeps: the links to the work nodes (docs/work-nodes.md, 「The link」), and
/// this machine's room and the pressure on it (docs/agent-control.md, 「Room on the machine」).
pub mod watches {
  pub mod links;
  pub mod machine;
  pub mod pressure;
}
pub mod wording;
pub mod worker_session;
pub mod worker_words;

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
use crate::readiness::Readiness;

/// What every handler stands on: the store, the deployment's configuration, the services agency calls,
/// the bus its events are published on and its hooks registered with, and whether its places have
/// stood once since it started.
#[derive(Clone)]
pub struct Service {
  pub db: DatabaseConnection,
  pub config: Arc<Config>,
  pub peers: Peers,
  pub events: Bus,
  pub hooks: Hooks,
  pub readiness: Arc<Readiness>,
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
      readiness: Arc::default(),
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

/// The store's place as every reader of it reads it (docs/store.md, 「Connecting」), or none where nothing
/// names one.
pub fn archive_place(data_root: &Path) -> Option<String> {
  store::place::place(data_root)
}

/// Now, in the epoch milliseconds agency's own columns keep.
pub fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}
