//! The session contract's eight operations as routes (docs/api.md, 「Sessions」): start, send a turn, push
//! words into it, read state, interrupt, fold, end and list, each answered by the backend the server stands
//! its places on. A fault is answered with its outcome code and an English sentence saying why.

use agent_session::{
  Aside, Delivery, Fault, FaultKind, RoomBlock, RunParts, Scope, Session, SessionId, Sessions, Start,
  State, Turn, TurnContext,
};
use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::herdr_backend::Shared;

fn backend(call: &Call) -> Result<Shared, Refused> {
  call.sessions().ok_or_else(|| {
    Refused::new(StatusCode::SERVICE_UNAVAILABLE, "agency stands no places to carry sessions on")
  })
}

// A fault as its route answers it: the status its kind stands for, its outcome code and why.
fn faulted(fault: Fault) -> Response {
  let status = match fault.kind {
    FaultKind::Unknown => StatusCode::NOT_FOUND,
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

/// The turn's context a body names, or why it does not read as one.
pub fn context_read(body: &Value) -> Result<TurnContext, Refused> {
  let read: ContextRead = serde_json::from_value(body.clone()).map_err(|why| {
    Refused::new(StatusCode::BAD_REQUEST, format!("the turn's context does not read: {why}"))
  })?;
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
  })
}

fn scope_value(scope: &Scope) -> Value {
  match scope {
    Scope::Agent => json!({ "kind": "agent" }),
    Scope::Room(room) => json!({ "kind": "room", "room": room }),
    Scope::Worker => json!({ "kind": "worker" }),
    Scope::Editor => json!({ "kind": "editor" }),
  }
}

fn session_value(session: &Session) -> Value {
  json!({ "id": session.id.0, "agent": session.agent, "scope": scope_value(&session.scope) })
}

fn state_value(state: &State) -> Value {
  json!({
    "session": state.session.0,
    "provider": state.provider,
    "model": state.model,
    "place": state.place,
    "generation": state.generation,
    "context": state.context.map(|context| json!({ "carried": context.carried, "window": context.window })),
  })
}

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Refused> {
  body
    .get(field)
    .and_then(Value::as_str)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| bad(format!("{field} (string) is required")))
}

fn scope_read(body: &Value) -> Result<Scope, Refused> {
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
    _ => Err(bad("scope.kind is agent, room, worker or editor")),
  }
}

fn ok() -> Response {
  answer(StatusCode::OK, json!({ "ok": true }))
}

/// A session started for an agent at a scope, or the one kept for that scope.
pub struct SessionStart;

impl Route for SessionStart {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/start";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let start = Start {
      agent: text(&body, "agent")?.to_owned(),
      scope: scope_read(&body)?,
    };

    Ok(match backend(&call)?.start(start).await {
      Ok(session) => answer(StatusCode::OK, session_value(&session)),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionStart);

/// One turn sent into a session and carried until it ends.
pub struct SessionTurn;

impl Route for SessionTurn {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/turn";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let turn = Turn {
      session: SessionId(text(&body, "session")?.to_owned()),
      prompt: text(&body, "prompt")?.to_owned(),
      context: context_read(&body)?,
    };

    Ok(match backend(&call)?.send_turn(turn).await {
      Ok(Delivery::Taken) => answer(StatusCode::OK, json!({ "delivery": "taken" })),
      Ok(Delivery::Blocked) => answer(StatusCode::OK, json!({ "delivery": "blocked" })),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionTurn);

/// Words pushed into the turn a session is working, answered with what became of them.
pub struct SessionPush;

impl Route for SessionPush {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/push";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let session = SessionId(text(&body, "session")?.to_owned());
    let words = text(&body, "words")?;

    Ok(match backend(&call)?.push(&session, words).await {
      Ok(said) => answer(StatusCode::OK, json!({ "said": said_word(said) })),
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

/// A session's state, by its id.
pub struct SessionState;

impl Route for SessionState {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session/state";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.query("session").ok_or_else(|| bad("session is required"))?);

    Ok(match backend(&call)?.read_state(&session).await {
      Ok(state) => answer(StatusCode::OK, state_value(&state)),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionState);

/// Which of the three operations on one session a route carries out.
#[derive(Clone, Copy)]
enum Operation {
  Interrupt,
  Fold,
  End,
}

async fn operated(mut call: Call, operation: Operation) -> Result<Response, Refused> {
  let body = call.body().await?;
  let session = SessionId(text(&body, "session")?.to_owned());
  let backend = backend(&call)?;
  let done = match operation {
    Operation::Interrupt => backend.interrupt(&session).await,
    Operation::Fold => backend.fold(&session).await,
    Operation::End => backend.end(&session).await,
  };

  Ok(match done {
    Ok(()) => ok(),
    Err(fault) => faulted(fault),
  })
}

/// The turn a session is in cut short.
pub struct SessionInterrupt;

impl Route for SessionInterrupt {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/interrupt";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    operated(call, Operation::Interrupt).await
  }
}

crate::moved!(SessionInterrupt);

/// A session carried past its window as its provider declares.
pub struct SessionFold;

impl Route for SessionFold {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/fold";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    operated(call, Operation::Fold).await
  }
}

crate::moved!(SessionFold);

/// A session ended and its program taken down.
pub struct SessionEnd;

impl Route for SessionEnd {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/session/end";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    operated(call, Operation::End).await
  }
}

crate::moved!(SessionEnd);

/// Every session the backend keeps.
pub struct SessionList;

impl Route for SessionList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session/list";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    Ok(match backend(&call)?.list().await {
      Ok(sessions) => answer(
        StatusCode::OK,
        json!({ "sessions": sessions.iter().map(session_value).collect::<Vec<_>>() }),
      ),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionList);
