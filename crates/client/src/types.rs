//! The agency API's bodies and members as Rust types.
//!
//! Every route of `docs/openapi.json` names one of these in its request or answer; the document is
//! generated from them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PromptDocumentKind {
  #[serde(rename = "base")]
  Base,
  #[serde(rename = "rulebook")]
  Rulebook,
  #[serde(rename = "persona")]
  Persona,
  #[serde(rename = "agent")]
  Agent,
  #[serde(rename = "system")]
  System,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SessionScope {
  #[serde(rename = "room")]
  Room,
  #[serde(rename = "agent")]
  Agent,
  #[serde(rename = "role")]
  Role,
  #[serde(rename = "conversation")]
  Conversation,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TurnState {
  #[serde(rename = "running")]
  Running,
  #[serde(rename = "ended")]
  Ended,
  #[serde(rename = "interrupted")]
  Interrupted,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TurnInputSaid {
  #[serde(rename = "delivered")]
  Delivered,
  #[serde(rename = "waiting")]
  Waiting,
  #[serde(rename = "refused")]
  Refused,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RunState {
  #[serde(rename = "open")]
  Open,
  #[serde(rename = "launched")]
  Launched,
  #[serde(rename = "stopping")]
  Stopping,
  #[serde(rename = "closed")]
  Closed,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum OvertimeRequestVerdict {
  #[serde(rename = "granted")]
  Granted,
  #[serde(rename = "refused")]
  Refused,
  #[serde(rename = "None")]
  None,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SeatRequestState {
  #[serde(rename = "waiting")]
  Waiting,
  #[serde(rename = "reserved")]
  Reserved,
  #[serde(rename = "active")]
  Active,
  #[serde(rename = "released")]
  Released,
  #[serde(rename = "cancelled")]
  Cancelled,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ReservationState {
  #[serde(rename = "held")]
  Held,
  #[serde(rename = "released")]
  Released,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PlaceState {
  #[serde(rename = "taken")]
  Taken,
  #[serde(rename = "prepared")]
  Prepared,
  #[serde(rename = "remembered")]
  Remembered,
  #[serde(rename = "retired")]
  Retired,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum PinCheckKind {
  #[serde(rename = "auto")]
  Auto,
  #[serde(rename = "provider")]
  Provider,
  #[serde(rename = "model")]
  Model,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssignmentState {
  #[serde(rename = "held")]
  Held,
  #[serde(rename = "running")]
  Running,
  #[serde(rename = "released")]
  Released,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum JobState {
  #[serde(rename = "starting")]
  Starting,
  #[serde(rename = "running")]
  Running,
  #[serde(rename = "ended")]
  Ended,
  #[serde(rename = "stopped")]
  Stopped,
}

/// What chose a job's provider and model: its item's `model` label, its role's forced provider, its
/// repository's pin or its role's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum JobChoice {
  #[serde(rename = "item")]
  Item,
  #[serde(rename = "forced")]
  Forced,
  #[serde(rename = "pin")]
  Pin,
  #[serde(rename = "order")]
  Order,
}

/// Error:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Error")]
pub struct ApiError {
  pub code: String,
  pub error: String,
}

/// Health:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
  pub ready: bool,
  pub reason: Option<String>,
  pub version: String,
}

/// Event:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Event {
  pub id: i64,
  pub kind: String,
  /// epoch milliseconds
  pub at: i64,
  pub data: Value,
}

/// EventPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventPage {
  pub items: Vec<Event>,
  pub more: bool,
  /// The notice that events after the number asked are no longer kept, or that it is a number the service
  /// never gave; none otherwise. The reader reads afresh the state it follows.
  #[serde(default)]
  pub missed: Option<EventsMissed>,
}

/// EventsMissed: what the stream's `events.missed` notice and a page's `missed` carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventsMissed {
  /// the number the reader named
  pub after: i64,
  /// the oldest number kept, none where none is
  pub kept_from: Option<i64>,
}

/// Agent:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
  pub name: String,
  pub label: String,
  pub role: String,
  pub gates: Value,
  pub reasoning: Value,
  pub voice: Option<String>,
  /// epoch milliseconds
  pub reset_at: Option<i64>,
  pub stand_in_available: bool,
  /// the id of the login account that owns it, none for the administrator's
  pub owner_account: Option<i64>,
}

/// PromptDocument:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptDocument {
  pub name: String,
  pub kind: PromptDocumentKind,
  pub agent_name: Option<String>,
  pub version: String,
  pub text: String,
}

/// DocumentMove:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMove {
  pub paths: Vec<Value>,
  /// epoch milliseconds
  pub at: i64,
  pub agent_name: Option<String>,
  pub room_ids: Vec<Value>,
}

/// Session:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Session {
  pub id: String,
  pub agent_name: String,
  pub scope: SessionScope,
  pub room_id: Option<i64>,
  pub provider: String,
  pub model: String,
  pub place: Option<String>,
  pub generation: i64,
  pub holder: Option<String>,
  pub prefix_change_reason: Option<String>,
  pub options: Value,
  /// The share of its model's window the session may fill before it is folded once the turn that crossed it is
  /// over: the compaction threshold of its supervisor or its role, read afresh with each read.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
  /// The context its last turn left, `carried` tokens against the model's `window`, or none.
  #[serde(default)]
  pub context: Option<Value>,
  /// Whether the session's program compacts itself at its fold ratio and takes no fold call.
  #[serde(default)]
  pub folds_itself: bool,
  /// The supervisor the session runs for, or none.
  #[serde(default)]
  pub supervisor: Option<String>,
  /// The role the session runs for, or none.
  #[serde(default)]
  pub role: Option<String>,
  /// What the record of every run of a role's session holds, its supervisor's among them where it is a
  /// supervisor's double; none for any other scope and wherever a session is read rather than started.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub carries: Option<SessionCarries>,
}

/// SessionCarries: the capabilities and the namespaces a role's session's record holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionCarries {
  pub capabilities: Vec<String>,
  pub namespaces: Vec<String>,
}

/// Turn:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
  pub id: i64,
  pub session_id: String,
  pub prompt: String,
  pub context: Value,
  pub state: TurnState,
  pub delivery: Option<String>,
  pub outcome: Option<String>,
  /// The word the turn's wait ended on, typed: for a turn put into a program's pane `answered`, `silent`,
  /// `errored`, `vanished`, `program-lost`, `silent-peer`, `blocked`, `untrusted`, `unsubmitted`,
  /// `overran` or `stopped`; for any other its outcome's word; none while the turn runs.
  #[serde(default)]
  pub ending: Option<String>,
  /// Why a turn with no answer has none; none where it answered or still runs.
  #[serde(default)]
  pub reason: Option<String>,
  pub answer: Option<String>,
  pub text: Vec<Value>,
  /// What the running turn waits for before its program is put to work; none while nothing holds it back
  /// and once it has ended.
  #[serde(default)]
  pub waiting: Option<TurnWaiting>,
}

/// TurnWaiting: what a running turn waits for, typed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnWaiting {
  /// What it waits for: `card`, a card the arbiter's master has not yet granted.
  #[serde(rename = "for")]
  pub kind: TurnWaitKind,
  /// The service whose grant it waits on.
  pub service: String,
  /// Why it waits, as it stands now.
  pub reason: String,
  /// epoch milliseconds: when the wait began
  pub since: i64,
}

/// One of the values the API accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TurnWaitKind {
  #[serde(rename = "card")]
  Card,
}

/// TurnInput:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnInput {
  pub words: String,
  pub said: TurnInputSaid,
}

/// Answer:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Answer")]
pub struct TurnAnswer {
  pub question_id: String,
  pub answer: Value,
}

/// Fold:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Fold {
  pub session_id: String,
  /// epoch milliseconds
  pub at: i64,
}

/// TurnContextAsk:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnContextAsk {
  pub session_id: String,
  pub text: String,
}

/// TurnContext:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnContext {
  pub context: Option<String>,
}

/// Run:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Run {
  pub id: i64,
  pub name: String,
  pub agent_name: String,
  pub unit_name: String,
  pub worker_name: Option<String>,
  pub job_id: Option<i64>,
  pub state: RunState,
  /// epoch milliseconds
  pub started_at: i64,
  /// epoch milliseconds
  pub ended_at: Option<i64>,
  pub outcome: Option<String>,
  /// epoch milliseconds
  pub cap_ends_at: Option<i64>,
  /// epoch milliseconds
  pub kept_until: Option<i64>,
  pub context_tokens: Option<i64>,
  /// What the run's turn spent, the four figures of `usage` together; none on an open run or where its road
  /// reported no figures.
  pub spent_tokens: Option<i64>,
  /// What the run's turn spent, apart.
  #[serde(default)]
  pub usage: Option<RunUsage>,
  pub counts: Value,
  /// The room the run ran for.
  #[serde(default)]
  pub room_id: Option<i64>,
  /// The round of the room the run belongs to, as the turn named it.
  #[serde(default)]
  pub round: Option<String>,
  /// The message the run's answer went out as once attached, the message the turn answered until then.
  #[serde(default)]
  pub message_id: Option<i64>,
}

