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
  #[serde(rename = "apart")]
  Apart,
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
  pub bound_ms: Option<i64>,
  pub options: Value,
  /// The prompt document the session starts from, one of GET /prompt-documents. A run with a fixed prompt names its document here and carries its own bounds in boundMs and options.
  pub prompt_document_name: Option<String>,
  /// The share of its model's window the session may fill before it is folded once the turn that crossed it is over: the fold-ratio table's value for the provider it stands on and the tier its agent's work key is assigned, read with the session's state; none in a list, or where the state names no provider.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fold_ratio: Option<f64>,
  /// The tier the session's agent's work key is assigned, which its fold ratio is read at; none where the key
  /// is assigned none.
  #[serde(default)]
  pub tier: Option<String>,
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

/// RunUsage: what a chat turn's run spent, each figure summed over the models that answered it.
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
  /// The session a worker's own turn continues, or none where it opens a fresh one.
  pub session: Option<String>,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub bytes: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sha256: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub engine: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub registration: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub note: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub thinking: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub temperature: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub top_p: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub presence_penalty: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_tokens: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
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

/// StandIn: whether a stand-in answers the agent named now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StandIn {
  pub answers: bool,
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

/// The body that registers a hook's address at a point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookRegistration {
  pub url: String,
  pub token: Option<String>,
}

/// The body that changes an agent's gates, reasoning, role or voice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentChange {
  pub gates: Option<Value>,
  pub reasoning: Option<Value>,
  pub role: Option<String>,
  pub voice: Option<Value>,
}

/// The body that registers an agent: its key, label, kind, role and owner, and the fields its kind
/// carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentAsk {
  pub name: String,
  pub label: String,
  pub kind: String,
  pub role: Option<String>,
  /// the id of the login account that owns it, none for the administrator's
  pub owner_account: Option<i64>,
  pub icon: Option<String>,
  pub thinking: Option<bool>,
  pub effort: Option<String>,
  /// the `chat` kind's pinned provider
  pub provider: Option<String>,
  /// the `chat` kind's pinned model
  pub model: Option<String>,
  /// the worker a `chat` agent's items go to
  pub worker: Option<String>,
  /// a `worker`'s checkout
  pub cwd: Option<String>,
  /// the node a `worker` stands on
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
  pub action: Option<String>,
  pub name: Option<String>,
  pub label: Option<String>,
  pub text: Option<String>,
  pub agent: Option<String>,
  pub prompt: Option<String>,
  pub by: Option<String>,
}

/// The body that words which session documents moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMoveAsk {
  pub paths: Vec<String>,
  pub room_ids: Option<Vec<i64>>,
  pub rooms: Option<Vec<i64>>,
  pub now: Option<bool>,
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
  pub agent_name: Option<String>,
  pub agent: Option<String>,
  pub scope: Value,
  pub room_id: Option<i64>,
  pub bound_ms: Option<i64>,
  pub prompt_document_name: Option<String>,
  pub options: Option<SessionOptions>,
  pub provider: Option<String>,
  pub model: Option<String>,
  pub cwd: Option<String>,
  pub todo: Option<i64>,
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
  pub provider: Option<String>,
  pub model: Option<String>,
  pub options: Option<SessionChangeOptions>,
  pub holder: Option<String>,
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

/// A job turn's parts as its runner names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPartsAsk {
  pub provider: Option<String>,
  pub model: Option<String>,
  pub mapping: Option<Value>,
  pub resume: Option<Value>,
  pub cwd: Option<String>,
  pub name: Option<String>,
  pub title: Option<String>,
  pub chat: Option<i64>,
  pub unit: Option<String>,
  pub user: Option<String>,
  pub work_key: Option<String>,
  pub hears_words: Option<bool>,
  pub variables: Option<Value>,
}

