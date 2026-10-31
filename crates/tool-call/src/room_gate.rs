//! The gate in front of what touches the machine or spends money for a room's words (docs/launch-lines.md,
//! 「The owner's gate」): a turn whose messages are not all the owner's goes on only where the account service
//! answers that every message's sender holds the action's relation in the room, and is refused, with a sentence
//! saying why, where it answers otherwise or cannot be asked. A turn no room message opened — a person's typed
//! words, a job, a schedule — and a turn the receiver marked the owner's are never asked about.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

/// The variables naming the account service's base address and its bearer token, and the env file under the data
/// root the web unit writes them into.
pub const ADDRESS_VARIABLE: &str = "WEB_ADDRESS";
pub const TOKEN_VARIABLE: &str = "WEB_TOKEN";
pub const ENV_FILE: &str = "env/web.env";

/// The room relation launching a worker or a job asks, the one filing an item asks, and the one a deploy asks.
pub const LAUNCH: &str = "launch";
pub const FILE_TODO: &str = "file_todo";
pub const DEPLOY: &str = "deploy";

// How long the account service is given to answer one check.
const ANSWER_WITHIN: Duration = Duration::from_secs(10);

/// The room turn an action is asked in, as the caller read it: the room, the messages it answers, and whether
/// the receiver marked every one of them the owner's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomTurn {
  pub chat: i64,
  pub messages: Vec<i64>,
  pub privileged: bool,
}

/// The account service as a caller reaches it: its base address and its token.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountService {
  pub address: String,
  pub token: String,
}

impl AccountService {
  /// The account service a data root names: each variable `variable` answers standing first, else the web unit's
  /// env file; the token by value or in the file `WEB_TOKEN_FILE` names.
  pub fn read(data_root: &Path, variable: impl Fn(&str) -> Option<String>) -> Self {
    let file = env_file(&data_root.join(ENV_FILE));
    let named = |name: &str| {
      variable(name)
        .or_else(|| file.get(name).cloned())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };
    let token = named(TOKEN_VARIABLE).or_else(|| {
      named(&format!("{TOKEN_VARIABLE}_FILE"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
    });

    Self {
      address: named(ADDRESS_VARIABLE).unwrap_or_default().trim_end_matches('/').to_owned(),
      token: token.unwrap_or_default(),
    }
  }

  /// Whether one message's sender holds a relation in the room the message stands in, or why it was not answered.
  pub async fn message_holds(&self, room: i64, message: i64, relation: &str) -> Result<bool, String> {
    if self.address.is_empty() {
      return Err(format!("no account service is named ({ADDRESS_VARIABLE})"));
    }

    let answer = reqwest::Client::new()
      .post(format!("{}/checks/message", self.address))
      .bearer_auth(&self.token)
      .json(&json!({ "room": room, "message": message, "relation": relation }))
      .timeout(ANSWER_WITHIN)
      .send()
      .await
      .map_err(|why| format!("the account service was not reached: {why}"))?;
    let status = answer.status();
    let read = answer.json::<Value>().await.unwrap_or(Value::Null);

    if !status.is_success() {
      let said = read.get("error").and_then(Value::as_str).unwrap_or("no reason given");

      return Err(format!("the account service answered {}: {said}", status.as_u16()));
    }

    read
      .get("allowed")
      .and_then(Value::as_bool)
      .ok_or_else(|| "the account service's answer names no `allowed`".to_owned())
  }
}

/// Whether an action goes on for the turn it is asked in: nothing is asked for no room turn or the owner's own;
/// otherwise every message the turn answers is checked for `relation`. The refusal is the sentence the caller
/// answers with.
pub async fn gate(
  service: &AccountService,
  turn: Option<&RoomTurn>,
  relation: &str,
  action: &str,
) -> Result<(), String> {
  let Some(turn) = turn.filter(|turn| !turn.privileged) else {
    return Ok(());
  };

  if turn.messages.is_empty() {
    return Err(format!(
      "{action} is refused: this turn is not the owner's, and it names no message whose sender could be checked"
    ));
  }

  for &message in &turn.messages {
    match service.message_holds(turn.chat, message, relation).await {
      Ok(true) => {}
      Ok(false) => {
        return Err(format!(
          "{action} is refused: message {message} in this room is not the owner's, and its sender holds no \
           `{relation}` grant here"
        ));
      }
      Err(why) => {
        return Err(format!(
          "{action} is refused: this turn is not the owner's, and whether its sender may ask it could not be \
           checked ({why})"
        ));
      }
    }
  }

  Ok(())
}

/// The room turn a tool call's session is made in, as its place's run names it: none for a turn that answers no
/// room.
pub fn room_turn_of(session: &crate::Session) -> Option<RoomTurn> {
  room_turn_from(
    session.turn_variable(crate::CHAT_VARIABLE),
    session.turn_variable(MESSAGES_VARIABLE),
    session.turn_variable(PRIVILEGED_VARIABLE),
  )
}

/// The room turn a resident program's place names as it carries one now: its run's variables, none where the
/// run is a person's typed words or answers no room.
pub fn room_turn_of_place(place: &Path) -> Option<RoomTurn> {
  let run: Value = serde_json::from_str(&std::fs::read_to_string(place.join("run.json")).ok()?).ok()?;

  if run.get("person").and_then(Value::as_bool) == Some(true) {
    return None;
  }

  let named = |name: &str| run.get("env")?.get(name)?.as_str().map(str::to_owned);

  room_turn_from(named(crate::CHAT_VARIABLE), named(MESSAGES_VARIABLE), named(PRIVILEGED_VARIABLE))
}

// A room turn out of its three variables as they are written.
fn room_turn_from(
  chat: Option<String>,
  messages: Option<String>,
  privileged: Option<String>,
) -> Option<RoomTurn> {
  Some(RoomTurn {
    chat: chat?.trim().parse().ok()?,
    messages: messages
      .unwrap_or_default()
      .split(',')
      .filter_map(|id| id.trim().parse().ok())
      .collect(),
    privileged: privileged.is_some_and(|flag| flag.trim() == "1"),
  })
}

/// The variables a room turn names the messages it answers in, and whether the receiver marked them the owner's.
pub const MESSAGES_VARIABLE: &str = "BMO_TURN_MESSAGES";
pub const PRIVILEGED_VARIABLE: &str = "BMO_TURN_PRIVILEGED";

// An env file's `NAME=value` lines.
fn env_file(path: &Path) -> BTreeMap<String, String> {
  std::fs::read_to_string(path)
    .unwrap_or_default()
    .lines()
    .filter(|line| !line.trim_start().starts_with('#'))
    .filter_map(|line| {
      let (name, value) = line.split_once('=')?;

      Some((name.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
    })
    .collect()
}
