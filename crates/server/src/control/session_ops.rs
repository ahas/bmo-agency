#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The session contract's ten operations as routes (docs/api.md, 「Sessions」): start, send a turn, push
//! words into it, read state, answer its question, read its output, interrupt, fold, end and list, each
//! answered by the backend the server stands its places on. A fault is answered with its outcome code and
//! an English sentence saying why.

use agent_session::{
  Answer, Apart, Aside, Conversation, Fault, FaultKind, JobMark, RoomBlock, RunParts, Scope, Session,
  SessionId, Sessions, Start, State, Turn, TurnContext, WorkParts,
};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::herdr_backend::Shared;
use crate::lines::Trees;
use crate::lines::standing::session_ttl_ms;
use crate::lines::worker::worker_run;
use crate::places::AgentRow;
use crate::worker_session::held as held_session;
use crate::worker_session::held::SessionRun;
use crate::worker_session::records;

fn backend(call: &Call) -> Result<Shared, Refused> {
  call.sessions().ok_or_else(|| {
    Refused::new(StatusCode::SERVICE_UNAVAILABLE, "agency stands no places to carry sessions on")
  })
}

// A fault as its route answers it: the status its kind stands for, its outcome code and why.
fn faulted(fault: Fault) -> Response {
  let status = match fault.kind {
    FaultKind::Unknown | FaultKind::UnknownTurn => StatusCode::NOT_FOUND,
    FaultKind::Busy => StatusCode::CONFLICT,
    FaultKind::Unreachable => StatusCode::SERVICE_UNAVAILABLE,
    FaultKind::Refused => StatusCode::UNPROCESSABLE_ENTITY,
  };

  answer(status, json!({ "code": fault.kind.code(), "error": fault.message }))
}

// A turn's context as a body names it, every part optional (docs/api.md, 「Sessions」): an editor run's
// parts are read only where the body names the run.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextRead {
  #[serde(default)]
  room: Option<RoomRead>,
  #[serde(default)]
  via: Option<String>,
  #[serde(default)]
  base_persona: Option<String>,
  #[serde(default)]
  privileged: bool,
  #[serde(default)]
  answers: Vec<i64>,
  #[serde(default)]
  began: Option<i64>,
  #[serde(default)]
  run: Option<String>,
  #[serde(default)]
  parent: Option<String>,
  #[serde(default)]
  agent: Option<String>,
  #[serde(default)]
  chat_id: Option<i64>,
  #[serde(default)]
  step: Option<i64>,
  #[serde(default)]
  steps: Option<i64>,
  #[serde(default)]
  workdir: Option<String>,
  #[serde(default)]
  desk: Option<String>,
  #[serde(default)]
  registration: Option<String>,
  #[serde(default)]
  job: Option<i64>,
  #[serde(default)]
  item: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomRead {
  chat_id: i64,
  #[serde(default)]
  text: String,
  #[serde(default)]
  mark: String,
}

/// The turn's context a body names, or why it does not read as one. A work turn's parts are read, whole,
/// where the body names the line its turn is laid on.
pub fn context_read(body: &Value) -> Result<TurnContext, Refused> {
  let read: ContextRead = serde_json::from_value(body.clone()).map_err(|why| {
    Refused::new(StatusCode::BAD_REQUEST, format!("the turn's context does not read: {why}"))
  })?;
  let work = match body.get("mapping") {
    None | Some(Value::Null) => None,
    Some(_) => Some(serde_json::from_value::<WorkParts>(body.clone()).map_err(|why| {
      Refused::new(StatusCode::BAD_REQUEST, format!("the work turn's parts do not read: {why}"))
    })?),
  };
  let named =
    |value: Option<String>| value.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());

  let run = named(read.run).map(|name| RunParts {
    name,
    parent: named(read.parent),
    agent: named(read.agent),
    chat_id: read.chat_id,
    step: read.step,
    steps: read.steps,
    workdir: named(read.workdir),
    desk: named(read.desk),
    registration: named(read.registration),
  });

  Ok(TurnContext {
    room: read.room.map(|room| RoomBlock {
      chat_id: room.chat_id,
      text: room.text,
      mark: room.mark,
    }),
    via: named(read.via),
    base_persona: named(read.base_persona),
    privileged: read.privileged,
    answers: read.answers,
    began: read.began,
    run,
    job: read.job.map(|job| JobMark {
      job,
      item: read.item,
    }),
    work,
  })
}