/// RunUsage: what a chat turn or a job's turn spent, each figure summed over the models that answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunUsage {
  pub input_tokens: i64,
  pub output_tokens: i64,
  pub cache_read_tokens: i64,
  pub cache_write_tokens: i64,
}

/// LogLine:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
  pub id: i64,
  pub run_id: i64,
  pub job_id: Option<i64>,
  pub agent_name: String,
  pub kind: String,
  pub text: String,
  /// epoch milliseconds
  pub at: i64,
  pub fields: Value,
  /// The metadata the writer kept beside the line, the JSON object itself or null where it kept none.
  pub metadata: Value,
  /// The room of the ledger's run the line was written under, where the ledger holds it.
  #[serde(default)]
  pub room_id: Option<i64>,
  /// That run's round.
  #[serde(default)]
  pub round: Option<String>,
  /// That run's message.
  #[serde(default)]
  pub message_id: Option<i64>,
}

/// Execution:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
  pub id: i64,
  pub run_id: i64,
  pub command: String,
  pub state: String,
  pub cancelled: bool,
}

/// OvertimeRequest:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OvertimeRequest {
  pub id: i64,
  pub run_id: i64,
  pub minutes: i64,
  pub why: String,
  pub verdict: Option<OvertimeRequestVerdict>,
  /// epoch milliseconds
  pub closed_at: Option<i64>,
}

/// StopMark:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StopMark {
  pub job_id: i64,
  pub reason: String,
  /// epoch milliseconds
  pub at: i64,
}

/// Pool:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pool {
  pub buckets: Vec<Value>,
  pub workers: Vec<Value>,
  /// epoch milliseconds
  pub read_at: i64,
}

/// SeatRequest: a seat asked of the pool: `waiting` in its bucket's line, `reserved` under a reservation,
/// `active` once let in, and `released` or `cancelled` once given back; the provider, model, effort and tier
/// its work key resolved to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequest {
  pub token: String,
  pub source: String,
  pub owner: String,
  pub role: String,
  pub provider: String,
  pub model: String,
  /// None where the mapping it resolved to names no effort.
  pub effort: Option<String>,
  pub tier: String,
  pub work_key: Option<String>,
  pub state: SeatRequestState,
  pub child_id: Option<String>,
  pub outcome: Option<String>,
  /// The session a worker's own turn continues, or none where it opens a fresh one.
  pub session: Option<String>,
  /// One line naming the work the seat is asked for, where its asker named one.
  pub title: Option<String>,
  /// The agent that asked for the seat, where its asker named one.
  pub asked_by: Option<String>,
  /// The agent the seat's child runs on, where its binding named one.
  pub agent_name: Option<String>,
}

/// DelegationStage: where a delegated task stands: `waiting` while its seat waits in the line, `running`
/// once it is let in, `ended` once it is finished or cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DelegationStage {
  Waiting,
  Running,
  Ended,
}

/// Delegation: a task a supervisor delegated, as its seat stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Delegation {
  pub token: String,
  pub title: Option<String>,
  pub asked_by: Option<String>,
  pub stage: DelegationStage,
  /// The agent its turn runs on; null before it is bound.
  pub agent_name: Option<String>,
  pub role: String,
  /// Epoch milliseconds.
  pub asked_at: i64,
  /// Epoch milliseconds; null until it is let in.
  pub running_since: Option<i64>,
  /// Epoch milliseconds; null until it ends.
  pub ended_at: Option<i64>,
  /// What it came to, once ended.
  pub outcome: Option<String>,
}

/// DelegationPage: the ones still waiting or running in the order they were asked, then the latest ended,
/// newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DelegationPage {
  pub items: Vec<Delegation>,
}

/// Reservation:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
  pub token: String,
  pub owner: String,
  pub role: String,
  pub tier: String,
  pub count: i64,
  pub state: ReservationState,
  pub outcome: Option<String>,
}

/// Place:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Place {
  pub id: String,
  pub path: String,
  pub node_name: Option<String>,
  pub slot: Option<i64>,
  pub role: Option<String>,
  pub state: PlaceState,
  /// any JSON value, or null
  pub last_write: Option<Value>,
}

/// ProviderQuota: what a provider has left as an order reads it at the default share.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuota {
  /// Whether it stands above the share.
  pub has: bool,
  /// A fraction from 0 to 1, or null where the provider reports none.
  pub left: Option<f64>,
  /// The name of the window that counted, or null.
  pub window: Option<String>,
  /// What its balance reader last read, for a provider that keeps a balance, or null.
  pub balance: Option<f64>,
  /// When the counting window resets, in epoch seconds, or null.
  pub resets_at: Option<i64>,
}

/// Provider: one provider agency carries, with its switch, what it declares and what it has left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
  pub name: String,
  /// The kebab-case word a screen words the provider by: its name.
  #[serde(default)]
  pub code: String,
  /// The English name a log keeps.
  #[serde(default)]
  pub label: String,
  pub enabled: bool,
  /// Whether its models are the registry's registrations alone.
  pub registered: bool,
  /// Whether its models are served by agency's model gateway.
  pub gateway: bool,
  /// The launch lines its program carries.
  pub lines: Vec<String>,
  pub quota: ProviderQuota,
  /// Why it cannot answer now, the English sentence, or null where it can.
  pub reason: Option<String>,
  /// The kebab-case code of why it cannot answer now, or null where it can.
  pub reason_code: Option<String>,
  /// The compaction threshold a row whose order begins on it is given where its add names none.
  pub fold_ratio_default: f64,
}

/// Model: one registration of the registry's provider and its health.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub key: String,
  /// The registration's own name, which an order's entry and a repository's pin name.
  pub name: String,
  pub provider: String,
  pub model: String,
  /// The arbiter service its engine is, where it names one: its instances are those the arbiter's master
  /// reports standing, and it holds no address of its own.
  #[serde(default)]
  pub card_service: Option<String>,
  /// Whether the card service's instances keep role files.
  #[serde(default)]
  pub role_files: bool,
  /// Every supervisor, role and repository whose order or pin names it.
  #[serde(default)]
  pub used_by: Vec<String>,
  pub healthy: Option<bool>,
  /// When the engine behind it last answered a health probe, in unix seconds, where it ever has.
  pub last_answered_at: Option<i64>,
  /// Its window; null for a model that is no registration.
  pub context_tokens: Option<i64>,
  /// Its answer ceiling; null for a model that is no registration.
  pub answer_tokens: Option<i64>,
  /// Its thinking default: what a run on it does where its order's entry leaves thinking to the provider;
  /// null for a model that is no registration.
  pub thinking: Option<bool>,
}

/// ModelCatalogueEntry:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogueEntry {
  pub name: String,
  pub label: String,
  pub file: String,
  pub bytes: i64,
  pub sha256: String,
  pub source: Option<String>,
  pub engine: Option<String>,
  pub registration: Option<String>,
  pub nodes: Vec<String>,
  pub thinking: Option<bool>,
  pub temperature: Option<f64>,
  pub top_p: Option<f64>,
  pub presence_penalty: Option<f64>,
  pub max_tokens: Option<i64>,
  pub note: Option<String>,
  pub serving: bool,
}

/// ModelCatalogueUse:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogueUse {
  pub id: i64,
  pub entry: String,
  pub engine: String,
  pub chosen_by: String,
  pub started_at: i64,
  pub ended_at: Option<i64>,
  pub state: String,
  pub step: Option<String>,
  pub reason: Option<String>,
  pub log_tail: Vec<String>,
  pub log: Option<String>,
}

/// ModelCatalogueView:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogueView {
  pub entries: Vec<ModelCatalogueEntry>,
  pub uses: Vec<ModelCatalogueUse>,
}

/// ModelCatalogueChange: the action and the fields a change of the catalogue names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogueChange {
  pub action: String,
  pub name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub file: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bytes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sha256: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub engine: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub registration: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub note: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub temperature: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub top_p: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub presence_penalty: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_tokens: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
}

/// PinCheckAsk:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinCheckAsk {
  pub value: String,
}

/// PinCheck:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinCheck {
  pub kind: PinCheckKind,
  pub provider: Option<String>,
  pub model: Option<String>,
}

/// Tier:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Tier {
  pub name: String,
  pub coworker: bool,
  pub mapping: Value,
}

