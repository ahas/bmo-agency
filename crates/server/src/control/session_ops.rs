//! The session contract's ten operations as routes (docs/api.md, 「Sessions」): start, send a turn, push
//! words into it, read state, answer its question, read its output, interrupt, fold, end and list, each
//! answered by the backend the server stands its places on. A fault is answered with its outcome code and
//! an English sentence saying why.

use agent_session::{
  Answer, Apart, Aside, Fault, FaultKind, JobMark, RoomBlock, RunParts, Scope, Session, SessionId,
  Sessions, Start, State, Turn, TurnContext, WorkParts,
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
    _ => Err(bad("scope.kind is agent, room, worker, editor or apart")),
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
      Ok(end) => answer(StatusCode::OK, json!(end)),
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

/// A session's state, by its id; a worker's names what its next turn on the model named would continue.
pub struct SessionState;

impl Route for SessionState {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session/state";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.query("session").ok_or_else(|| bad("session is required"))?);
    let model = call.query("model").filter(|model| !model.trim().is_empty());

    Ok(match backend(&call)?.read_state(&session, model.as_deref()).await {
      Ok(state) => answer(StatusCode::OK, state_value(&state)),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionState);

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
  const PATH: &'static str = "/session/answer";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let session = SessionId(text(&body, "session")?.to_owned());
    let question = text(&body, "question")?;
    let given = answer_read(&body)?;

    Ok(match backend(&call)?.answer(&session, question, given).await {
      Ok(()) => ok(),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionAnswer);

/// What a session's turn now going, or its last one, has written so far.
pub struct SessionOutput;

impl Route for SessionOutput {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session/output";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = SessionId(call.query("session").ok_or_else(|| bad("session is required"))?);

    Ok(match backend(&call)?.output(&session).await {
      Ok(output) => answer(StatusCode::OK, json!(output)),
      Err(fault) => faulted(fault),
    })
  }
}

crate::moved!(SessionOutput);

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