/// The body that sends a turn: its prompt, the context around it, and a job turn's parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnSend {
  pub prompt: String,
  /// The context as the answer echoes it; the parts below are what the turn reads.
  pub context: Option<Value>,
  pub room: Option<RoomBlockAsk>,
  pub via: Option<String>,
  pub base_persona: Option<String>,
  pub privileged: Option<bool>,
  pub answers: Option<Vec<i64>>,
  pub began: Option<i64>,
  /// The round of the room the turn belongs to, which the turn's run keeps.
  pub round: Option<String>,
  pub job: Option<i64>,
  pub item: Option<i64>,
  pub provider: Option<String>,
  pub model: Option<String>,
  pub mapping: Option<Value>,
  pub resume: Option<Value>,
  pub cwd: Option<String>,
  pub name: Option<String>,
  pub title: Option<String>,
  pub chat: Option<i64>,
  pub unit: Option<String>,
  pub user: Option<String>,
  pub work_key: Option<String>,
  pub hears_words: Option<bool>,
  pub variables: Option<Value>,
}

/// The body that interrupts a turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterrupt {
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
  pub question_id: Option<String>,
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
  pub chat_id: Option<i64>,
  pub unit: String,
  pub depth: u32,
  pub cap_ms: Option<i64>,
  pub started_at: i64,
  pub started_by: Option<String>,
  pub search_id: Option<Value>,
}

/// The body that closes a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunClose {
  /// The message the run's answer went out as; named alone, it is attached and the closing left as it
  /// stands.
  pub message_id: Option<i64>,
  pub finished_at: Option<i64>,
  pub outcome: Option<String>,
  pub context_tokens: Option<i64>,
  pub state: Option<String>,
  /// The older name for `finishedAt`, read where that is absent.
  #[serde(rename = "finished_at")]
  pub finished_at_legacy: Option<i64>,
  /// The older name for `contextTokens`, read where that is absent.
  #[serde(rename = "context_tokens")]
  pub context_tokens_legacy: Option<i64>,
}

/// The body that writes a note into a run's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogLineAsk {
  pub agent: Option<String>,
  pub agent_name: Option<String>,
  pub run: Option<Value>,
  pub run_id: Option<Value>,
  pub chat: Option<i64>,
  pub chat_id: Option<i64>,
  pub job: Option<i64>,
  pub job_id: Option<i64>,
  pub text: String,
  pub kind: Option<String>,
  pub at: Option<i64>,
  pub source: Option<String>,
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
  pub ask: Option<i64>,
  pub verdict: String,
  pub minutes: Option<i64>,
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
  pub tier: Option<String>,
  pub provider: Option<String>,
  pub work_key: Option<String>,
  pub generation: Option<i64>,
  pub yielding: Option<bool>,
  pub token: Option<String>,
  pub place_key: Option<String>,
  pub source: Option<String>,
}

/// The body that moves a seat request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeatRequestChange {
  pub state: Option<String>,
  pub action: Option<String>,
  pub child_id: Option<String>,
  pub outcome: Option<String>,
  pub cancel: Option<bool>,
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
  pub tier: Option<String>,
  pub provider: Option<String>,
  pub work_key: Option<String>,
  pub count: i64,
}

/// The body that releases a reservation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationChange {
  pub outcome: Option<String>,
}

/// The body that takes a branch place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAsk {
  pub cwd: String,
  pub selection: String,
  pub role: String,
  pub tier: Option<String>,
  pub held: Option<Vec<Value>>,
}

/// The body that prepares, remembers or retires a place, or records its last write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlaceChange {
  pub state: Option<String>,
  pub source: Option<String>,
  pub cwd: Option<String>,
  pub slot: Option<i64>,
  pub role: Option<String>,
  pub mode: Option<String>,
  pub model: Option<String>,
  pub effort: Option<String>,
  pub upstream: Option<String>,
  pub used: Option<Vec<i64>>,
  pub held: Option<Vec<Value>>,
  pub provider: Option<String>,
  pub last_write: Option<Value>,
}

/// The body that throws a provider's switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderChange {
  pub enabled: Option<bool>,
}

