//! The client: one method for each route of `docs/openapi.json`.

use reqwest::{Client, Method, RequestBuilder, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::types::*;

/// What a call came to short of the route's answer.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientFailure {
  /// The service answered with a failure: its status, and its error shape where it gave one.
  Answered {
    status: u16,
    answer: Option<ApiError>,
  },
  /// Nothing answered.
  Unreachable(String),
  /// The service answered in a shape the route does not give.
  Unreadable(String),
}

impl std::fmt::Display for ClientFailure {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Answered {
        status,
        answer: Some(answer),
      } => write!(out, "the agency answered {status} {}: {}", answer.code, answer.error),
      Self::Answered {
        status,
        answer: None,
      } => write!(out, "the agency answered {status}"),
      Self::Unreachable(why) => write!(out, "the agency did not answer: {why}"),
      Self::Unreadable(why) => write!(out, "the agency's answer did not read: {why}"),
    }
  }
}

impl std::error::Error for ClientFailure {}

/// What a call answers.
pub type Answer<T> = Result<T, ClientFailure>;

/// The agency at one base address.
#[derive(Debug, Clone)]
pub struct AgencyClient {
  http: Client,
  base: Url,
  token: String,
}

impl AgencyClient {
  /// The agency whose routes stand at `base`, called with `token`; `None` where the address does not read.
  pub fn new(base: &str, token: impl Into<String>) -> Result<Self, String> {
    let base = Url::parse(base).map_err(|why| format!("{base} is no address: {why}"))?;
    if base.cannot_be_a_base() {
      return Err(format!("{base} is no base address"));
    }
    Ok(Self {
      http: Client::new(),
      base,
      token: token.into(),
    })
  }

  /// The same agency called through another HTTP client, such as one with a timeout of its own.
  pub fn with_http(mut self, http: Client) -> Self {
    self.http = http;
    self
  }

  fn request(&self, method: Method, segments: &[&str]) -> RequestBuilder {
    let mut url = self.base.clone();
    {
      let mut path = url.path_segments_mut().expect("the base takes a path");
      for segment in segments {
        path.push(segment);
      }
    }
    self.http.request(method, url).bearer_auth(&self.token)
  }

  async fn answered<T: DeserializeOwned>(request: RequestBuilder) -> Answer<T> {
    let answer = Self::sent(request).await?;
    let status = answer.status();
    let bytes = answer.bytes().await.map_err(|why| ClientFailure::Unreadable(why.to_string()))?;
    match status.as_u16() {
      200 | 201 => serde_json::from_slice(&bytes).map_err(|why| ClientFailure::Unreadable(why.to_string())),
      other => Err(ClientFailure::Unreadable(format!("{other} where the member was due"))),
    }
  }

  async fn sent(request: RequestBuilder) -> Answer<reqwest::Response> {
    let answer = request.send().await.map_err(|why| ClientFailure::Unreachable(why.to_string()))?;
    if answer.status().is_success() {
      return Ok(answer);
    }
    let status = answer.status();
    let body = answer.bytes().await.unwrap_or_default();
    Err(ClientFailure::Answered {
      status: status.as_u16(),
      answer: serde_json::from_slice(&body).ok(),
    })
  }

  async fn nothing(request: RequestBuilder) -> Answer<()> {
    let answer = Self::sent(request).await?;
    match answer.status() {
      StatusCode::NO_CONTENT => Ok(()),
      status => Err(ClientFailure::Unreadable(format!("{status} where 204 was due"))),
    }
  }
}