/// A scope as a start and a list answer it.
pub fn scope_value(scope: &Scope) -> Value {
  match scope {
    Scope::Agent => json!({ "kind": "agent" }),
    Scope::Room(room) => json!({ "kind": "room", "room": room }),
    Scope::Worker => json!({ "kind": "worker" }),
    Scope::Editor => json!({ "kind": "editor" }),
    Scope::Apart(apart) => json!({
      "kind": "apart",
      "provider": apart.provider,
      "model": apart.model,
      "cwd": apart.cwd,
      "todo": apart.todo,
      "role": apart.role,
    }),
    Scope::Conversation(conversation) => json!({
      "kind": "conversation",
      "provider": conversation.provider,
      "model": conversation.model,
      "cwd": conversation.cwd,
    }),
  }
}

fn session_value(session: &Session) -> Value {
  json!({ "id": session.id.0, "agent": session.agent, "scope": scope_value(&session.scope) })
}

/// A session's state as its route answers it.
pub fn state_value(state: &State) -> Value {
  json!({
    "session": state.session.0,
    "provider": state.provider,
    "model": state.model,
    "place": state.place,
    "generation": state.generation,
    "context": state.context.map(|context| json!({ "carried": context.carried, "window": context.window })),
    "run": state.run,
    "went_out": state.went_out,
    "question": state.question,
    "continues": state.continues,
  })
}

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// The turn a path names, or why it names none.
fn turn_asked(call: &Call) -> Result<i64, Refused> {
  call
    .param("turnId")
    .and_then(|id| id.trim().parse::<i64>().ok())
    .ok_or_else(|| bad("turnId (a whole number) is required"))
}

fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Refused> {
  body
    .get(field)
    .and_then(Value::as_str)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| bad(format!("{field} (string) is required")))
}

/// The scope a start's body names, or why it does not read as one.
pub fn scope_read(body: &Value) -> Result<Scope, Refused> {
  let scope = body.get("scope").ok_or_else(|| bad("scope is required"))?;

  match scope.get("kind").and_then(Value::as_str) {
    Some("agent") => Ok(Scope::Agent),
    Some("worker") => Ok(Scope::Worker),
    Some("editor") => Ok(Scope::Editor),
    Some("room") => match scope.get("room") {
      Some(Value::String(room)) if !room.is_empty() => Ok(Scope::Room(room.clone())),
      Some(Value::Number(room)) => Ok(Scope::Room(room.to_string())),
      _ => Err(bad("a room's scope names its room")),
    },
    Some("apart") => {
      let named = |field: &str| text(scope, field).map(|value| value.trim().to_owned());
      let todo = match scope.get("todo") {
        None | Some(Value::Null) => None,
        Some(todo) => Some(todo.as_i64().ok_or_else(|| bad("a thrown-away scope's todo is a number"))?),
      };
      let role = match scope.get("role") {
        None | Some(Value::Null) => None,
        Some(Value::String(role)) => Some(role.trim().to_owned()).filter(|role| !role.is_empty()),
        Some(_) => return Err(bad("a thrown-away scope's role is a string")),
      };

      Ok(Scope::Apart(Apart {
        provider: named("provider")?,
        model: named("model")?,
        cwd: named("cwd")?,
        todo,
        role,
      }))
    }
    Some("conversation") => {
      let named = |field: &str| text(scope, field).map(|value| value.trim().to_owned());

      Ok(Scope::Conversation(Conversation {
        provider: named("provider")?,
        model: named("model")?,
        cwd: scope.get("cwd").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned(),
      }))
    }
    _ => Err(bad("scope.kind is agent, room, worker, editor, apart or conversation")),
  }
}

fn ok() -> Response {
  answer(StatusCode::OK, json!({ "ok": true }))
}

/// The word a session's scope goes by across the boundary.
fn scope_word(scope: &Scope) -> &'static str {
  match scope {
    Scope::Agent => "agent",
    Scope::Room(_) => "room",
    Scope::Worker => "worker",
    Scope::Editor => "editor",
    Scope::Apart(_) => "apart",
    Scope::Conversation(_) => "conversation",
  }
}

fn room_id(scope: &Scope) -> Option<i64> {
  match scope {
    Scope::Room(room) => room.parse().ok(),
    _ => None,
  }
}