/// Fallback:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
  pub routes: Vec<Value>,
  pub answering: bool,
  /// The agent serving the chat now and what it has used.
  #[serde(default)]
  pub active: Option<FallbackActive>,
  /// Every Claude account as the road walks them.
  #[serde(default)]
  pub accounts: Vec<FallbackAccount>,
}

/// The agent serving the chat now: the provider its road answers on, the login it walks to where the
/// provider holds logins, and that slot's two headline windows and balance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FallbackActive {
  pub provider: String,
  pub account: Option<String>,
  /// The login's name, or the provider's label where it holds none.
  pub account_name: String,
  /// How many logins are on, where the provider holds logins; 0 otherwise.
  pub accounts: i64,
  pub session: Option<UsageWindow>,
  pub week: Option<UsageWindow>,
  /// What the slot has left, where its provider or its endpoint account offers a balance.
  pub balance: Option<Balance>,
}

/// One Claude account as the fallback's view shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAccount {
  pub id: String,
  pub name: String,
  pub place: i64,
  pub enabled: bool,
  /// One of `directory`, `token` or `endpoint`.
  pub credential_kind: String,
  pub credential_path: String,
  pub endpoint_url: Option<String>,
  /// An endpoint account's key file. Its content is never reported.
  pub key_file: Option<String>,
  pub models: Option<EndpointModels>,
  pub base_prompt: Option<String>,
  /// One of `standing`, `ending` or `gone`.
  pub login: String,
  /// epoch seconds
  pub login_ends_at: Option<i64>,
  pub chosen: bool,
  pub serving: bool,
  /// epoch seconds
  pub rest_until: Option<i64>,
  pub session: Option<UsageWindow>,
  pub week: Option<UsageWindow>,
  pub balance: Option<Balance>,
}

/// What a paid account has left, as its balance reader last read it (docs/usage-windows.md, 「Balances」).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
  pub amount: f64,
  pub currency: String,
  /// epoch seconds
  pub seen_at: i64,
}

/// ItemFilingAsk: an item a supervisor files for the turn it carries now; the agent is the one its harness
/// plugin was started for, and the item's room and whether the owner asked it are agency's to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemFilingAsk {
  pub agent_name: String,
  pub title: String,
  pub brief: String,
  pub queue_name: String,
  /// The repository the work is in, written as the item's `assigneeName`; none for host work.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub priority: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub assignee_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub requester_name: Option<String>,
  #[serde(default)]
  pub labels: std::collections::BTreeMap<String, String>,
}

/// FileItem: an item to file as the supervisor asks it, everything but the agent its plugin was started for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
  /// The item's name in the queue: a short line in the asker's language.
  pub title: String,
  /// The run's whole instruction, in English.
  pub brief: String,
  /// The queue it is filed in.
  pub queue_name: String,
  /// The repository the work is in, by its internal name, so only that repository's worker takes it;
  /// left out for host work, which is in no repository. Work across several repositories is one item per
  /// repository, each with the same brief and the same `series=<name>` label.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub priority: Option<i64>,
  /// The worker it is to go to, where it names one; the repository's name where `repository` names one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub assignee_name: Option<String>,
  /// Who asked for it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub requester_name: Option<String>,
  /// Labels of the filer's own, each a string.
  #[serde(default)]
  pub labels: std::collections::BTreeMap<String, String>,
}

/// ItemFiled: the item as the job service raised it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemFiled {
  pub id: i64,
  /// `open` where the job service's unattended labels opened it, `draft` otherwise.
  pub state: String,
  pub scope: Option<String>,
  /// The item's labels, each a string.
  pub labels: Value,
}

/// JobStopAsk: a job a supervisor stops for the turn it carries now, by its id or by its item; the agent is the
/// one its harness plugin was started for, and whether the owner asked for the stop is agency's to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobStopAsk {
  pub agent_name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item_id: Option<i64>,
  pub reason: String,
}

/// StopJob: a job to stop as the supervisor asks it, everything but the agent its plugin was started for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StopJob {
  /// The job, where the stop names it by its id.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job_id: Option<i64>,
  /// The item whose starting or running job is stopped, where the stop names it by its item.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item_id: Option<i64>,
  /// Why, in a short line: the job's report.
  pub reason: String,
}

/// WorkKeyAssignment: one work key's tier, the tiers it may be assigned, the owner's choices from before the
/// tiers kept for review, and what it resolves to on each provider of the registry's archive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkKeyAssignment {
  pub key: String,
  /// The key in kebab case, the word a screen words it by.
  pub code: String,
  /// The English name its profile gives, none for a key no profile declares.
  pub label: Option<String>,
  pub tier: String,
  pub choices: Vec<String>,
  /// Each `provider`, `previous` and `accepted`.
  pub conflicts: Vec<Value>,
  /// Each `provider`, `selection`, `error` (one short clause), `detail` (the whole of why) and `errorCode`.
  pub effective: Vec<Value>,
}

/// WorkKeyAssignmentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkKeyAssignmentPage {
  pub items: Vec<WorkKeyAssignment>,
  pub more: bool,
}

/// WorkKey:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkKey {
  pub key: String,
  pub provider: String,
  pub model: String,
  pub thinking: Option<bool>,
  pub effort: Option<String>,
  pub answer_tokens: Option<i64>,
  pub cap_ms: i64,
  pub run_mode: String,
}

/// RoadBlock:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadBlock {
  pub id: i64,
  pub selection: String,
  /// epoch milliseconds
  pub until: i64,
  pub reason: String,
}

/// LaunchRoom:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRoom {
  pub held: bool,
  pub reasons: Vec<Value>,
  pub job_count: i64,
  pub browser_job_count: i64,
}

/// An endpoint's model ids by the family word Claude Code picks a model by:
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndpointModels {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub opus: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sonnet: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub haiku: Option<String>,
}

/// One usage window of a slot as the usage-window store holds it (docs/usage-windows.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
  pub name: String,
  /// The login the slot is kept under, none on a provider that holds no logins.
  pub account: Option<String>,
  /// The provider the window was read of.
  pub mode: String,
  /// The share of the limit used, 0 to 1.
  pub utilization: f64,
  /// epoch seconds
  pub resets_at: Option<i64>,
  /// epoch seconds: when the reading it came in was seen.
  pub seen_at: i64,
  pub duration_minutes: Option<i64>,
  pub limit_name: Option<String>,
}

/// Account:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Account {
  pub id: String,
  pub provider: String,
  pub user_name: Option<String>,
  pub held: bool,
  pub home: Option<String>,
  /// The usage a report named, on the report's answer; null on the list.
  pub window: Value,
  /// The account's session window, none where it holds none: an endpoint account never does.
  #[serde(default)]
  pub session: Option<UsageWindow>,
  /// The account's week window, none where it holds none.
  #[serde(default)]
  pub week: Option<UsageWindow>,
  /// What an endpoint account has left, where its endpoint's provider offers a balance.
  #[serde(default)]
  pub balance: Option<Balance>,
  /// epoch milliseconds
  pub rest_until: Option<i64>,
  /// One of `directory`, `token` or `endpoint`.
  pub credential_kind: Option<String>,
  /// An endpoint account's base URL.
  pub endpoint_url: Option<String>,
  /// An endpoint account's key file. Its content is never reported.
  pub key_file: Option<String>,
  /// An endpoint account's model ids by family word.
  pub models: Option<EndpointModels>,
  /// The base prompt a worker's run on it opens with.
  pub base_prompt: Option<String>,
}

/// Rest:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rest {
  pub resting: bool,
  /// epoch milliseconds
  pub until: Option<i64>,
  pub kind: Option<String>,
  pub account_id: Option<String>,
}

/// StandIn: whether a stand-in answers the agent named now, and on which provider and model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StandIn {
  pub answers: bool,
  /// The first provider of its order other than Claude Code that can answer now, or null.
  #[serde(default)]
  pub provider: Option<String>,
  /// The model its order names on that provider, or null.
  #[serde(default)]
  pub model: Option<String>,
}

/// Usage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
  pub groups: Vec<Value>,
  /// epoch milliseconds
  pub after: i64,
  /// epoch milliseconds
  pub before: i64,
}

/// Settings:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
  /// The supervisor a run of the search or browse line no supervisor delegated writes for, or null.
  pub research_supervisor: Option<String>,
  pub downgrade_return: Value,
}

