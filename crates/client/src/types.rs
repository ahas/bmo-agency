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
  #[serde(rename = "worker")]
  Worker,
  #[serde(rename = "room")]
  Room,
  #[serde(rename = "agent")]
  Agent,
  #[serde(rename = "editor")]
  Editor,
  #[serde(rename = "apart")]
  Apart,
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
  #[serde(rename = "admitted")]
  Admitted,
  #[serde(rename = "bound")]
  Bound,
  #[serde(rename = "finished")]
  Finished,
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
  #[serde(rename = "expired")]
  Expired,
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
  pub bound_ms: Option<i64>,
  pub options: Value,
  /// The prompt document the session starts from, one of GET /prompt-documents. A run with a fixed prompt names its document here and carries its own bounds in boundMs and options.
  pub prompt_document_name: Option<String>,
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
  pub answer: Option<String>,
  pub text: Vec<Value>,
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
  pub spent_tokens: Option<i64>,
  pub counts: Value,
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

/// SeatRequest:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequest {
  pub token: String,
  pub source: String,
  pub owner: String,
  pub role: String,
  pub tier: String,
  pub work_key: Option<String>,
  pub state: SeatRequestState,
  pub child_id: Option<String>,
  pub outcome: Option<String>,
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

/// Role:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Role {
  pub name: String,
  pub tier: String,
  pub pooled: bool,
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

/// Provider:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
  pub name: String,
  pub enabled: bool,
  pub registered: bool,
  pub coworker: bool,
}

/// Model:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub key: String,
  pub provider: String,
  pub model: String,
  pub tier: Option<String>,
  pub healthy: Option<bool>,
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
}

/// WorkKey:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkKey {
  pub key: String,
  pub provider: String,
  pub model: String,
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

/// Account:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Account {
  pub id: String,
  pub provider: String,
  pub user_name: Option<String>,
  pub held: bool,
  pub home: Option<String>,
  pub window: Value,
  /// epoch milliseconds
  pub rest_until: Option<i64>,
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
  pub keepalive_stopped: bool,
  pub downgrade_return: Value,
}

/// Companion:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Companion {
  pub name: String,
  pub label: String,
  pub provider: String,
  pub model: String,
  pub worker_name: Option<String>,
  pub room_ids: Vec<Value>,
}

/// CompanionReset:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionReset {
  pub agent_name: String,
  pub ranges: Value,
  pub room_ids: Vec<Value>,
  pub gone: Value,
  pub stayed: Value,
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
  pub state: String,
  pub sampling: Value,
  /// epoch milliseconds
  pub resting_since: Option<i64>,
  pub session_id: Option<String>,
  pub run_count: i64,
  /// epoch milliseconds
  pub last_run_at: Option<i64>,
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

/// PromptDocumentPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptDocumentPage {
  pub items: Vec<PromptDocument>,
  pub more: bool,
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

/// RolePage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RolePage {
  pub items: Vec<Role>,
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

/// CompanionPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionPage {
  pub items: Vec<Companion>,
  pub more: bool,
}

/// WorkerPage:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerPage {
  pub items: Vec<Worker>,
  pub more: bool,
}

/// Assignment: A worker held for one item for the harness that asked. held until POST /jobs uses it or it runs out at expiresAt; running while its job runs; released when the harness lets it go or the job ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
  pub id: i64,
  pub item_id: i64,
  pub holder_name: String,
  pub worker_name: String,
  pub state: AssignmentState,
  /// epoch milliseconds
  pub expires_at: i64,
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
  pub session_id: Option<String>,
  pub run_id: Option<i64>,
  pub run_name: Option<String>,
  pub state: JobState,
  pub outcome: Option<String>,
  pub report: Option<String>,
  pub token_count: i64,
  /// epoch milliseconds
  pub started_at: i64,
  /// epoch milliseconds
  pub ended_at: Option<i64>,
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

/// SessionSummary:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
  pub agent_name: String,
  pub text: String,
  /// epoch milliseconds
  pub changed_at: i64,
}

/// SummaryTask:
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTask {
  pub agent_name: String,
  pub task: String,
}

/// The body that asks for a worker to be assigned to an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignWorker {
  pub item_id: i64,
  pub holder_name: String,
  pub item: JobItem,
  pub assignee_name: Option<String>,
  pub hold_ms: Option<i64>,
}

/// The body that releases an assignment or holds it longer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssignmentChange {
  pub state: Option<AssignmentState>,
  pub hold_ms: Option<i64>,
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
}

/// The body that ends or stops a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobChange {
  pub state: Option<JobState>,
  pub outcome: Option<String>,
  pub report: Option<String>,
}

/// The body that records the item a session serves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionWorkChange {
  pub item_id: Option<i64>,
}

/// The body that keeps an agent's session summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryWrite {
  pub text: String,
}
