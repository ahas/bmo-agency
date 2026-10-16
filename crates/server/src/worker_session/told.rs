//! What a worker's session tells its consumers (docs/api.md, 「Events」): a session that answered on a
//! lesser model, and a fold that reached a session. Agency publishes them and voices nothing.

use serde::Serialize;

use crate::events::Event;

/// A worker's session answered on a lesser model than it was launched on: the worker, the model it was
/// launched on, the models it answered on, whether its session's record was dropped, and the job it ran
/// for where one is known (docs/cache-fallback.md, 「Healing differs for a worker」).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerDowngraded {
  pub worker: String,
  pub launched: String,
  pub answered_on: Vec<String>,
  pub dropped: bool,
  pub job: Option<i64>,
}

impl Event for WorkerDowngraded {
  const NAME: &'static str = "worker-downgraded";
}

/// A fold reached, or may have reached, a worker's session: whatever a consumer sent that session to hold
/// is folded away with it (docs/sessions.md, 「Session compaction」).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFolded {
  pub session: String,
}

impl Event for SessionFolded {
  const NAME: &'static str = "session-folded";
}