/// SupervisorSettings: a supervisor as its settings page shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorSettings {
  pub agent: String,
  pub label: String,
  pub icon: Option<String>,
  /// Whether the deployment seeds it.
  pub builtin: bool,
  pub turn_shape: String,
  /// What its order names first.
  pub provider: Option<String>,
  pub model: Option<String>,
  /// The registration its order names at the registry's provider, or null.
  pub registration: Option<String>,
  /// Every offered registration, each `name` and `label`.
  pub registrations: Vec<Value>,
  /// The model files behind its registration.
  pub entries: Vec<Value>,
  pub serving: Option<String>,
  pub model_use: Option<Value>,
  /// `referenceOn`, `persona`, `generation`, `updatedAt` and `updatedBy`.
  pub settings: Value,
  /// Every generation setting with its bounds.
  pub generation_settings: Vec<Value>,
  pub thinking: Option<bool>,
  pub persona_file: String,
  pub base_prompt: Option<String>,
  pub base_prompts: Vec<Value>,
  /// The system prompt its next turn sends, `characters`, `hash` and `text`, where its turn shape is `pieces`.
  pub system: Option<Value>,
  /// Its room sessions.
  pub sessions: Vec<Value>,
}

/// SupervisorReset: what a reset of a supervisor took, or would take on a dry run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorReset {
  pub agent: String,
  pub label: String,
  pub ranges: Vec<String>,
  /// The rooms it answers alone, each `chatId` and `title`.
  pub alone: Vec<Value>,
  /// The rooms it shares with other supervisors, each `chatId` and `title`.
  pub shared: Vec<Value>,
  /// What it took of each table and range, each `table`, `range` and `rows`.
  pub rows: Vec<Value>,
  /// The harness records it took.
  pub files: Vec<String>,
  /// The brain's answer for the person range, or null.
  pub brain: Value,
}

/// Worker:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Worker {
  pub name: String,
  pub label: String,
  pub cwd: String,
  pub icon: Option<String>,
  pub builtin: bool,
  pub node_name: Option<String>,
  pub role: Option<String>,
  /// resting, working while a job of its is starting or running, or idle
  pub state: String,
  /// The job it is working on, where it is working.
  #[serde(default)]
  pub job_id: Option<i64>,
  /// That job's item.
  #[serde(default)]
  pub item_id: Option<i64>,
  pub sampling: Value,
  /// epoch milliseconds
  pub resting_since: Option<i64>,
  pub session_id: Option<String>,
  pub run_count: i64,
  /// epoch milliseconds
  pub last_run_at: Option<i64>,
  /// The documents its session's prefix carries, in order: a relative path from its checkout, an absolute
  /// one where it stands.
  #[serde(default)]
  pub prefix_documents: Vec<String>,
}

/// WorkerDefinition:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerDefinition {
  pub graph: String,
  pub trailer: String,
  pub handoff: Option<String>,
}

/// Workbench:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Workbench {
  pub root: String,
  pub scratches: Vec<Value>,
}

/// PrefixCheckAsk:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrefixCheckAsk {
  pub worker_name: String,
  pub files: Vec<Value>,
}

/// PrefixCheck:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrefixCheck {
  pub touched: Vec<Value>,
}

/// Hook:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hook {
  pub point: String,
  pub url: String,
  pub token: Option<String>,
}

/// PublishedEvent:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedEvent {
  pub kind: String,
  pub data: Value,
}

/// RoomHolding:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomHolding {
  pub room_id: i64,
  pub holdings: Vec<Value>,
}

/// HookPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookPage {
  pub items: Vec<Hook>,
  pub more: bool,
}

/// AgentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentPage {
  pub items: Vec<Agent>,
  pub more: bool,
}

/// AgentRoleBundle: a bundle a role may give, by its name and the label a screen writes it by.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoleBundle {
  pub name: String,
  pub label: String,
}

/// RoleSessions: whether a role's runs go on in the session an earlier run left while its cache is warm,
/// `keep`, or each open a session of its own, `fresh`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RoleSessions {
  #[default]
  Keep,
  Fresh,
}

/// AgentRole: one role an agent may hold, as the store keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRole {
  pub name: String,
  pub label: String,
  pub bundles: Vec<AgentRoleBundle>,
  /// The most of its agents that run a turn at once; null where it is unbounded.
  pub max_concurrent: Option<i64>,
  /// The prompt its agents are given before their own; null where it carries none.
  pub prompt: Option<String>,
  /// The tier its work key `role:<name>` is assigned; null where none is.
  pub tier: Option<String>,
  /// Whether its runs keep their sessions.
  pub sessions: RoleSessions,
  /// How many agent rows hold it.
  pub agents: i64,
  /// Whether a kind's own is it, which keeps it from being removed.
  pub kind_default: bool,
}

/// AgentRoleList: every role an agent may hold, and every bundle a role may give.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoleList {
  pub roles: Vec<AgentRole>,
  pub bundles: Vec<AgentRoleBundle>,
}

/// AgentRoleAsk: the body that adds a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoleAsk {
  /// kebab-case, held by no role and no co-worker role
  pub name: String,
  pub label: String,
  /// bundle names the policy declares
  pub bundles: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_concurrent: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  /// `keep` where none is named
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sessions: Option<RoleSessions>,
}

// A field given, even as null, read as given; a field left out stays none.
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
  given: D,
) -> Result<Option<Option<T>>, D::Error> {
  Option::<T>::deserialize(given).map(Some)
}

/// AgentRoleChange: a change to a role; a field left out keeps what it holds, and a bound, a prompt or a
/// tier given as null is cleared.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoleChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bundles: Option<Vec<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub max_concurrent: Option<Option<i64>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub prompt: Option<Option<String>>,
  #[serde(default, deserialize_with = "present", skip_serializing_if = "Option::is_none")]
  pub tier: Option<Option<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sessions: Option<RoleSessions>,
}

/// PoolLimit: how many children of one co-worker tier may be alive on one provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PoolLimit {
  pub provider: String,
  pub tier: String,
  /// from 0, which closes the tier on the provider
  pub capacity: i64,
}

/// PromptDocumentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptDocumentPage {
  pub items: Vec<PromptDocument>,
  pub more: bool,
}

/// ChatDocument: one of the supervisor's own documents, as it stands now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatDocument {
  pub name: String,
  pub kind: String,
  /// `stored` where the owner's text stands, `file` where the deployed file does.
  pub source: String,
  /// When the stored text was last saved, in seconds; null on a document standing as its file.
  pub version: Option<i64>,
  pub text: String,
}

/// ChatDocumentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatDocumentPage {
  pub items: Vec<ChatDocument>,
  pub more: bool,
}

/// ChatDocumentChange:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatDocumentChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub action: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub text: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
}

/// Rulebook:
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rulebook {
  pub name: String,
  pub text: String,
}

/// Rulebooks:
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rulebooks {
  pub rulebooks: Vec<Rulebook>,
}

/// SessionPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPage {
  pub items: Vec<Session>,
  pub more: bool,
}

/// RunPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunPage {
  pub items: Vec<Run>,
  pub more: bool,
}

/// LogLinePage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogLinePage {
  pub items: Vec<LogLine>,
  pub more: bool,
}

/// One agent's log across its runs: its lines, whether another page waits towards the way it was read,
/// and, for a page opened at a moment, whether older lines stand before it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentLogPage {
  pub items: Vec<LogLine>,
  pub has_more: bool,
  #[serde(default)]
  pub has_older: bool,
}

/// ExecutionPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPage {
  pub items: Vec<Execution>,
  pub more: bool,
}

/// OvertimeRequestPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OvertimeRequestPage {
  pub items: Vec<OvertimeRequest>,
  pub more: bool,
}

/// SeatRequestPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequestPage {
  pub items: Vec<SeatRequest>,
  pub more: bool,
}

/// ReservationPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationPage {
  pub items: Vec<Reservation>,
  pub more: bool,
}

/// PlacePage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlacePage {
  pub items: Vec<Place>,
  pub more: bool,
}

/// ProviderPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPage {
  pub items: Vec<Provider>,
  pub more: bool,
}

/// ModelPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelPage {
  pub items: Vec<Model>,
  pub more: bool,
}

/// TierPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TierPage {
  pub items: Vec<Tier>,
  pub more: bool,
}

/// Where a fold ratio comes from: the table's row, the provider's own declared share where the table holds none, or the general share where the provider declares none either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FoldRatioSource {
  #[serde(rename = "set")]
  Set,
  #[serde(rename = "provider")]
  Provider,
  #[serde(rename = "default")]
  Default,
}

/// A provider's fold ratio at a tier: the share of a model's window a session may fill before it is folded once the turn that crossed it is over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FoldRatio {
  pub provider: String,
  pub tier: String,
  pub ratio: f64,
  pub source: FoldRatioSource,
}

/// FoldRatioPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FoldRatioPage {
  pub items: Vec<FoldRatio>,
  pub more: bool,
}

