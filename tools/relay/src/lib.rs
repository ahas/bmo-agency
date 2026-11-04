//! `relay`: a run hands a line to another person's BMO, or opens a chat into that person's room
//! (docs/relays.md). The sender is the agent the calling turn belongs to, so a relay is always a BMO's
//! word and never the person's: a call standing in no agent's turn is refused.

use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::{Answer, Asked, Handed, Session, Turn};

/// The tool's namespace and entries.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "relay";
pub const CHAT_NAME: &str = "chat";
pub const TITLE: &str = "Relay to another person's BMO";
pub const DESCRIPTION: &str = "Hand a line to another person's BMO, recorded for both sides: the receiving BMO answers it as another agent's word, never its person's, and may refuse it. The line is said in the room the call names, or the turn's own room where none is named.";
pub const CHAT_TITLE: &str = "Open a chat with another person";
pub const CHAT_DESCRIPTION: &str = "Open a chat into another person's room and hand it a line, answered with the receiving BMO's answer; a refusal is reported to you. The room must be named.";
pub const CAPABILITIES: &[&str] = &["room.tools"];

const DEFAULT_CONTROL_PORT: u16 = 3220;

pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

pub fn chat_entry() -> String {
  format!("{NAMESPACE}/{CHAT_NAME}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayInput {
  /// The receiving BMO's name.
  pub to: String,
  pub text: String,
  /// The room the line is said in; the turn's own room where none is named.
  #[serde(default)]
  pub room: Option<i64>,
  /// The receiving person's account id, where the sender knows it.
  #[serde(default)]
  pub person: Option<i64>,
}

/// The agency server a tool reaches: its port and token, from its own environment or the server's env file.
pub struct Api {
  pub port: u16,
  pub token: Option<String>,
}

impl Api {
  pub fn from_record(session: &Session) -> Self {
    let root = session.data_root();
    let named = |name: &str| {
      std::env::var(name).ok().or_else(|| {
        let text = std::fs::read_to_string(root.join("env").join("agency.env")).ok()?;
        text.lines().find_map(|line| {
          let (key, value) = line.split_once('=')?;
          (key.trim() == name).then(|| value.trim().to_owned())
        })
      })
    };

    Self {
      port: named("AGENCY_CONTROL_PORT")
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_CONTROL_PORT),
      token: named("AGENCY_CONTROL_TOKEN").filter(|token| !token.is_empty()).or_else(|| {
        named("AGENCY_CONTROL_TOKEN_FILE")
          .and_then(|file| std::fs::read_to_string(file).ok())
          .map(|token| token.trim().to_owned())
          .filter(|token| !token.is_empty())
      }),
    }
  }

  // A call answered once agency answers, however long the receiving BMO's turn takes.
  async fn post(&self, path: &str, body: Value) -> Result<(reqwest::StatusCode, Value), String> {
    let mut request =
      reqwest::Client::new().post(format!("http://127.0.0.1:{}{path}", self.port)).json(&body);

    if let Some(token) = &self.token {
      request = request.bearer_auth(token);
    }

    let answered =
      request.send().await.map_err(|why| format!("agency could not be asked {path}: {why}"))?;
    let status = answered.status();

    Ok((status, answered.json().await.unwrap_or(Value::Null)))
  }
}

/// The sender of a call, or the refusal saying the call stands in no agent's turn.
fn sender(session: &Session) -> Result<(String, Option<i64>), String> {
  let turn = Turn::of(session);
  let agent = turn
    .agent
    .filter(|agent| !agent.trim().is_empty())
    .ok_or_else(|| "a relay is a BMO's word, and this call stands in no agent's turn".to_owned())?;

  Ok((agent, turn.chat))
}

fn text(input: &RelayInput) -> Result<&str, String> {
  let text = input.text.trim();

  if text.is_empty() {
    return Err("text must not be empty".to_owned());
  }

  Ok(text)
}

fn named(input: &RelayInput) -> Result<&str, String> {
  let to = input.to.trim();

  if to.is_empty() {
    return Err("to names the other BMO".to_owned());
  }

  Ok(to)
}

fn body(from: &str, to: &str, said: &str, input: &RelayInput, room: i64) -> Value {
  json!({
    "from": from,
    "to": to,
    "text": said,
    "room": room,
    "person": input.person,
  })
}

/// The words a delivered relay answers with.
fn delivered(answer: &Value) -> String {
  let relay = &answer["relay"];
  let to = relay["to"].as_str().unwrap_or("the other BMO");
  let id = relay["id"].as_i64().unwrap_or_default();

  format!("Relayed to {to}; record {id}.")
}

/// The answer of a chat, or the record it opened where the BMO said nothing.
fn opened(answer: &Value) -> String {
  let relay = &answer["relay"];
  let to = relay["to"].as_str().unwrap_or("the other BMO");
  let id = relay["id"].as_i64().unwrap_or_default();
  let said = answer["answer"].as_str().map(str::trim).filter(|said| !said.is_empty());

  match said {
    Some(said) => format!("{said}\n\n(chat with {to}, record {id})"),
    None => format!("The chat with {to} stands; record {id}."),
  }
}

pub async fn send_with(
  api: &Api,
  session: &Session,
  input: RelayInput,
  chat: bool,
) -> Result<Answer, String> {
  let (from, own_room) = match sender(session) {
    Ok(sender) => sender,
    Err(why) => return Ok(Answer::failure(why)),
  };
  let to = match named(&input) {
    Ok(to) => to,
    Err(why) => return Ok(Answer::failure(why)),
  };
  let said = match text(&input) {
    Ok(said) => said,
    Err(why) => return Ok(Answer::failure(why)),
  };
  let room = match (chat, input.room.or(own_room)) {
    (true, None) => {
      return Ok(Answer::failure("room names the person's room the chat is opened into"));
    }
    (false, None) => {
      return Ok(Answer::failure("room names the room the line is said in, and this turn names none"));
    }
    (_, Some(room)) => room,
  };
  let path = if chat { "/chats" } else { "/relays" };
  let (status, answer) = api.post(path, body(&from, to, said, &input, room)).await?;

  if !status.is_success() {
    let why = answer["error"]
      .as_str()
      .filter(|why| !why.is_empty())
      .map(str::to_owned)
      .unwrap_or_else(|| format!("agency answered {}", status.as_u16()));

    return Ok(Answer::failure(why));
  }

  Ok(Answer::text(if chat { opened(&answer) } else { delivered(&answer) }))
}

async fn send(session: &Session, input: RelayInput, chat: bool) -> Result<Answer, String> {
  let api = Api::from_record(session);

  send_with(&api, session, input, chat).await
}

/// The tool's entry point: one call, answered.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => {
      let input: RelayInput =
        serde_json::from_value(arguments).map_err(|why| format!("the call does not read: {why}"))?;

      send(&handed.session, input, false).await.unwrap_or_else(Answer::failure)
    }
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == chat_entry() => {
      let input: RelayInput =
        serde_json::from_value(arguments).map_err(|why| format!("the call does not read: {why}"))?;

      send(&handed.session, input, true).await.unwrap_or_else(Answer::failure)
    }
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}