// One query body for each route that takes one.

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEventsQuery {
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  /// a comma-separated list
  #[serde(skip_serializing_if = "Option::is_none")]
  pub kinds: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptDocumentsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  /// a comma-separated list
  #[serde(skip_serializing_if = "Option::is_none")]
  pub names: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub scope: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRunsQuery {
  /// true the runs still open, false the closed ones
  #[serde(skip_serializing_if = "Option::is_none")]
  pub live: Option<bool>,
  /// the search a run carried; answers every run that carried it, the other filters aside
  #[serde(skip_serializing_if = "Option::is_none")]
  pub search: Option<String>,
  /// a comma-separated list
  #[serde(skip_serializing_if = "Option::is_none")]
  pub names: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
  /// the round a chat turn's context named
  #[serde(skip_serializing_if = "Option::is_none")]
  pub round: Option<String>,
  /// a run's id: the runs after it, nearest to it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  /// a run's id: the runs before it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  /// how many runs a page holds: 50 where none is named, at most 500
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLogLinesQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub run_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub job_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  /// a comma-separated list
  #[serde(skip_serializing_if = "Option::is_none")]
  pub kinds: Option<String>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExecutionsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub open: Option<bool>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOvertimeRequestsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub open: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDelegationsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub asked_by: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ended: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSeatRequestsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub owner: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSeatRequestQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub wait_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListReservationsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub owner: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPlacesQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub node_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub checkout: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWorkKeyQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub tier: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRoadBlocksQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub worker_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAccountsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub user_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetStandInQuery {
  pub agent: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUsageQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub group_by: Option<String>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCompanionsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAgentQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub force: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub forget_brain: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rooms: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRoomHoldingQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAssignmentsQuery {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub worker_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  /// a cursor: an id or a time in epoch milliseconds, as the route states
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListJobsQuery {
  /// true the jobs starting or running, false every other
  #[serde(skip_serializing_if = "Option::is_none")]
  pub running: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub item_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub worker_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder_name: Option<String>,
  /// one state, or several comma-separated
  #[serde(skip_serializing_if = "Option::is_none")]
  pub state: Option<String>,
  /// a job's id: the jobs after it, nearest to it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  /// a job's id: the jobs before it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  /// how many jobs a page holds: 50 where none is named, at most 500
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

impl AgencyClient {
  /// `GET /health`: Readiness and version.
  pub async fn get_health(&self) -> Answer<Health> {
    Self::answered(self.request(Method::GET, &["health"])).await
  }

  /// `GET /events`: The agency's one stream.
  pub async fn stream_events(&self, query: &StreamEventsQuery) -> Answer<EventPage> {
    Self::answered(
      self
        .request(Method::GET, &["events"])
        .query(query)
        .header(reqwest::header::ACCEPT, "application/json"),
    )
    .await
  }

  /// `POST /events`: A process of the agency's own publishes an event.
  pub async fn create_events(&self, body: &PublishedEvent) -> Answer<PublishedEvent> {
    Self::answered(self.request(Method::POST, &["events"]).json(body)).await
  }

  /// `GET /hooks`: The hook points and what is registered at each.
  pub async fn list_hooks(&self) -> Answer<HookPage> {
    Self::answered(self.request(Method::GET, &["hooks"])).await
  }

  /// `PUT /hooks/{point}`: Register a downstream's URL at a hook point.
  pub async fn put_hook(&self, point: &str, body: &HookRegistration) -> Answer<Hook> {
    Self::answered(self.request(Method::PUT, &["hooks", point]).json(body)).await
  }

  /// `DELETE /hooks/{point}`: Take a registration away.
  pub async fn delete_hook(&self, point: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["hooks", point])).await
  }

  /// `GET /agents`: Every agent, each with the login account that owns it.
  pub async fn list_agents(&self) -> Answer<AgentPage> {
    Self::answered(self.request(Method::GET, &["agents"])).await
  }

  /// `POST /agents`: Register an agent of any kind; one that converses is given its brain.
  pub async fn create_agents(&self, body: &AgentAsk) -> Answer<AgentCreated> {
    Self::answered(self.request(Method::POST, &["agents"]).json(body)).await
  }

  /// `GET /agents/{agentName}`: One agent: role, gates, reasoning, voice, reset point, stand-in.
  pub async fn get_agent(&self, agent_name: &str) -> Answer<Agent> {
    Self::answered(self.request(Method::GET, &["agents", agent_name])).await
  }

  /// `PATCH /agents/{agentName}`: Change an agent's gates, reasoning, role or voice.
  pub async fn change_agent(&self, agent_name: &str, body: &AgentChange) -> Answer<Agent> {
    Self::answered(self.request(Method::PATCH, &["agents", agent_name]).json(body)).await
  }

  /// `DELETE /agents/{agentName}`: End an agent's registration; `forgetBrain` lets its brain go with it.
  pub async fn delete_agent(&self, agent_name: &str, query: &DeleteAgentQuery) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["agents", agent_name]).query(query)).await
  }

  /// `GET /agent-roles`: Every role an agent may hold, and every bundle a role may give.
  pub async fn list_agent_roles(&self) -> Answer<AgentRoleList> {
    Self::answered(self.request(Method::GET, &["agent-roles"])).await
  }

  /// `POST /agent-roles`: Add a role.
  pub async fn create_agent_roles(&self, body: &AgentRoleAsk) -> Answer<AgentRole> {
    Self::answered(self.request(Method::POST, &["agent-roles"]).json(body)).await
  }

  /// `PATCH /agent-roles/{roleName}`: Change a role's label, bundles, bound, prompt or tier.
  pub async fn change_agent_role(&self, role_name: &str, body: &AgentRoleChange) -> Answer<AgentRole> {
    Self::answered(self.request(Method::PATCH, &["agent-roles", role_name]).json(body)).await
  }

  /// `DELETE /agent-roles/{roleName}`: Remove a role no agent holds and no kind is for.
  pub async fn delete_agent_role(&self, role_name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["agent-roles", role_name])).await
  }

  /// `GET /prompt-documents`: Deployed prompt documents: base prompts, rulebooks, personas, agent definitions, system prompts.
  pub async fn list_prompt_documents(
    &self,
    query: &ListPromptDocumentsQuery,
  ) -> Answer<PromptDocumentPage> {
    Self::answered(self.request(Method::GET, &["prompt-documents"]).query(query)).await
  }

  /// `GET /prompt-documents/{documentName}`: One prompt document with its version.
  pub async fn get_prompt_document(&self, document_name: &str) -> Answer<PromptDocument> {
    Self::answered(self.request(Method::GET, &["prompt-documents", document_name])).await
  }

  /// `PATCH /prompt-documents/{documentName}`: Change a base prompt or its chooser.
  pub async fn change_prompt_document(
    &self,
    document_name: &str,
    body: &PromptDocumentChange,
  ) -> Answer<PromptDocument> {
    Self::answered(self.request(Method::PATCH, &["prompt-documents", document_name]).json(body)).await
  }

  /// `POST /document-moves`: Word that session documents moved, so sessions read them again.
  pub async fn create_document_moves(&self, body: &DocumentMoveAsk) -> Answer<DocumentMove> {
    Self::answered(self.request(Method::POST, &["document-moves"]).json(body)).await
  }

  /// `GET /sessions`: Every session, by agent or scope.
  pub async fn list_sessions(&self, query: &ListSessionsQuery) -> Answer<SessionPage> {
    Self::answered(self.request(Method::GET, &["sessions"]).query(query)).await
  }

  /// `POST /sessions`: Start a session for an agent at a scope; a throwaway one is scope apart with boundMs and its own options. A run on a runner line (search, browse) is such a session naming the line and handing its instructions in whole as systemPrompt; it calls the upstreams it needs through their own MCP servers.
  pub async fn create_sessions(&self, body: &SessionStart) -> Answer<Session> {
    Self::answered(self.request(Method::POST, &["sessions"]).json(body)).await
  }

  /// `GET /sessions/{sessionId}`: A session's state: provider, model, place, holder, why the prefix changed.
  pub async fn get_session(&self, session_id: &str) -> Answer<Session> {
    Self::answered(self.request(Method::GET, &["sessions", session_id])).await
  }

  /// `PATCH /sessions/{sessionId}`: Hand a session over to another holder.
  pub async fn change_session(&self, session_id: &str, body: &SessionChange) -> Answer<Session> {
    Self::answered(self.request(Method::PATCH, &["sessions", session_id]).json(body)).await
  }

  /// `DELETE /sessions/{sessionId}`: End a session, or clear a room's session.
  pub async fn delete_session(&self, session_id: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["sessions", session_id])).await
  }

  /// `POST /sessions/{sessionId}/turns`: Send a turn into a session; words to a worker are a turn into its session.
  pub async fn create_sessions_turns(&self, session_id: &str, body: &TurnSend) -> Answer<Turn> {
    Self::answered(self.request(Method::POST, &["sessions", session_id, "turns"]).json(body)).await
  }

  /// `GET /sessions/{sessionId}/turns/{turnId}`: A turn and what it has written so far.
  pub async fn get_sessions_turn(&self, session_id: &str, turn_id: i64) -> Answer<Turn> {
    Self::answered(self.request(Method::GET, &["sessions", session_id, "turns", &turn_id.to_string()]))
      .await
  }

  /// `PATCH /sessions/{sessionId}/turns/{turnId}`: Interrupt a turn.
  pub async fn change_sessions_turn(
    &self,
    session_id: &str,
    turn_id: i64,
    body: &TurnInterrupt,
  ) -> Answer<Turn> {
    Self::answered(
      self
        .request(Method::PATCH, &["sessions", session_id, "turns", &turn_id.to_string()])
        .json(body),
    )
    .await
  }

  /// `POST /sessions/{sessionId}/turns/{turnId}/inputs`: Push words into a working turn.
  pub async fn create_sessions_turns_inputs(
    &self,
    session_id: &str,
    turn_id: i64,
    body: &TurnWords,
  ) -> Answer<TurnInput> {
    Self::answered(
      self
        .request(Method::POST, &["sessions", session_id, "turns", &turn_id.to_string(), "inputs"])
        .json(body),
    )
    .await
  }

  /// `POST /sessions/{sessionId}/answers`: Answer a question a turn asked.
  pub async fn create_sessions_answers(&self, session_id: &str, body: &AnswerAsk) -> Answer<TurnAnswer> {
    Self::answered(self.request(Method::POST, &["sessions", session_id, "answers"]).json(body)).await
  }

  /// `POST /sessions/{sessionId}/folds`: Fold a session, compacting its context.
  pub async fn create_sessions_folds(&self, session_id: &str, body: &FoldAsk) -> Answer<Fold> {
    Self::answered(self.request(Method::POST, &["sessions", session_id, "folds"]).json(body)).await
  }

  /// `POST /turn-contexts`: The context the submit hook would add to a typed line.
  pub async fn make_turn_contexts(&self, body: &TurnContextAsk) -> Answer<TurnContext> {
    Self::answered(self.request(Method::POST, &["turn-contexts"]).json(body)).await
  }

  /// `GET /runs`: Runs by search: live, recent, by agent, room, model, job, worker, names; with the tokens spent so far.
  pub async fn list_runs(&self, query: &ListRunsQuery) -> Answer<RunPage> {
    Self::answered(self.request(Method::GET, &["runs"]).query(query)).await
  }

  /// `POST /runs`: Open a run in the ledger.
  pub async fn create_runs(&self, body: &RunOpen) -> Answer<Run> {
    Self::answered(self.request(Method::POST, &["runs"]).json(body)).await
  }

  /// `GET /runs/{runId}`: One run, its clock bound among its fields.
  pub async fn get_run(&self, run_id: i64) -> Answer<Run> {
    Self::answered(self.request(Method::GET, &["runs", &run_id.to_string()])).await
  }

  /// `PATCH /runs/{runId}`: Close a run, or stop it.
  pub async fn change_run(&self, run_id: i64, body: &RunClose) -> Answer<Run> {
    Self::answered(self.request(Method::PATCH, &["runs", &run_id.to_string()]).json(body)).await
  }

  /// `GET /log-lines`: Log lines across runs: by run, job or agent, kinds, latest, before or after.
  pub async fn list_log_lines(&self, query: &ListLogLinesQuery) -> Answer<LogLinePage> {
    Self::answered(self.request(Method::GET, &["log-lines"]).query(query)).await
  }

  /// `POST /log-lines`: Add a line to a run's log: a note, a turn record, a program's output.
  pub async fn create_log_lines(&self, body: &LogLineAsk) -> Answer<LogLine> {
    Self::answered(self.request(Method::POST, &["log-lines"]).json(body)).await
  }

  /// `GET /executions`: Executions, latest first.
  pub async fn list_executions(&self, query: &ListExecutionsQuery) -> Answer<ExecutionPage> {
    Self::answered(self.request(Method::GET, &["executions"]).query(query)).await
  }

  /// `POST /executions`: Launch a run's program in the agent host.
  pub async fn create_executions(&self, body: &ExecutionLaunch) -> Answer<Execution> {
    Self::answered(self.request(Method::POST, &["executions"]).json(body)).await
  }

  /// `GET /executions/{executionId}`: One execution.
  pub async fn get_execution(&self, execution_id: i64) -> Answer<Execution> {
    Self::answered(self.request(Method::GET, &["executions", &execution_id.to_string()])).await
  }

  /// `PATCH /executions/{executionId}`: Cancel an execution or take the cancel back.
  pub async fn change_execution(&self, execution_id: i64, body: &ExecutionChange) -> Answer<Execution> {
    Self::answered(self.request(Method::PATCH, &["executions", &execution_id.to_string()]).json(body)).await
  }

  /// `GET /overtime-requests`: Runs asking for more time.
  pub async fn list_overtime_requests(
    &self,
    query: &ListOvertimeRequestsQuery,
  ) -> Answer<OvertimeRequestPage> {
    Self::answered(self.request(Method::GET, &["overtime-requests"]).query(query)).await
  }

  /// `PATCH /overtime-requests/{requestId}`: Answer an overtime request.
  pub async fn change_overtime_request(
    &self,
    request_id: i64,
    body: &OvertimeAnswer,
  ) -> Answer<OvertimeRequest> {
    Self::answered(self.request(Method::PATCH, &["overtime-requests", &request_id.to_string()]).json(body))
      .await
  }

  /// `GET /stop-marks/{jobId}`: Why a job is to stop, if it is.
  pub async fn get_stop_mark(&self, job_id: i64) -> Answer<StopMark> {
    Self::answered(self.request(Method::GET, &["stop-marks", &job_id.to_string()])).await
  }

  /// `PUT /stop-marks/{jobId}`: Mark a job to stop.
  pub async fn put_stop_mark(&self, job_id: i64, body: &StopMarkAsk) -> Answer<StopMark> {
    Self::answered(self.request(Method::PUT, &["stop-marks", &job_id.to_string()]).json(body)).await
  }

  /// `DELETE /stop-marks/{jobId}`: Clear a job's stop mark.
  pub async fn delete_stop_mark(&self, job_id: i64) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["stop-marks", &job_id.to_string()])).await
  }

  /// `GET /pool`: The co-worker pool: buckets, seats, waiting work.
  pub async fn get_pool(&self) -> Answer<Pool> {
    Self::answered(self.request(Method::GET, &["pool"])).await
  }

  /// `GET /seat-requests`: Open seat requests, by owner or waiting.
  pub async fn list_seat_requests(&self, query: &ListSeatRequestsQuery) -> Answer<SeatRequestPage> {
    Self::answered(self.request(Method::GET, &["seat-requests"]).query(query)).await
  }

  /// `GET /delegations`: The tasks an agent delegated, each with its title, stage, helper agent and start.
  pub async fn list_delegations(&self, query: &ListDelegationsQuery) -> Answer<DelegationPage> {
    Self::answered(self.request(Method::GET, &["delegations"]).query(query)).await
  }

  /// `POST /seat-requests`: Ask for a co-worker seat; the tier is resolved from the work key.
  pub async fn create_seat_requests(&self, body: &SeatRequestAsk) -> Answer<SeatRequest> {
    Self::answered(self.request(Method::POST, &["seat-requests"]).json(body)).await
  }

  /// `GET /seat-requests/{token}`: One seat request.
  pub async fn get_seat_request(&self, token: &str, query: &GetSeatRequestQuery) -> Answer<SeatRequest> {
    Self::answered(self.request(Method::GET, &["seat-requests", token]).query(query)).await
  }

  /// `PATCH /seat-requests/{token}`: Move a seat request: admitted, bound to its launched unit, finished.
  pub async fn change_seat_request(&self, token: &str, body: &SeatRequestChange) -> Answer<SeatRequest> {
    Self::answered(self.request(Method::PATCH, &["seat-requests", token]).json(body)).await
  }

  /// `GET /reservations`: Held seat reservations.
  pub async fn list_reservations(&self, query: &ListReservationsQuery) -> Answer<ReservationPage> {
    Self::answered(self.request(Method::GET, &["reservations"]).query(query)).await
  }

  /// `POST /reservations`: Reserve seats for an owner.
  pub async fn create_reservations(&self, body: &ReservationAsk) -> Answer<Reservation> {
    Self::answered(self.request(Method::POST, &["reservations"]).json(body)).await
  }

  /// `PATCH /reservations/{token}`: Release a reservation with its outcome.
  pub async fn change_reservation(&self, token: &str, body: &ReservationChange) -> Answer<Reservation> {
    Self::answered(self.request(Method::PATCH, &["reservations", token]).json(body)).await
  }

  /// `GET /roles`: The roles the pool can lend.
  pub async fn list_roles(&self) -> Answer<RolePage> {
    Self::answered(self.request(Method::GET, &["roles"])).await
  }

  /// `PATCH /pool/limits`: Set how many children of one co-worker tier may be alive on one provider.
  pub async fn change_pool_limit(&self, body: &PoolLimit) -> Answer<PoolLimit> {
    Self::answered(self.request(Method::PATCH, &["pool", "limits"]).json(body)).await
  }

  /// `GET /places`: Branch places, by node and checkout.
  pub async fn list_places(&self, query: &ListPlacesQuery) -> Answer<PlacePage> {
    Self::answered(self.request(Method::GET, &["places"]).query(query)).await
  }

  /// `POST /places`: Take a branch place and its folder.
  pub async fn create_places(&self, body: &PlaceAsk) -> Answer<Place> {
    Self::answered(self.request(Method::POST, &["places"]).json(body)).await
  }

  /// `GET /places/{placeId}`: One place.
  pub async fn get_place(&self, place_id: &str) -> Answer<Place> {
    Self::answered(self.request(Method::GET, &["places", place_id])).await
  }

  /// `PATCH /places/{placeId}`: Prepare, remember or retire a place, or record its last write.
  pub async fn change_place(&self, place_id: &str, body: &PlaceChange) -> Answer<Place> {
    Self::answered(self.request(Method::PATCH, &["places", place_id]).json(body)).await
  }

  /// `GET /providers`: Providers in walk order.
  pub async fn list_providers(&self) -> Answer<ProviderPage> {
    Self::answered(self.request(Method::GET, &["providers"])).await
  }

  /// `GET /providers/{providerName}`: One provider.
  pub async fn get_provider(&self, provider_name: &str) -> Answer<Provider> {
    Self::answered(self.request(Method::GET, &["providers", provider_name])).await
  }

  /// `PATCH /providers/{providerName}`: Change a provider.
  pub async fn change_provider(&self, provider_name: &str, body: &ProviderChange) -> Answer<Provider> {
    Self::answered(self.request(Method::PATCH, &["providers", provider_name]).json(body)).await
  }

  /// `GET /models`: Registered models, by provider or tier.
  pub async fn list_models(&self, query: &ListModelsQuery) -> Answer<ModelPage> {
    Self::answered(self.request(Method::GET, &["models"]).query(query)).await
  }

  /// `POST /models`: Register a model.
  pub async fn create_models(&self, body: &ModelChange) -> Answer<Model> {
    Self::answered(self.request(Method::POST, &["models"]).json(body)).await
  }

  /// `GET /models/{modelKey}`: One model and its health.
  pub async fn get_model(&self, model_key: &str) -> Answer<Model> {
    Self::answered(self.request(Method::GET, &["models", model_key])).await
  }

  /// `PATCH /models/{modelKey}`: Change a model's registration.
  pub async fn change_model(&self, model_key: &str, body: &ModelChange) -> Answer<Model> {
    Self::answered(self.request(Method::PATCH, &["models", model_key]).json(body)).await
  }

  /// `DELETE /models/{modelKey}`: Take a model's registration away.
  pub async fn delete_model(&self, model_key: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["models", model_key])).await
  }

  /// `GET /model-catalogue`: The model files the deployment's engines serve, and each engine's latest
  /// choice.
  pub async fn list_model_catalogue(&self) -> Answer<ModelCatalogueView> {
    Self::answered(self.request(Method::GET, &["model-catalogue"])).await
  }

  /// `POST /model-catalogue`: Save or remove an entry, or choose one for its engine.
  pub async fn change_model_catalogue(&self, body: &ModelCatalogueChange) -> Answer<ModelCatalogueView> {
    Self::answered(self.request(Method::POST, &["model-catalogue"]).json(body)).await
  }

  /// `POST /pin-checks`: Read a worker pin value: auto, a provider, or a provider's model.
  pub async fn make_pin_checks(&self, body: &PinCheckAsk) -> Answer<PinCheck> {
    Self::answered(self.request(Method::POST, &["pin-checks"]).json(body)).await
  }

  /// `GET /tiers`: Tiers in ladder order, each with its mapping.
  pub async fn list_tiers(&self) -> Answer<TierPage> {
    Self::answered(self.request(Method::GET, &["tiers"])).await
  }

  /// `PATCH /tiers/{tierName}`: Change a tier's mapping for a role.
  pub async fn change_tier(&self, tier_name: &str, body: &TierChange) -> Answer<Tier> {
    Self::answered(self.request(Method::PATCH, &["tiers", tier_name]).json(body)).await
  }

  /// `GET /fold-ratios`: Every provider's fold ratio at every tier, each with where it comes from.
  pub async fn list_fold_ratios(&self) -> Answer<FoldRatioPage> {
    Self::answered(self.request(Method::GET, &["fold-ratios"])).await
  }

  /// `PUT /fold-ratios/{provider}/{tier}`: Set a provider's fold ratio at a tier.
  pub async fn put_fold_ratio(
    &self,
    provider: &str,
    tier: &str,
    body: &FoldRatioWrite,
  ) -> Answer<FoldRatio> {
    Self::answered(self.request(Method::PUT, &["fold-ratios", provider, tier]).json(body)).await
  }

  /// `GET /fallback`: The fallback roads and whether the fallback is answering.
  pub async fn get_fallback(&self) -> Answer<Fallback> {
    Self::answered(self.request(Method::GET, &["fallback"])).await
  }

  /// `PATCH /fallback`: Change the fallback roads.
  pub async fn change_fallback(&self, body: &FallbackChange) -> Answer<Fallback> {
    Self::answered(self.request(Method::PATCH, &["fallback"]).json(body)).await
  }

  /// `POST /item-filings`: File an item in the job service for the turn an agent is carrying now.
  pub async fn create_item_filings(&self, body: &ItemFilingAsk) -> Answer<ItemFiled> {
    Self::answered(self.request(Method::POST, &["item-filings"]).json(body)).await
  }

  /// `POST /job-stops`: Stop a job, by its id or by its item, for the turn an agent is carrying now.
  pub async fn create_job_stops(&self, body: &JobStopAsk) -> Answer<Job> {
    Self::answered(self.request(Method::POST, &["job-stops"]).json(body)).await
  }

  /// `GET /work-keys`: Every work key with the tier it is assigned, the tiers it may be, and what it resolves to on each provider.
  pub async fn list_work_keys(&self) -> Answer<WorkKeyAssignmentPage> {
    Self::answered(self.request(Method::GET, &["work-keys"])).await
  }

  /// `GET /work-keys/{workKey}`: Where a work key goes: provider, model, effort, answer tokens, run bound, run mode.
  pub async fn get_work_key(&self, work_key: &str, query: &GetWorkKeyQuery) -> Answer<WorkKey> {
    Self::answered(self.request(Method::GET, &["work-keys", work_key]).query(query)).await
  }

  /// `GET /road-blocks`: Roads blocked now, by model or worker; none means open.
  pub async fn list_road_blocks(&self, query: &ListRoadBlocksQuery) -> Answer<RoadBlockPage> {
    Self::answered(self.request(Method::GET, &["road-blocks"]).query(query)).await
  }

  /// `POST /road-blocks`: Block a road until a time.
  pub async fn create_road_blocks(&self, body: &RoadBlockAsk) -> Answer<RoadBlock> {
    Self::answered(self.request(Method::POST, &["road-blocks"]).json(body)).await
  }

  /// `DELETE /road-blocks/{blockId}`: Open a road again.
  pub async fn delete_road_block(&self, block_id: i64) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["road-blocks", &block_id.to_string()])).await
  }

  /// `GET /launch-room`: Whether a launch may start, why not, and the jobs and browsers running.
  pub async fn get_launch_room(&self) -> Answer<LaunchRoom> {
    Self::answered(self.request(Method::GET, &["launch-room"])).await
  }

  /// `PATCH /launch-room`: Record that launches are held, and why.
  pub async fn change_launch_room(&self, body: &LaunchHeldAsk) -> Answer<LaunchRoom> {
    Self::answered(self.request(Method::PATCH, &["launch-room"]).json(body)).await
  }

  /// `GET /accounts`: Accounts: held, home, usage window, rest.
  pub async fn list_accounts(&self, query: &ListAccountsQuery) -> Answer<AccountPage> {
    Self::answered(self.request(Method::GET, &["accounts"]).query(query)).await
  }

  /// `PATCH /accounts/{accountId}`: Record an account's usage and its rest.
  pub async fn change_account(&self, account_id: &str, body: &AccountUsage) -> Answer<Account> {
    Self::answered(self.request(Method::PATCH, &["accounts", account_id]).json(body)).await
  }

  /// `GET /rest`: Whether the deployment rests, until when, of what kind.
  pub async fn get_rest(&self) -> Answer<Rest> {
    Self::answered(self.request(Method::GET, &["rest"])).await
  }

  /// `PATCH /rest`: Set the rest, or clear a rest that has passed.
  pub async fn change_rest(&self, body: &RestAsk) -> Answer<Rest> {
    Self::answered(self.request(Method::PATCH, &["rest"]).json(body)).await
  }

  /// `GET /stand-in`: Whether a stand-in answers the agent named while the rest closes Claude Code's road.
  pub async fn get_stand_in(&self, query: &GetStandInQuery) -> Answer<StandIn> {
    Self::answered(self.request(Method::GET, &["stand-in"]).query(query)).await
  }

  /// `GET /usage`: Usage by model, agent, user or room over a span, with session contexts.
  pub async fn get_usage(&self, query: &GetUsageQuery) -> Answer<Usage> {
    Self::answered(self.request(Method::GET, &["usage"]).query(query)).await
  }

  /// `GET /settings`: The agency's own switches: keepalive, downgrade return.
  pub async fn get_settings(&self) -> Answer<Settings> {
    Self::answered(self.request(Method::GET, &["settings"])).await
  }

  /// `PATCH /settings`: Change the agency's switches.
  pub async fn change_settings(&self, body: &SettingsAsk) -> Answer<Settings> {
    Self::answered(self.request(Method::PATCH, &["settings"]).json(body)).await
  }

  /// `GET /companions`: Companion agents, by room.
  pub async fn list_companions(&self, query: &ListCompanionsQuery) -> Answer<CompanionPage> {
    Self::answered(self.request(Method::GET, &["companions"]).query(query)).await
  }

  /// `POST /companions`: Add a companion agent.
  pub async fn create_companions(&self, body: &CompanionAsk) -> Answer<Companion> {
    Self::answered(self.request(Method::POST, &["companions"]).json(body)).await
  }

  /// `PATCH /companions/{companionName}`: Change a companion and the rooms it answers in.
  pub async fn change_companion(&self, companion_name: &str, body: &CompanionChange) -> Answer<Companion> {
    Self::answered(self.request(Method::PATCH, &["companions", companion_name]).json(body)).await
  }

  /// `DELETE /companions/{companionName}`: Remove a companion from the deployment.
  pub async fn delete_companion(&self, companion_name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["companions", companion_name])).await
  }

  /// `POST /companion-resets`: Reset a companion's person ranges and rooms; dryRun counts it first.
  pub async fn create_companion_resets(&self, body: &CompanionResetAsk) -> Answer<CompanionReset> {
    Self::answered(self.request(Method::POST, &["companion-resets"]).json(body)).await
  }

  /// `GET /workers`: Every worker.
  pub async fn list_workers(&self) -> Answer<WorkerPage> {
    Self::answered(self.request(Method::GET, &["workers"])).await
  }

  /// `POST /workers`: Add a worker.
  pub async fn create_workers(&self, body: &WorkerAdd) -> Answer<Worker> {
    Self::answered(self.request(Method::POST, &["workers"]).json(body)).await
  }

  /// `GET /workers/{workerName}`: One worker: checkout, node, role, sampling, session, runs.
  pub async fn get_worker(&self, worker_name: &str) -> Answer<Worker> {
    Self::answered(self.request(Method::GET, &["workers", worker_name])).await
  }

  /// `PATCH /workers/{workerName}`: Change a worker: fields, node placement, sampling, stand it again, ready its bench.
  pub async fn change_worker(&self, worker_name: &str, body: &WorkerChange) -> Answer<Worker> {
    Self::answered(self.request(Method::PATCH, &["workers", worker_name]).json(body)).await
  }

  /// `DELETE /workers/{workerName}`: Remove a worker.
  pub async fn delete_worker(&self, worker_name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["workers", worker_name])).await
  }

  /// `GET /workers/{workerName}/definition`: The worker's agent definition: graph, trailer, handoff.
  pub async fn get_workers_definition(&self, worker_name: &str) -> Answer<WorkerDefinition> {
    Self::answered(self.request(Method::GET, &["workers", worker_name, "definition"])).await
  }

  /// `GET /workers/{workerName}/workbench`: The worker's workbench and scratch folders.
  pub async fn get_workers_workbench(&self, worker_name: &str) -> Answer<Workbench> {
    Self::answered(self.request(Method::GET, &["workers", worker_name, "workbench"])).await
  }

  /// `DELETE /workers/{workerName}/servers`: End the job servers a worker left running.
  pub async fn delete_workers_servers(&self, worker_name: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["workers", worker_name, "servers"])).await
  }

  /// `POST /prefix-checks`: Which of these files are in a worker's session prefix.
  pub async fn make_prefix_checks(&self, body: &PrefixCheckAsk) -> Answer<PrefixCheck> {
    Self::answered(self.request(Method::POST, &["prefix-checks"]).json(body)).await
  }

  /// `GET /room-holdings/{roomId}`: What the agency holds of a room.
  pub async fn get_room_holding(&self, room_id: i64) -> Answer<RoomHolding> {
    Self::answered(self.request(Method::GET, &["room-holdings", &room_id.to_string()])).await
  }

  /// `DELETE /room-holdings/{roomId}`: Let a room's data go; dryRun says what would go.
  pub async fn delete_room_holding(&self, room_id: i64, query: &DeleteRoomHoldingQuery) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["room-holdings", &room_id.to_string()]).query(query)).await
  }

  /// `GET /assignments`: Worker assignments, by holder, worker or state.
  pub async fn list_assignments(&self, query: &ListAssignmentsQuery) -> Answer<AssignmentPage> {
    Self::answered(self.request(Method::GET, &["assignments"]).query(query)).await
  }

  /// `POST /assignments`: Assign a worker to an item for the harness holding it: the agency picks the worker from the item's content and the assignee it names, and holds it until the job starts or the hold runs out.
  pub async fn create_assignments(&self, body: &AssignWorker) -> Answer<Assignment> {
    Self::answered(self.request(Method::POST, &["assignments"]).json(body)).await
  }

  /// `GET /assignments/{assignmentId}`: One assignment.
  pub async fn read_assignments_by_assignment_id(&self, assignment_id: i64) -> Answer<Assignment> {
    Self::answered(self.request(Method::GET, &["assignments", &assignment_id.to_string()])).await
  }

  /// `PATCH /assignments/{assignmentId}`: Release an assignment, or hold it longer.
  pub async fn change_assignments_by_assignment_id(
    &self,
    assignment_id: i64,
    body: &AssignmentChange,
  ) -> Answer<Assignment> {
    Self::answered(self.request(Method::PATCH, &["assignments", &assignment_id.to_string()]).json(body))
      .await
  }

  /// `GET /jobs`: Jobs: recent, by room, by worker, by holder, running.
  pub async fn list_jobs(&self, query: &ListJobsQuery) -> Answer<JobPage> {
    Self::answered(self.request(Method::GET, &["jobs"]).query(query)).await
  }

  /// `POST /jobs`: Run an item on its assigned worker: the harness hands over the item's content and the lease it holds, and the agency launches the worker's session on it. The agency calls nothing of the job service.
  pub async fn create_jobs(&self, body: &RunJob) -> Answer<Job> {
    Self::answered(self.request(Method::POST, &["jobs"]).json(body)).await
  }

  /// `POST /github/deliveries`: One GitHub delivery of a pull request event, its body as GitHub sent it; the
  /// gateway hands its `X-GitHub-Event` and `X-Hub-Signature-256` headers on with it.
  pub async fn create_github_deliveries(&self, body: &serde_json::Value) -> Answer<GithubDeliveryTaken> {
    Self::answered(self.request(Method::POST, &["github", "deliveries"]).json(body)).await
  }

  /// `POST /translation-runs`: Translate one text on the translator runner's road, answered whole.
  pub async fn create_translation_runs(&self, body: &TranslationRunAsk) -> Answer<TranslationRun> {
    Self::answered(self.request(Method::POST, &["translation-runs"]).json(body)).await
  }

  /// `GET /jobs/{jobId}`: One job and the tokens its model has spent.
  pub async fn read_jobs_by_job_id(&self, job_id: i64) -> Answer<Job> {
    Self::answered(self.request(Method::GET, &["jobs", &job_id.to_string()])).await
  }

  /// `PATCH /jobs/{jobId}`: End a job with its outcome, or stop it; its assignment is released, and the harness gives the lease back or closes the item in the job service.
  pub async fn change_jobs_by_job_id(&self, job_id: i64, body: &JobChange) -> Answer<Job> {
    Self::answered(self.request(Method::PATCH, &["jobs", &job_id.to_string()]).json(body)).await
  }

  /// `GET /jobs/{jobId}/prompt`: The prompt the job was launched with, with its English where it was translated.
  pub async fn list_jobs_prompt_by_job_id(&self, job_id: i64) -> Answer<JobPrompt> {
    Self::answered(self.request(Method::GET, &["jobs", &job_id.to_string(), "prompt"])).await
  }

  /// `GET /session-work/{sessionId}`: The item and results a worker session holds.
  pub async fn read_session_work_by_session_id(&self, session_id: &str) -> Answer<SessionWork> {
    Self::answered(self.request(Method::GET, &["session-work", session_id])).await
  }

  /// `PATCH /session-work/{sessionId}`: Record the item a session serves.
  pub async fn change_session_work_by_session_id(
    &self,
    session_id: &str,
    body: &SessionWorkChange,
  ) -> Answer<SessionWork> {
    Self::answered(self.request(Method::PATCH, &["session-work", session_id]).json(body)).await
  }

  /// `PUT /session-work/{sessionId}/results/{resultKey}`: Record a result sent into a session.
  pub async fn put_session_work_results_by_session_id_result_key(
    &self,
    session_id: &str,
    result_key: &str,
    body: &Value,
  ) -> Answer<SessionWork> {
    Self::answered(
      self
        .request(Method::PUT, &["session-work", session_id, "results", result_key])
        .json(body),
    )
    .await
  }

  /// `DELETE /session-work/{sessionId}/results`: Forget a session's results as it folds.
  pub async fn delete_session_work_results_by_session_id(&self, session_id: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["session-work", session_id, "results"])).await
  }

  /// `GET /session-loads/{sessionId}`: The documents loaded into a session.
  pub async fn read_session_loads_by_session_id(&self, session_id: &str) -> Answer<SessionLoads> {
    Self::answered(self.request(Method::GET, &["session-loads", session_id])).await
  }

  /// `PUT /session-loads/{sessionId}`: Keep a session's loads.
  pub async fn put_session_loads_by_session_id(
    &self,
    session_id: &str,
    body: &SessionLoadsAsk,
  ) -> Answer<SessionLoads> {
    Self::answered(self.request(Method::PUT, &["session-loads", session_id]).json(body)).await
  }

  /// `DELETE /session-loads/{sessionId}`: Forget a session's loads.
  pub async fn delete_session_loads_by_session_id(&self, session_id: &str) -> Answer<()> {
    Self::nothing(self.request(Method::DELETE, &["session-loads", session_id])).await
  }
}