/// The body that sets a provider's fold ratio at a tier: a share above 0 and at most 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FoldRatioWrite {
  pub ratio: f64,
}

/// RoadBlockPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadBlockPage {
  pub items: Vec<RoadBlock>,
  pub more: bool,
}

/// AccountPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountPage {
  pub items: Vec<Account>,
  pub more: bool,
}

/// WorkerPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerPage {
  pub items: Vec<Worker>,
  pub more: bool,
}

/// Assignment: A worker held for one item for the harness that asked. held until POST /jobs uses it; running while its job runs; released when the harness lets it go or the job ends. No clock ends a hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
  pub id: i64,
  pub item_id: i64,
  pub holder_name: String,
  pub worker_name: String,
  /// The repository the checkout `workerName` names is of, none for a checkout no repository holds any longer.
  #[serde(default)]
  pub repository_name: Option<String>,
  /// The role the item's job runs as.
  #[serde(default)]
  pub role_name: Option<String>,
  pub state: AssignmentState,
  /// epoch milliseconds
  pub created_at: i64,
  /// epoch milliseconds
  pub changed_at: i64,
}

/// AssignmentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentPage {
  pub items: Vec<Assignment>,
  pub more: bool,
}

/// JobItem: The item's content as the harness read it from the job service; the agency reads nothing there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobItem {
  pub title: String,
  pub brief: String,
  pub work_order: Option<Value>,
  pub notes: Vec<Value>,
  /// the caller's own metadata
  pub labels: BTreeMap<String, String>,
  /// The room the item was filed from, where it names one: the room a job's end is told in.
  #[serde(default)]
  pub scope: Option<String>,
  /// The item's small-model instruction, where it carries one.
  #[serde(default)]
  pub instruction: Option<JobInstruction>,
}

/// JobInstruction: the whole of a small-model instruction: the checkout the change is made in, the
/// canonical documents it answers to, the sentences it must make or keep true and the commands whose
/// passing says it is done; every one present and none empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobInstruction {
  pub repository: String,
  pub documents: Vec<String>,
  pub sentences: Vec<String>,
  pub checks: Vec<String>,
}

/// InstructionText: the one line a work-order step is written as, composed from an instruction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionText {
  pub text: String,
}

/// Job: itemId and leaseId are the harness's references, kept as given: the harness renews the lease, and reports and closes the item in the job service from report and outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
  pub id: i64,
  pub item_id: i64,
  pub lease_id: i64,
  pub holder_name: String,
  pub assignment_id: i64,
  pub worker_name: String,
  /// The repository the checkout `workerName` names is of, none for a checkout no repository holds any longer.
  #[serde(default)]
  pub repository_name: Option<String>,
  /// The role the job runs as.
  #[serde(default)]
  pub role_name: Option<String>,
  pub session_id: Option<String>,
  pub run_id: Option<i64>,
  pub run_name: Option<String>,
  pub state: JobState,
  pub outcome: Option<String>,
  pub report: Option<String>,
  /// What the job's turn spent, the four figures of `usage` together; 0 until it ends or where its road
  /// reported no figures.
  pub token_count: i64,
  /// What the job's turn spent, apart; none until it ends or where its road reported no figures.
  #[serde(default)]
  pub usage: Option<RunUsage>,
  /// The small-model instruction the job carries, where it carries one.
  #[serde(default)]
  pub instruction: Option<JobInstruction>,
  /// Every road the job's turns went out on, in order, each with why the road before it was left.
  #[serde(default)]
  pub roads: Vec<JobRoad>,
  /// Why the job waits, where no provider of its order can answer now; none whenever a turn is out.
  #[serde(default)]
  pub held_for: Option<String>,
  /// What chose its provider and model; none for a job launched before it was kept.
  #[serde(default)]
  pub chosen_by: Option<JobChoice>,
  /// epoch milliseconds
  pub started_at: i64,
  /// epoch milliseconds
  pub ended_at: Option<i64>,
}

/// JobRoad: one road a job's turn went out on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobRoad {
  pub provider: String,
  pub model: String,
  /// The login it went out on, where its provider holds logins.
  pub account: Option<String>,
  /// epoch milliseconds
  pub at: i64,
  /// Why the road before it was left; none on the first.
  pub reason: Option<String>,
}

/// JobPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobPage {
  pub items: Vec<Job>,
  pub more: bool,
}

/// JobPrompt:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobPrompt {
  pub job_id: i64,
  pub text: String,
  pub english: Option<String>,
}

/// SessionWork:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionWork {
  pub session_id: String,
  pub item_id: Option<i64>,
  pub results: Vec<Value>,
}

/// SessionLoads:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoads {
  pub session_id: String,
  pub documents: Vec<Value>,
}

/// The body that asks for a worker to be assigned to an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignWorker {
  pub item_id: i64,
  pub holder_name: String,
  pub item: JobItem,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub assignee_name: Option<String>,
  /// The role the item's job runs as, a role the store holds; `worker` where it names none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role_name: Option<String>,
}

/// The body that releases an assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<AssignmentState>,
}

/// The body that hands an item and its lease over to be run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunJob {
  pub assignment_id: i64,
  pub item_id: i64,
  pub lease_id: i64,
  pub holder_name: String,
  pub item: JobItem,
  /// The job's small-model instruction, over any its item carries.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub instruction: Option<JobInstruction>,
}

/// The body that ends or stops a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<JobState>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outcome: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub report: Option<String>,
}

/// The body that records the item a session serves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionWorkChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item_id: Option<i64>,
}

/// The body that registers a hook's address at a point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookRegistration {
  pub url: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub token: Option<String>,
}

/// The body that changes an agent's gates, reasoning, role, voice or own model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gates: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reasoning: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub voice: Option<Value>,
  /// the agent's own model
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<AgentModel>,
}

/// An agent's own model: `fixed` on a provider and one model of it, or on a registration by its name
/// alone; `route` on the supervisor key's road, a supervisor's alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentModel {
  pub method: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub registration: Option<String>,
}

/// The body that registers an agent: its key, label, kind, role and owner, and the fields its kind
/// carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentAsk {
  pub name: String,
  pub label: String,
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  /// the id of the login account that owns it, none for the administrator's
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner_account: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub icon: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  /// the `chat` kind's pinned provider
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  /// the `chat` kind's pinned model
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  /// the worker a `chat` agent's items go to
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub worker: Option<String>,
  /// a `worker`'s checkout
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  /// the node a `worker` stands on
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node_name: Option<String>,
}

/// What became of a registered agent's brain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BrainBirth {
  Made,
  Kept,
  None,
  Failed,
}

/// An agent registered, and what became of its brain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentCreated {
  pub agent: Agent,
  pub brain: BrainBirth,
}

/// The body that creates, saves, removes or chooses a prompt document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptDocumentChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub action: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub text: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
}

/// The body that words which session documents moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMoveAsk {
  pub paths: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room_ids: Option<Vec<i64>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub rooms: Option<Vec<i64>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub now: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
}

/// What a session's start options name: the provider, the model and the folder a throwaway or a
/// conversation's turns go out on, and the item and role a co-work branch's place labels its run with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionOptions {
  pub provider: Option<String>,
  pub model: Option<String>,
  pub cwd: Option<String>,
  pub todo: Option<i64>,
  pub role: Option<String>,
}

/// The body that starts a session: the agent, the scope, the room and the options, with the older
/// top-level names read where the options name none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionStart {
  /// The token of the slot request a role's run was let in on: the run takes its role, provider, model, effort
  /// and, where the start names no folder, its path.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub slot: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
  pub scope: Value,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
  /// The runner line a thrown-away session goes out on with the instructions its caller hands in whole.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line: Option<String>,
  /// The line's instructions, handed in whole after the platform's harness opening; needs `line`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub system_prompt: Option<String>,
  /// What the session's record carries under each key of its labels, for the placed tools that read them.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub labels: Option<serde_json::Map<String, Value>>,
  /// The capabilities the session's record keeps, of those its line and its agent's role give.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tools_allowed: Option<Vec<String>>,
  /// On a role's session, the supervisor whose double the run is: every turn holds that supervisor's tools
  /// beside its role's.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub supervisor: Option<String>,
  /// A granted demand of the caller's own on the arbiter's master, which each turn of a thrown-away session
  /// on a registration naming a card service stands on, with no other asked.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub demand_id: Option<i64>,
  /// The caller's granted demand each turn's own demand is asked within.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub within: Option<i64>,
  /// The rank a demand this work asks stands at, a whole number of zero or more.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_rank: Option<u32>,
  /// The pool a demand this work asks stands in.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_pool: Option<CardPool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub options: Option<SessionOptions>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub todo: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
}