/// The scope a start's body names: the scope's word and a room's id, or the older object.
fn scope_asked(body: &Value) -> Result<Scope, Refused> {
  if let Some(scope) = body.get("scope").and_then(Value::as_str) {
    return match scope {
      "agent" => Ok(Scope::Agent),
      "worker" => Ok(Scope::Worker),
      "editor" => Ok(Scope::Editor),
      "room" => {
        let room = body
          .get("roomId")
          .and_then(super::whole)
          .ok_or_else(|| bad("a room's scope names its roomId"))?;
        Ok(Scope::Room(room.to_string()))
      }
      "apart" => {
        let options = body.get("options").cloned().unwrap_or(Value::Null);
        let field = |name: &str| {
          options
            .get(name)
            .or_else(|| body.get(name))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        };
        Ok(Scope::Apart(Apart {
          provider: field("provider").unwrap_or("claude-code").to_owned(),
          model: field("model").ok_or_else(|| bad("a thrown-away scope names its model"))?.to_owned(),
          cwd: field("cwd").map(str::to_owned).unwrap_or_else(|| ".".to_owned()),
          todo: number(&options, "todo").or_else(|| number(body, "todo")),
          role: field("role").map(str::to_owned),
        }))
      }
      "conversation" => {
        let options = body.get("options").cloned().unwrap_or(Value::Null);
        let field = |name: &str| {
          options
            .get(name)
            .or_else(|| body.get(name))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        };
        Ok(Scope::Conversation(Conversation {
          provider: field("provider")
            .ok_or_else(|| bad("a conversation's scope names its provider"))?
            .to_owned(),
          model: field("model").ok_or_else(|| bad("a conversation's scope names its model"))?.to_owned(),
          cwd: field("cwd").map(str::to_owned).unwrap_or_default(),
        }))
      }
      other => {
        Err(bad(format!("scope is agent, room, worker, editor, apart or conversation, not {other}")))
      }
    };
  }

  scope_read(body)
}

fn number(body: &Value, field: &str) -> Option<i64> {
  body
    .get(field)
    .and_then(Value::as_i64)
    .or_else(|| body.get(field).and_then(super::whole))
}

fn agent_named<'a>(body: &'a Value) -> Result<&'a str, Refused> {
  body
    .get("agentName")
    .or_else(|| body.get("agent"))
    .and_then(Value::as_str)
    .filter(|agent| !agent.is_empty())
    .ok_or_else(|| bad("agentName (a string) is required"))
}

/// A session as the boundary answers it: the state its backend holds and what its start named.
fn member(session: &Session, state: Option<&State>, body: &Value) -> Value {
  let scope = &session.scope;
  json!({
    "id": session.id.0,
    "agentName": session.agent,
    "scope": scope_word(scope),
    "roomId": room_id(scope),
    "provider": state.map(|state| state.provider.clone()).unwrap_or_default(),
    "model": state.map(|state| state.model.clone()).unwrap_or_default(),
    "place": state.map(|state| state.place.clone()),
    "generation": state.map(|state| state.generation).unwrap_or(0),
    "holder": Value::Null,
    "prefixChangeReason": Value::Null,
    "boundMs": number(body, "boundMs"),
    "options": body.get("options").cloned().unwrap_or_else(|| json!({})),
    "promptDocumentName": body.get("promptDocumentName").cloned().unwrap_or(Value::Null),
  })
}

/// A session started for an agent at a scope, or the one kept for that scope.
pub struct SessionStart;

impl Route for SessionStart {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/sessions";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let mut start = Start {
      agent: agent_named(&body)?.to_owned(),
      scope: scope_asked(&body)?,
    };

    if let Scope::Conversation(conversation) = &mut start.scope
      && conversation.cwd.trim().is_empty()
    {
      conversation.cwd = call.ground().source.to_string_lossy().into_owned();
    }

    let backend = backend(&call)?;

    match backend.start(start).await {
      Ok(session) => {
        let state = backend.read_state(&session.id, None).await.ok();
        Ok(answer(StatusCode::CREATED, member(&session, state.as_ref(), &body)))
      }
      Err(fault) => Ok(faulted(fault)),
    }
  }
}

crate::moved!(SessionStart);

/// One turn sent into a session and carried until it ends.
pub struct SessionTurn;

impl Route for SessionTurn {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/sessions/{sessionId}/turns";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let session = call.param("sessionId").unwrap_or_default().to_owned();
    let turn = Turn {
      session: SessionId(session.clone()),
      prompt: text(&body, "prompt")?.to_owned(),
      context: context_read(&body)?,
    };