/// The body that saves or removes a registration, or sets or removes one of its addresses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelChange {
  pub action: Option<String>,
  pub name: Option<String>,
  pub label: Option<String>,
  pub model: Option<String>,
  pub context_tokens: Option<i64>,
  pub answer_tokens: Option<i64>,
  pub wait_seconds: Option<i64>,
  pub thinking: Option<bool>,
  pub walked: Option<bool>,
  pub keeps_research: Option<bool>,
  pub node: Option<String>,
  pub url: Option<String>,
  pub key: Option<String>,
}

/// The body that assigns a tier to a work key or sets a tier's mapping on a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TierChange {
  pub action: String,
  pub tier: Option<String>,
  pub key: Option<String>,
  pub accept_conflicts: Option<bool>,
  pub provider: Option<String>,
  pub model: Option<String>,
  pub thinking: Option<bool>,
  pub effort: Option<String>,
  pub answer_tokens: Option<i64>,
}

/// The body that changes the fallback screen's routes, providers and accounts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FallbackChange {
  pub action: Option<String>,
  pub order: Option<Value>,
  pub provider: Option<String>,
  pub enabled: Option<bool>,
  pub account: Option<String>,
  pub name: Option<String>,
  pub kind: Option<String>,
  pub path: Option<String>,
  /// An endpoint account's base URL.
  pub endpoint: Option<String>,
  /// An endpoint account's key file.
  pub key_file: Option<String>,
  /// An endpoint account's model ids by family word.
  pub models: Option<EndpointModels>,
  /// The base prompt the account names, `none` or null to let it go.
  pub base_prompt: Option<String>,
  pub key: Option<String>,
  pub mode: Option<String>,
  /// The registration a work key runs on at the registry's provider in place of its tier's cell, `auto`
  /// or null to read the cell again.
  pub registration: Option<String>,
}

/// The body that blocks a selection's road.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadBlockAsk {
  pub selection: String,
  pub until: i64,
  pub reason: Option<String>,
}

/// The body that tells the launcher what holds launches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LaunchHeldAsk {
  pub held: bool,
  pub reasons: Option<Vec<String>>,
}

/// The body that reports a login's session usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
  pub usage: Option<Value>,
  pub window: Option<Value>,
}

/// The body that writes a rest down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestAsk {
  pub clear_passed: Option<bool>,
  pub until: Option<f64>,
  pub kind: Option<String>,
  pub account: Option<String>,
}

/// The body that throws the warming switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsAsk {
  pub keepalive_stopped: Option<bool>,
  pub stopped: Option<bool>,
}

/// The body that adds a companion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionAsk {
  pub name: String,
  pub label: String,
  pub provider: String,
  pub model: String,
  pub worker: String,
}

/// The body that changes a companion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionChange {
  pub agent: Option<String>,
  pub by: Option<String>,
  pub reference_on: Option<bool>,
  pub own_pieces: Option<Value>,
  pub pieces: Option<Value>,
  pub persona: Option<String>,
  pub registration: Option<String>,
  pub generation: Option<Value>,
  pub thinking: Option<bool>,
}

/// The body that asks for a companion reset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionResetAsk {
  pub agent: String,
  pub conversation: Option<Value>,
  pub person: Option<Value>,
  pub alone: Option<Value>,
  pub shared: Option<Value>,
  pub dry_run: Option<bool>,
  pub by: Option<String>,
}

/// The body that adds a worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerAdd {
  pub name: String,
  pub label: Option<String>,
  pub cwd: Option<String>,
  pub icon: Option<String>,
  pub role: Option<String>,
  pub node_name: Option<String>,
}

/// The body that changes a worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkerChange {
  pub sampling: Option<Value>,
  pub node_name: Option<String>,
  pub node: Option<String>,
  pub label: Option<String>,
  pub cwd: Option<String>,
  pub icon: Option<String>,
  pub role: Option<String>,
  pub stand: Option<bool>,
  pub bench: Option<bool>,
}

/// The body that keeps a session's loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoadsAsk {
  pub documents: Vec<Value>,
}