/// What a session's change names besides its model and holder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionChangeOptions {
  pub provider: Option<String>,
  pub model: Option<String>,
  pub cwd: Option<String>,
  pub session: Option<String>,
}

/// The body that moves a conversation's or a throwaway session's provider and model, or hands a worker's
/// session over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub options: Option<SessionChangeOptions>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix_change_reason: Option<String>,
}

/// A room's block as a turn's body names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomBlockAsk {
  pub chat_id: i64,
  pub text: Option<String>,
  pub mark: Option<String>,
}

/// What a role's session's turn names of its own; its road, effort and folder are the session's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPartsAsk {
  pub resume: Option<Value>,
  pub name: Option<String>,
  pub title: Option<String>,
  pub chat: Option<i64>,
  pub unit: Option<String>,
  pub user: Option<String>,
  pub hears_words: Option<bool>,
  pub variables: Option<Value>,
}

/// The body that sends a turn: its prompt, the context around it, and what a role's session's turn names of its
/// own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnSend {
  pub prompt: String,
  /// The context as the answer echoes it; the parts below are what the turn reads.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub context: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<RoomBlockAsk>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub base_persona: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub privileged: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answers: Option<Vec<i64>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
  /// The round of the room the turn belongs to, which the turn's run keeps.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub round: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub resume: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub unit: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub hears_words: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub variables: Option<Value>,
}

/// The body that interrupts a turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterrupt {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
}

/// The body that pushes words into a working turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnWords {
  pub words: String,
}

/// The body that answers a question a turn stands on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnswerAsk {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub question_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub question: Option<String>,
  pub answer: Value,
}

/// The body a fold takes, which reads nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FoldAsk {}

/// The body that opens a run in the ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunOpen {
  pub agent: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat_id: Option<i64>,
  pub unit: String,
  pub depth: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cap_ms: Option<i64>,
  pub started_at: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub started_by: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub search_id: Option<Value>,
}

/// The body that closes a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunClose {
  /// The message the run's answer went out as; named alone, it is attached and the closing left as it
  /// stands.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub finished_at: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outcome: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub context_tokens: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  /// The older name for `finishedAt`, read where that is absent.
  #[serde(default, rename = "finished_at", skip_serializing_if = "Option::is_none")]
  pub finished_at_legacy: Option<i64>,
  /// The older name for `contextTokens`, read where that is absent.
  #[serde(default, rename = "context_tokens", skip_serializing_if = "Option::is_none")]
  pub context_tokens_legacy: Option<i64>,
}

/// The body that writes a note into a run's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogLineAsk {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub run: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub run_id: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub job_id: Option<i64>,
  pub text: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub at: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
}

/// A run's runner as a launch names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProgramAsk {
  pub program: String,
  pub args: Vec<String>,
  pub cwd: String,
  pub environment: std::collections::BTreeMap<String, String>,
}

/// The body that launches a run's runner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionLaunch {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub run_id: Option<i64>,
  pub title: String,
  pub agent: String,
  pub command: ProgramAsk,
}

/// The body that cancels an execution or takes the cancel back, which reads nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionChange {}

/// The body that answers an overtime request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OvertimeAnswer {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ask: Option<i64>,
  pub verdict: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub minutes: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub why: Option<String>,
}

/// The body that marks a job to stop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StopMarkAsk {
  pub reason: String,
}

/// What a seat request or a reservation asks of the pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PoolWanted {
  pub role: String,
  pub selection: String,
  pub tier: Option<String>,
  pub provider: Option<String>,
  pub work_key: Option<String>,
}

/// The body that asks for a co-worker seat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequestAsk {
  pub request_key: String,
  pub owner: String,
  pub role: String,
  pub selection: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub work_key: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub generation: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub yielding: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub token: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub place_key: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  /// One line naming the work the seat is asked for.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  /// The agent asking for the seat.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub asked_by: Option<String>,
}

/// The body that moves a seat request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequestChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub action: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub child_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outcome: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cancel: Option<bool>,
  /// On an admission ask: a granted demand of the caller's own the seat stands on, with none of its own
  /// asked.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub demand_id: Option<i64>,
  /// On an admission ask: the asking run's granted demand the seat's own is asked within.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub within: Option<i64>,
  /// The rank a demand this work asks stands at, a whole number of zero or more.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_rank: Option<u32>,
  /// The pool a demand this work asks stands in.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_pool: Option<CardPool>,
  /// On a binding: the agent the seat's child runs on.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
}

/// The body that reserves seats for an owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationAsk {
  pub reservation_key: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  pub selection: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub work_key: Option<String>,
  pub count: i64,
}

/// The body that releases a reservation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outcome: Option<String>,
}

/// The body that takes a branch place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAsk {
  pub cwd: String,
  pub selection: String,
  pub role: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub held: Option<Vec<Value>>,
}

/// The body that prepares, remembers or retires a place, or records its last write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaceChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub slot: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub upstream: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub used: Option<Vec<i64>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub held: Option<Vec<Value>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub last_write: Option<Value>,
}

/// The body that throws a provider's switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub enabled: Option<bool>,
}

/// The body that saves or removes a registration, or sets or removes one of its addresses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub action: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub context_tokens: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answer_tokens: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub walked: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub keeps_research: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub url: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
  /// Whether the instance behind the address keeps role files, or on a registration naming a card service
  /// whether its instances do; left as it stands where none is named.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role_files: Option<bool>,
  /// The arbiter service the registration's engine is, or `none`; naming one takes its addresses away.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_service: Option<String>,
}

/// The body that assigns a tier to a work key or sets a tier's mapping on a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TierChange {
  pub action: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub accept_conflicts: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answer_tokens: Option<i64>,
}

/// The body that changes the fallback screen's routes, providers and accounts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FallbackChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub action: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub enabled: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
  /// An endpoint account's base URL.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub endpoint: Option<String>,
  /// An endpoint account's key file.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key_file: Option<String>,
  /// An endpoint account's model ids by family word.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub models: Option<EndpointModels>,
  /// The base prompt the account names, `none` or null to let it go.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub base_prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  /// The registration a work key runs on at the registry's provider in place of its tier's cell, `auto`
  /// or null to read the cell again.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub registration: Option<String>,
}

/// The body that blocks a selection's road.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadBlockAsk {
  pub selection: String,
  pub until: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

/// The body that tells the launcher what holds launches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LaunchHeldAsk {
  pub held: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reasons: Option<Vec<String>>,
}

/// The body that reports a login's session usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub usage: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub window: Option<Value>,
}

/// The body that writes a rest down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestAsk {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub clear_passed: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub until: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<String>,
}

/// The body that throws the warming switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsAsk {
  /// A supervisor the store holds, or null for none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub research_supervisor: Option<Value>,
}

/// The body that changes a supervisor's settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorSettingsChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reference_on: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub persona: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub registration: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub generation: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
}

/// The body that asks for a supervisor reset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorResetAsk {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub conversation: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub person: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub alone: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub shared: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dry_run: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
}

/// The body that adds a worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerAdd {
  pub name: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub icon: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node_name: Option<String>,
  /// The documents its session's prefix carries, in order.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix_documents: Option<Vec<String>>,
}

/// The body that changes a worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sampling: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub icon: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub stand: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bench: Option<bool>,
  /// The documents its session's prefix carries, in order, the list whole; an empty list clears it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix_documents: Option<Vec<String>>,
}

/// The body that keeps a session's loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoadsAsk {
  pub documents: Vec<Value>,
}

/// GithubDeliveryTaken: a GitHub delivery taken, to be acted on after the answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GithubDeliveryTaken {
  pub taken: bool,
}

/// TranslationRunAsk: one text to translate on the `translate` road: what it is for (`document`, `worker` or
/// `record`), the language to render it in and the text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRunAsk {
  pub purpose: String,
  pub language: String,
  pub text: String,
  /// The caller's word on this one part, carried into the run's input as a note not to be translated.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub note: Option<String>,
  /// The words of this one part the translation renders as the caller says, set before the text as the
  /// translation model's reference translations.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub terms: Vec<TranslationTerm>,
}

/// TranslationTerm: a word of a part to translate and how the translation writes it: `source` as the text
/// writes it, and `target`, the source itself where none is named.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranslationTerm {
  pub source: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub target: Option<String>,
}

/// TranslationRun: a translation as it came back, whole, with the road it went out on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranslationRun {
  pub text: String,
  pub provider: String,
  pub model: String,
}