    Ok(match backend(&call)?.send_turn(turn).await {
      Ok(end) => {
        let state = match end.outcome {
          agent_session::Outcome::Answered => "ended",
          agent_session::Outcome::Blocked => "ended",
          agent_session::Outcome::Stopped => "interrupted",
          _ => "ended",
        };
        answer(
          StatusCode::CREATED,
          json!({
            "id": end.turn,
            "sessionId": session,
            "prompt": text(&body, "prompt")?,
            "context": body.get("context").cloned().unwrap_or_else(|| json!({})),
            "state": state,
            "delivery": if end.delivery == agent_session::Delivery::Taken { "delivered" } else { "blocked" },
            "outcome": format!("{:?}", end.outcome).to_lowercase(),
            "answer": end.answer,
            "text": end.answer.as_ref().map(|answer| json!([answer])).unwrap_or_else(|| json!([])),
          }),
        )
      }
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionTurn);

/// Words pushed into the turn a session is working, answered with what became of them.
pub struct SessionPush;

impl Route for SessionPush {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/sessions/{sessionId}/turns/{turnId}/inputs";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());
    let turn = turn_asked(&call)?;
    let words = text(&body, "words")?.to_owned();

    Ok(match backend(&call)?.push(&session, turn, &words).await {
      Ok(said) => answer(StatusCode::CREATED, json!({ "words": words, "said": said_word(said) })),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionPush);

fn said_word(said: Aside) -> &'static str {
  match said {
    Aside::Delivered => "delivered",
    Aside::Waiting => "waiting",
    Aside::Refused => "refused",
  }
}

/// A session's state, by its id; a worker's names what its next turn on the model named would continue.
pub struct SessionState;

impl Route for SessionState {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/sessions/{sessionId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id = call.param("sessionId").unwrap_or_default().to_owned();
    let session = SessionId(id.clone());
    let model = call.query("model").filter(|model| !model.trim().is_empty());
    let backend = backend(&call)?;
    let (agent, scope) = match backend.held_scope(&session).await {
      Ok(held) => held,
      Err(fault) => return Ok(faulted(fault)),
    };

    Ok(match backend.read_state(&session, model.as_deref()).await {
      Ok(state) => {
        let started = agent_session::Session {
          id: SessionId(id.clone()),
          agent,
          scope,
        };
        let (holder, reason) = holder_of(call.store(), &id).await?;

        answer(StatusCode::OK, with_holder(member(&started, Some(&state), &json!({})), holder, reason))
      }
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionState);

// Where a session's holder and the reason its prefix last changed are kept, by the session's id.
const HOLDER: &str = "session_holder:";

/// The holder a session's row names, and why its prefix last changed.
pub(crate) async fn holder_of(
  db: &impl sea_orm::ConnectionTrait,
  session: &str,
) -> Result<(Option<String>, Option<String>), Refused> {
  let Some(text) = crate::queue::settings::get(db, &format!("{HOLDER}{session}")).await? else {
    return Ok((None, None));
  };
  let read: Value = serde_json::from_str(&text).unwrap_or(Value::Null);

  Ok((
    read.get("holder").and_then(Value::as_str).map(str::to_owned),
    read.get("prefixChangeReason").and_then(Value::as_str).map(str::to_owned),
  ))
}

// The holder kept as the body gives it: a name written, a name cleared where null, and the field left
// alone where the body does not name it.
async fn keep_holder(
  db: &impl sea_orm::ConnectionTrait,
  session: &str,
  body: &Value,
) -> Result<(Option<String>, Option<String>), Refused> {
  let (mut holder, mut reason) = holder_of(db, session).await?;

  if let Some(given) = body.get("holder") {
    holder = given.as_str().map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned);
  }

  if let Some(given) = body.get("prefixChangeReason") {
    reason = given.as_str().map(str::trim).filter(|why| !why.is_empty()).map(str::to_owned);
  }

  let key = format!("{HOLDER}{session}");

  if holder.is_none() && reason.is_none() {
    crate::queue::settings::forget(db, &key).await?;
  } else {
    let kept = json!({ "holder": holder, "prefixChangeReason": reason });

    crate::queue::settings::set(db, &key, &kept.to_string()).await?;
  }

  Ok((holder, reason))
}

/// A session moved: a conversation's or a thrown-away session's provider and model switched where the body
/// names them, or a worker's session handed over to the holder the body names, the held session closed
/// with its note for the next turn, its record dropped, and the new holder kept for the session's state.
pub struct SessionHandover;

impl Route for SessionHandover {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/sessions/{sessionId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id = call.param("sessionId").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let session = SessionId(id.clone());
    let backend = backend(&call)?;
    let (agent, scope) = match backend.held_scope(&session).await {
      Ok(held) => held,
      Err(fault) => return Ok(faulted(fault)),
    };

    // A conversation's and a thrown-away session's provider and model move on the change route; a
    // worker's model moves by its hand-over below, and every other scope's stands on its agent's row.
    if !matches!(scope, Scope::Worker) {
      let options = body.get("options").cloned().unwrap_or(Value::Null);
      let given = |name: &str| {
        options
          .get(name)
          .or_else(|| body.get(name))
          .and_then(Value::as_str)
          .map(str::trim)
          .filter(|value| !value.is_empty())
          .map(str::to_owned)
      };
      let (provider, model) = (given("provider"), given("model"));

      if provider.is_none() && model.is_none() {
        return Err(Refused::new(
          StatusCode::UNPROCESSABLE_ENTITY,
          format!("{id} is switched by naming the provider or the model it moves to"),
        ));
      }

      return Ok(match backend.switch(&session, provider.as_deref(), model.as_deref()).await {
        Ok(moved) => {
          let state = backend.read_state(&moved.id, None).await.ok();

          answer(StatusCode::OK, member(&moved, state.as_ref(), &body))
        }
        Err(fault) => faulted(fault),
      });
    }

    let ground = call.ground();
    let db = &ground.db;
    let model = body.get("model").and_then(Value::as_str).map(str::to_owned);
    let state = match backend.read_state(&session, model.as_deref()).await {
      Ok(state) => state,
      // Nothing of the worker's is open to hand over: the holder is kept all the same, since the
      // session is the worker's whether or not its record still stands.
      Err(fault) if fault.kind == FaultKind::Unknown => {
        if !AgentRow::named(db, &agent).await?.is_some_and(|row| row.kind == "worker") {
          return Err(Refused::new(StatusCode::NOT_FOUND, format!("no worker is named {agent}")));
        }

        let (holder, reason) = keep_holder(db, &id, &body).await?;

        return Ok(answer(
          StatusCode::OK,
          with_holder(member(&session_with(&id, &agent), None, &body), holder, reason),
        ));
      }
      Err(fault) => return Ok(faulted(fault)),
    };
    let model = model.filter(|model| !model.trim().is_empty()).unwrap_or(state.model.clone());
    let options = body.get("options").cloned().unwrap_or_else(|| json!({}));
    let given = |key: &str| options.get(key).and_then(Value::as_str).map(str::to_owned);
    let cwd = match given("cwd") {
      Some(cwd) => cwd,
      None => AgentRow::named(db, &agent)
        .await
        .ok()
        .flatten()
        .and_then(|row| row.cwd)
        .unwrap_or_else(|| state.place.clone()),
    };

    // The session named, or with none named the kept session on the model under the login its newest
    // record names.
    let (held, kept_under) = match given("session") {
      Some(session) => (session, None),
      None => {
        let kept = records::sessions(db, &agent)
          .await?
          .into_iter()
          .filter(|held| held.model == model)
          .max_by_key(|held| held.last_reply_at);
        let Some(kept) = kept else {
          let (holder, reason) = keep_holder(db, &id, &body).await?;
          return Ok(answer(
            StatusCode::OK,
            with_holder(member(&session_with(&id, &agent), Some(&state), &body), holder, reason),
          ));
        };
        let ttl = session_ttl_ms(ground, &model).await;
        let Some(session) = records::resumable(db, &agent, &model, &kept.account, ttl).await? else {
          let (holder, reason) = keep_holder(db, &id, &body).await?;
          return Ok(answer(
            StatusCode::OK,
            with_holder(member(&session_with(&id, &agent), Some(&state), &body), holder, reason),
          ));
        };

        (session, Some(kept.account))
      }
    };
    let line = worker_run(ground, &Trees::of(ground), Some(&agent), None, &model, &cwd)
      .await
      .map_err(|why| Refused::failed(why.detail))?;
    let run = SessionRun {
      mode: crate::worker_session::PROVIDER,
      line,
      prompt: String::new(),
      resume: Some(held),
      account: None,
      placement: None,
      thread: given("thread"),
    };

    held_session::hand_over(ground, &agent, &run, given("note").as_deref())
      .await
      .map_err(|why| Refused::failed(why.detail))?;

    if let Some(account) = kept_under {
      records::forget(db, &agent, &model, Some(&account)).await?;
    }

    let (holder, reason) = keep_holder(db, &id, &body).await?;

    Ok(answer(
      StatusCode::OK,
      with_holder(member(&session_with(&id, &agent), Some(&state), &body), holder, reason),
    ))
  }
}

// The session a route read its state for, as the member builder takes it.
fn session_with(id: &str, agent: &str) -> Session {
  Session {
    id: SessionId(id.to_owned()),
    agent: agent.to_owned(),
    scope: Scope::Worker,
  }
}

// A session's member with the holder its row keeps.
fn with_holder(member: Value, holder: Option<String>, reason: Option<String>) -> Value {
  let mut member = member;

  member["holder"] = json!(holder);
  member["prefixChangeReason"] = json!(reason);

  member
}

crate::moved!(SessionHandover);

/// The answer a body gives the question a session's turn stands on, or why it does not read as one.
pub fn answer_read(body: &Value) -> Result<Answer, Refused> {
  let given = body.get("answer").ok_or_else(|| bad("answer is required"))?;

  serde_json::from_value(given.clone())
    .map_err(|why| bad(format!("answer.kind is allow, leave or refuse, with words for a refusal: {why}")))
}

/// The caller's answer to the question a session's turn stands on.
pub struct SessionAnswer;

impl Route for SessionAnswer {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/sessions/{sessionId}/answers";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());
    let question = body
      .get("questionId")
      .or_else(|| body.get("question"))
      .and_then(Value::as_str)
      .ok_or_else(|| bad("questionId (a string) is required"))?
      .to_owned();
    let given = match body.get("answer") {
      Some(Value::String(word)) => match word.as_str() {
        "allow" => Answer::Allow,
        "leave" => Answer::Leave,
        "refuse" => Answer::Refuse { words: None },
        other => return Err(bad(format!("an answer is allow, leave or refuse, not {other}"))),
      },
      Some(_) => answer_read(&body)?,
      None => return Err(bad("answer is required")),
    };

    Ok(match backend(&call)?.answer(&session, &question, given).await {
      Ok(()) => answer(
        StatusCode::CREATED,
        json!({ "questionId": question, "answer": body.get("answer").cloned().unwrap_or(Value::Null) }),
      ),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionAnswer);

/// What a session's turn now going, or its last one, has written so far.
pub struct SessionOutput;

impl Route for SessionOutput {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/sessions/{sessionId}/turns/{turnId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());
    let turn = turn_asked(&call)?;

    Ok(match backend(&call)?.output(&session, Some(turn)).await {
      Ok(output) => answer(
        StatusCode::OK,
        json!({
          "id": output.turn,
          "sessionId": session.0,
          "prompt": "",
          "context": {},
          "state": if output.running { "running" } else { "ended" },
          "delivery": Value::Null,
          "outcome": Value::Null,
          "answer": Value::Null,
          "text": [output.text],
        }),
      ),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionOutput);

/// The turn a session is in cut short.
pub struct SessionInterrupt;

impl Route for SessionInterrupt {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/sessions/{sessionId}/turns/{turnId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked = call.body().await?;
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());
    let turn = turn_asked(&call)?;

    if asked.get("state").and_then(Value::as_str).is_some_and(|state| state != "interrupted") {
      return Err(bad("a turn is interrupted, not moved to another state"));
    }

    Ok(match backend(&call)?.interrupt(&session, turn).await {
      Ok(()) => answer(
        StatusCode::OK,
        json!({
          "id": turn,
          "sessionId": session.0,
          "prompt": "",
          "context": {},
          "state": "interrupted",
          "delivery": Value::Null,
          "outcome": "stopped",
          "answer": Value::Null,
          "text": [],
        }),
      ),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionInterrupt);

/// A session carried past its window as its provider declares.
pub struct SessionFold;

impl Route for SessionFold {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/sessions/{sessionId}/folds";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());

    Ok(match backend(&call)?.fold(&session).await {
      Ok(()) => answer(StatusCode::CREATED, json!({ "sessionId": session.0, "at": crate::now_ms() })),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionFold);

/// A session ended and its program taken down.
pub struct SessionEnd;

impl Route for SessionEnd {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/sessions/{sessionId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.param("sessionId").unwrap_or_default().to_owned());

    Ok(match backend(&call)?.end(&session).await {
      Ok(()) => StatusCode::NO_CONTENT.into_response(),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionEnd);