/// One gate kind as the gate declares it: its key, the part of a turn it stands in, its step there and
/// the condition it runs under where it runs in some turns only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GateKind {
  pub key: String,
  pub part: String,
  pub step: i64,
  pub condition: Option<String>,
}

/// A page of the gate kinds the gate declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GateKindPage {
  pub items: Vec<GateKind>,
  pub more: bool,
}

/// One gate of the gate's declaration, as the gate publishes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredGate {
  pub gate_name: String,
  /// `pre`, `post` or `apart`.
  pub part: String,
  pub step: i64,
  pub condition: Option<String>,
  pub switchable: bool,
  pub laya_probe: bool,
  pub default_laya_mode: Option<String>,
  pub answer_tokens: Option<i64>,
}

/// The gate's declaration of its kinds, written over the one standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GateKindDeclaration {
  pub gates: Vec<DeclaredGate>,
}

/// OrderEntry: one provider of a supervisor's or a role's order, and what the row runs on there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderEntry {
  pub provider: String,
  pub model: String,
  /// a level of the effort scale; null for the provider's own
  #[serde(default)]
  pub effort: Option<String>,
  /// `on`, `off`, or null for the provider's own
  #[serde(default)]
  pub thinking: Option<String>,
  /// on the registry's provider, the answer ceiling; null for the registration's own
  #[serde(default)]
  pub answer_tokens: Option<i64>,
}

/// A pool the demands of a work stand in: its key, and the most cards they use of one service at once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CardPool {
  pub key: String,
  pub cards: u32,
}

/// Supervisor: one supervisor as its row holds it, and what it stands on now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Supervisor {
  pub name: String,
  pub label: Option<String>,
  pub owner_account: Option<i64>,
  pub aliases: Option<String>,
  pub icon: Option<String>,
  pub session_scope: String,
  pub turn_shape: String,
  pub brain_folder: Option<String>,
  /// the repository its TODO items go to
  pub repository: Option<String>,
  pub order: Vec<OrderEntry>,
  /// a fraction from 0 to 1
  pub quota_share: f64,
  /// the provider the owner forced as its active provider, or null where its order chooses
  #[serde(default)]
  pub active: Option<ActiveProvider>,
  /// for each provider of its order, whether its prefix is kept warm there
  pub warming: BTreeMap<String, bool>,
  /// its compaction threshold
  pub fold_ratio: f64,
  /// the rank of the demands of the runs for it, where it holds one
  #[serde(default)]
  pub card_rank: Option<u32>,
  pub gates: Value,
  pub voice: Option<String>,
  pub sampling: Option<Value>,
  /// what its last turn went out on
  pub provider: Option<String>,
  pub model: Option<String>,
  pub effort: Option<String>,
  /// why its turn waits where no provider of its order has quota, or null
  pub waiting_for: Option<String>,
  /// Whether the deployment seeds it: its removal is refused 409 `seeded`.
  #[serde(default)]
  pub seeded: bool,
  /// The moment in epoch milliseconds its last reset cut its turns off at, none where it was never reset.
  #[serde(default)]
  pub reset_point: Option<i64>,
}

/// SupervisorRoom: one room a supervisor's turns went into, with its latest run there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRoom {
  pub room_id: i64,
  /// How many of its runs the ledger holds there.
  pub runs: i64,
  pub last_run_id: i64,
  pub last_run_unit: String,
  /// epoch milliseconds
  pub last_started_at: i64,
  /// epoch milliseconds
  pub last_finished_at: Option<i64>,
}

/// SupervisorRoomList: the rooms a supervisor's turns went into, the latest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRoomList {
  pub items: Vec<SupervisorRoom>,
  pub more: bool,
}

/// SupervisorResetPoint: a supervisor's reset point alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorResetPoint {
  pub supervisor: String,
  /// epoch milliseconds, or null where it was never reset
  pub reset_point: Option<i64>,
}

/// SupervisorList: every supervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorList {
  pub items: Vec<Supervisor>,
  pub more: bool,
}

/// SupervisorAsk: the body that registers a supervisor: its order whole with its share, or one provider and model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorAsk {
  pub name: String,
  pub label: String,
  /// Its provider order, written whole; where it is named with `provider` and `model`, its first entry is
  /// theirs.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<Vec<OrderEntry>>,
  /// One provider and model, an order of that one entry where no order is named.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quota_share: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner_account: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
}

/// SupervisorRegistered: a supervisor registered, and what became of its brain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRegistered {
  pub supervisor: Supervisor,
  pub brain: BrainBirth,
}

/// SupervisorChange: the fields a change of a supervisor names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<Vec<OrderEntry>>,
  /// The provider forced as its active provider; null clears it.
  #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
  pub active: Option<Option<ActiveProvider>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quota_share: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub warming: Option<BTreeMap<String, bool>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
  /// the rank of the demands of the runs for it; null for none
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_rank: Option<Option<u32>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub gates: Option<BTreeMap<String, bool>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sampling: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub voice: Option<String>,
}

/// DeleteSupervisorQuery: what a supervisor's removal is asked with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSupervisorQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub force: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub forget_brain: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub rooms: Option<String>,
}

/// RoleBundle: one bundle a role may give, with its kebab-case code and its label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleBundle {
  pub name: String,
  pub code: String,
  pub label: String,
}

/// Role: one role as its row holds it, and what stands of it now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Role {
  pub name: String,
  pub code: String,
  pub label: String,
  pub prompt: Option<String>,
  pub description: Option<String>,
  pub bundles: Vec<RoleBundle>,
  pub line: String,
  pub order: Vec<OrderEntry>,
  pub quota_share: f64,
  /// the provider the owner forced as its active provider, or null where its order chooses
  #[serde(default)]
  pub active: Option<ActiveProvider>,
  /// the most of its runs that go at once
  pub max_concurrent: i64,
  pub sessions: RoleSessions,
  pub fold_ratio: f64,
  pub callable: bool,
  /// Whether its order is empty, so its runs go out on the road of the supervisor that asks them; a screen
  /// shows its road as the supervisor's.
  #[serde(default)]
  pub follows_supervisor: bool,
  pub running: i64,
  pub waiting: i64,
  /// its active provider, or null where none of its order has quota or its order is empty
  pub provider: Option<String>,
  pub waiting_for: Option<String>,
}

/// RoleList: every role, every bundle a role may give and every line a role may name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleList {
  pub roles: Vec<Role>,
  pub bundles: Vec<RoleBundle>,
  pub lines: Vec<String>,
}

/// RoleAsk: the body that adds a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleAsk {
  pub name: String,
  pub label: String,
  pub bundles: Vec<String>,
  pub line: String,
  pub order: Vec<OrderEntry>,
  pub max_concurrent: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quota_share: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sessions: Option<RoleSessions>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub callable: Option<bool>,
}

/// RoleChange: the fields a change of a role names; a prompt or a description given as null clears it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleChange {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bundles: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<Vec<OrderEntry>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quota_share: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max_concurrent: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
  pub prompt: Option<Option<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
  pub description: Option<Option<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sessions: Option<RoleSessions>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub callable: Option<bool>,
  /// The provider forced as its active provider; null clears it.
  #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
  pub active: Option<Option<ActiveProvider>>,
}

/// RoleSession: one session a role's runs opened, and whether the next run of its key would resume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleSession {
  pub id: i64,
  pub session_id: String,
  pub path: String,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  pub account: Option<String>,
  /// epoch milliseconds
  pub opened_at: i64,
  /// epoch milliseconds
  pub last_answered_at: Option<i64>,
  pub runs: i64,
  pub context: RoleSessionContext,
  pub valid: bool,
}

/// RoleSessionContext: what a session's last answered turn carried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleSessionContext {
  /// The tokens it carried, or null where none was read.
  pub tokens: Option<i64>,
  /// The share of the window they filled, from 0 to 1, or null.
  pub share: Option<f64>,
}

/// RoleSessionPage: the sessions a role's runs opened, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleSessionPage {
  pub items: Vec<RoleSession>,
  pub more: bool,
}

/// RoleSessionsQuery: the page of a role's sessions asked for.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleSessionsQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// ActiveProvider: the provider and model the owner forced as a supervisor's or a role's active provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProvider {
  pub provider: String,
  pub model: String,
}

/// RepositoryPin: a repository's pinned provider and model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryPin {
  pub provider: String,
  pub model: String,
}

/// CheckoutJob: the job holding a checkout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutJob {
  pub job_id: Option<i64>,
  pub item_id: Option<i64>,
  /// the role the job runs as
  pub role: Option<String>,
}

/// RepositoryCheckout: one checkout of a repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCheckout {
  pub name: String,
  pub path: String,
  /// The work node it stands on, or null for this machine.
  pub node_name: Option<String>,
  /// `base`, `support` or `node`
  pub kind: String,
  /// The job holding it, or null where it is free.
  pub job: Option<CheckoutJob>,
}

/// SamplingSetting: one setting a sampling may hold, with its bounds; a switch has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SamplingSetting {
  pub name: String,
  /// Whether it takes on or off rather than a number.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub switch: Option<bool>,
  /// Whether the chat template reads it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub template: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub min: Option<f64>,
  /// the most it takes, or null where it is held below the registration's window
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max: Option<f64>,
  /// Whether the least value itself is refused.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub above_min: Option<bool>,
  /// Whether only whole numbers are taken.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub whole: Option<bool>,
}

/// NodeCheckoutAsk: one checkout on a work node a repository's change names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeCheckoutAsk {
  pub node_name: String,
  /// written from the node's home or absolute; null for the default beside the base's folder
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
}

/// WorkNode: one node the node service keeps, with whether it takes jobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkNode {
  pub name: String,
  pub label: String,
  /// `ssh`, or `jake` for a node reached through a hub
  pub kind: String,
  /// the services the node service reserves it for, or null
  pub role: Option<String>,
  pub state: String,
  /// epoch seconds
  pub checked_at: Option<i64>,
  pub takes_jobs: bool,
  /// why it takes no jobs, or null where it takes them
  pub jobs_refusal: Option<String>,
}

/// WorkNodeList: the nodes the node service keeps, in its list's order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkNodeList {
  pub items: Vec<WorkNode>,
  pub more: bool,
}

/// Repository: one repository as its row holds it, with its checkouts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
  pub name: String,
  pub label: Option<String>,
  pub icon: Option<String>,
  pub owner_account: Option<i64>,
  pub path: String,
  pub node_name: Option<String>,
  pub support_count: i64,
  pub prefix_documents: Vec<String>,
  pub preamble: Option<String>,
  pub rules: Option<String>,
  pub dev_command: Option<String>,
  pub pin: Option<RepositoryPin>,
  pub downgrade_heal: bool,
  /// each setting it holds by name: a number or a switch; a setting left out is the engine's own default
  pub sampling: Option<BTreeMap<String, Value>>,
  /// every setting its sampling may hold, with its bounds
  pub sampling_settings: Vec<SamplingSetting>,
  pub checkouts: Vec<RepositoryCheckout>,
}

/// RepositoryList: every repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryList {
  pub items: Vec<Repository>,
  pub more: bool,
}

/// RepositoryAsk: the body that adds a repository, or the fields a change names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAsk {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner_account: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub support_count: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prefix_documents: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub preamble: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub rules: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dev_command: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
  pub pin: Option<Option<RepositoryPin>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub downgrade_heal: Option<bool>,
  /// each setting by name, a number, a switch, or null to leave it to the engine
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sampling: Option<BTreeMap<String, Value>>,
  /// the checkouts on the work nodes, the list whole; a change only
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub node_checkouts: Option<Vec<NodeCheckoutAsk>>,
  /// ready the repository's bench; a change only
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bench: Option<bool>,
}

/// SlotRequest: one slot request of a role with its lease.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRequest {
  pub sequence: i64,
  pub token: String,
  pub request_key: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  pub path: Option<String>,
  /// the road it is let in on; empty until it is, unless its caller fixed it
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  /// the role's cap when it was let in
  pub capacity: i64,
  /// `waiting`, `reserved`, `active`, `released` or `cancelled`
  pub state: String,
  /// epoch milliseconds
  pub enqueued_at: i64,
  pub leased_at: Option<i64>,
  pub child_id: Option<String>,
  pub ended_at: Option<i64>,
  pub outcome: Option<String>,
  pub reservation: Option<String>,
  pub yielding: bool,
  pub place_key: Option<String>,
  pub session: Option<String>,
  pub title: Option<String>,
  pub asked_by: Option<String>,
  pub agent_name: Option<String>,
  /// why it waits where its provider has no quota, or null
  pub held_for: Option<String>,
  /// who gave it back — `holder`, `owner` or `run-ended` — or null while it stands
  #[serde(default)]
  pub ended_by: Option<String>,
}

/// SlotAdmission: what an admission asked of a slot request answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotAdmission {
  /// the request, active, where it is let in; null otherwise
  pub request: Option<SlotRequest>,
  /// the card demand a slot on a registration the arbiter stands asks, or null
  pub demand: Option<Value>,
  /// `waiting`, `reserved`, `active`, `released` or `cancelled`: the request now
  pub state: String,
  /// who gave it back — `holder`, `owner` or `run-ended` — or null while it stands
  pub ended_by: Option<String>,
  /// the words it was given back with, or null while it stands
  pub outcome: Option<String>,
}

/// SlotFinished: what giving a slot request back answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotFinished {
  /// whether it was still held
  pub held: bool,
  pub stood_down: bool,
  pub note: Option<String>,
}

/// SlotRequestMoved: what a move of a slot request answers: the request bound, the request given back, or an
/// admission. Each is read in that order, the first whose required members the answer holds: a bound request
/// carries its token, a given-back one `held`, and an admission neither.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SlotRequestMoved {
  Bound(SlotRequest),
  Finished(SlotFinished),
  Admission(SlotAdmission),
}

/// OwnerRelease: what an owner's release gave back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRelease {
  /// its requests that waited or stood reserved, cancelled
  pub cancelled: i64,
  /// its requests that were let in, released
  pub released: i64,
  /// its reservations, released
  pub reservations: i64,
  /// the role sessions its leases were bound to, ended
  pub sessions_ended: Vec<String>,
}

/// OwnerReleaseQuery: whose holds an owner's release gives back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReleaseQuery {
  pub owner: String,
}

/// SlotRequestAsk: the body that asks a slot of a role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRequestAsk {
  pub request_key: String,
  pub owner: String,
  /// the doppelganger where a delegation names none
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub path: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub place_key: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub yielding: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub token: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub session: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub asked_by: Option<String>,
  /// true where a supervisor's double asks, which delegated runs holding the role's whole cap refuse
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub from_double: Option<bool>,
}

/// SlotRequestMove: a slot request asked for its admission, bound to its run, or given back.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRequestMove {
  /// `bound` or `finished`; none asks for admission
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub child_id: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub outcome: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cancel: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub demand_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub within: Option<i64>,
  /// The rank a demand this work asks stands at, a whole number of zero or more.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_rank: Option<u32>,
  /// The pool a demand this work asks stands in.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub card_pool: Option<CardPool>,
}

/// SlotRequestPage: the open slot requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRequestPage {
  pub items: Vec<SlotRequest>,
  pub more: bool,
}

/// SlotRequestsQuery: the open slot requests narrowed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRequestsQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub waiting: Option<bool>,
}

/// SlotRun: one run let in on a role's slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlotRun {
  pub token: String,
  pub role: String,
  pub path: Option<String>,
  pub provider: String,
  pub model: String,
  pub source: String,
  /// epoch milliseconds
  pub since: Option<i64>,
  /// the run a binding named since the slot's last start; else, for a slot bound to a role session, the run
  /// name of that session's running turn, null while none runs; else the child it is bound to, null until bound
  pub run_name: Option<String>,
  /// the role session it is bound to, or null
  pub session: Option<String>,
  /// the job whose run it is, where its source is `job`
  pub job_id: Option<i64>,
  pub title: Option<String>,
}

/// RoleSlots: one role's slots: its cap, its runs let in, reserved and waiting, and each run let in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleSlots {
  pub role: String,
  pub max_concurrent: i64,
  pub active: i64,
  pub reserved: i64,
  pub waiting: i64,
  pub runs: Vec<SlotRun>,
}

/// Slots: every role's slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Slots {
  pub items: Vec<RoleSlots>,
}

/// ProviderModel: one model an order's entry may name on a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
  pub id: String,
  /// the levels it takes; null in the list stands for the provider's own default
  pub efforts: Vec<Option<String>>,
  /// whether it costs nothing, where the catalogue prices it
  pub free: Option<bool>,
  /// whether it is served rather than superseded, where the catalogue tells them apart
  pub in_service: Option<bool>,
  /// its window, for a gateway road's member or a registration
  pub context_tokens: Option<i64>,
  /// its answer ceiling, for a gateway road's member or a registration
  pub answer_tokens: Option<i64>,
  /// its thinking default, for a gateway road's member or a registration
  pub thinking: Option<bool>,
}

/// ProviderModels: the models an order's entry may name on a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModels {
  pub provider: String,
  pub models: Vec<ProviderModel>,
  /// the moment of a stale read in epoch seconds, or null
  pub stale_read_at: Option<f64>,
}
