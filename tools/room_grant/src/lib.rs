//! `room_grant`: the grants standing in the room a turn answers, read, made and taken back on the account
//! service (docs/tools.md, 「The room grants」). A grant or a revocation is made on a message the turn answers, so
//! the service judges the granter by that message's sender and nobody gives what they do not hold; a refusal is
//! answered in the service's own words, and what was done is read back from the record the room's GRANTS IN FORCE
//! are composed from before it is answered.

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tool_call::room_gate::{AccountService, RoomTurn, room_turn_of};
use tool_call::{Answer, Asked, Handed, Session};

/// The entry the manifest declares.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "room_grant";
pub const TITLE: &str = "Room grants";
pub const DESCRIPTION: &str = "List, make or take back what the owner allows a person or the members of the room this turn answers: the record its GRANTS IN FORCE are read from. A grant or a revocation is made on the owner's message of this turn and read back from the record; it is in force only once the row answered shows it. Where the account service refuses, its words are answered as it gave them.";
pub const CAPABILITIES: &[&str] = &["room.tools"];

/// The relation a standing rule is: the owner's words for one person, which a grant of it must carry.
pub const STANDING_RULE: &str = "rule";

pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

/// What a call does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
  List,
  Grant,
  Revoke,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantInput {
  pub action: Action,
  #[serde(default)]
  pub relation: Option<String>,
  #[serde(default)]
  pub sender: Option<i64>,
  #[serde(default)]
  pub members: Option<bool>,
  #[serde(default)]
  pub words: Option<String>,
  /// When the grant lapses, as an RFC 3339 time.
  #[serde(default)]
  pub until: Option<String>,
  #[serde(default)]
  pub message: Option<i64>,
}

/// Whom a grant is to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
  Sender(i64),
  Members,
}

impl Holder {
  fn wire(self) -> Value {
    match self {
      Self::Sender(sender) => json!({ "sender": sender, "members": false }),
      Self::Members => json!({ "members": true }),
    }
  }
}

/// One grant or revocation as asked, judged before the account service is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
  pub relation: String,
  pub holder: Holder,
  pub words: Option<String>,
  /// Epoch milliseconds.
  pub expires_at: Option<i64>,
}

/// The message a grant or a revocation is made on: one the turn answers, the one named or the turn's own where it
/// answers one, or its newest where the receiver marked every message of the turn the owner's.
pub fn message_of(turn: &RoomTurn, named: Option<i64>) -> Result<i64, String> {
  let listed = || turn.messages.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");

  match named {
    Some(message) if turn.messages.contains(&message) => Ok(message),
    Some(message) if turn.messages.is_empty() => Err(format!(
      "message {message} is not one this turn answers, and this turn answers no message: a grant is made on the \
       owner's message of the turn"
    )),
    Some(message) => Err(format!(
      "message {message} is not one this turn answers ({}): a grant is made on the owner's message of the turn",
      listed()
    )),
    None => match turn.messages.as_slice() {
      [] => Err(
        "this turn answers no message, so no grant can be made on one: a grant is made on the owner's message of \
         the turn"
          .to_owned(),
      ),
      [only] => Ok(*only),
      _ if turn.privileged => Ok(turn.messages.iter().copied().max().unwrap_or_default()),
      _ => Err(format!(
        "this turn answers several messages ({}), not all the owner's: name the owner's one as message",
        listed()
      )),
    },
  }
}

/// A grant or a revocation as the call asks it, or why it cannot be asked: a relation, one holder, the owner's words
/// for a standing rule, and a lapse that reads as a time; a revocation takes no words and no lapse.
pub fn change_of(input: &GrantInput) -> Result<Change, String> {
  let relation = input.relation.as_deref().map(str::trim).filter(|relation| !relation.is_empty());
  let Some(relation) = relation else {
    return Err("name the relation to grant or take back".to_owned());
  };
  let holder = match (input.sender, input.members == Some(true)) {
    (Some(sender), false) => Holder::Sender(sender),
    (None, true) => Holder::Members,
    (Some(_), true) => return Err("name the holder once: a sender, or the room's members".to_owned()),
    (None, false) => return Err("name the holder: a sender by their id, or members".to_owned()),
  };
  let words = input
    .words
    .as_deref()
    .map(str::trim)
    .filter(|words| !words.is_empty())
    .map(str::to_owned);

  if input.action == Action::Revoke {
    if words.is_some() || input.until.is_some() {
      return Err("a revocation takes no words and no lapse".to_owned());
    }

    return Ok(Change {
      relation: relation.to_owned(),
      holder,
      words: None,
      expires_at: None,
    });
  }

  if relation == STANDING_RULE && words.is_none() {
    return Err(
      "a standing rule carries the owner's words and the scope he gave: name them as words".to_owned(),
    );
  }

  let expires_at = match input.until.as_deref().map(str::trim).filter(|until| !until.is_empty()) {
    Some(until) => Some(
      chrono::DateTime::parse_from_rfc3339(until)
        .map_err(|why| format!("until does not read as an RFC 3339 time ({until}): {why}"))?
        .timestamp_millis(),
    ),
    None => None,
  };

  Ok(Change {
    relation: relation.to_owned(),
    holder,
    words,
    expires_at,
  })
}

/// The account service as the tool asks it: every call waited on until it answers.
pub struct Accounts<'a> {
  pub service: &'a AccountService,
}

impl Accounts<'_> {
  async fn asked(&self, request: reqwest::RequestBuilder, what: &str) -> Result<Value, String> {
    if self.service.address.is_empty() {
      return Err("no account service is named (WEB_ADDRESS)".to_owned());
    }

    let answered = request
      .bearer_auth(&self.service.token)
      .send()
      .await
      .map_err(|why| format!("the account service was not reached for {what}: {why}"))?;
    let status = answered.status();
    let said = answered.json::<Value>().await.unwrap_or(Value::Null);

    if status.is_success() {
      return Ok(said);
    }

    let code = said.get("code").and_then(Value::as_str).unwrap_or("no code");
    let why = said.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    Err(format!("the account service refused {what} ({} {code}): {why}", status.as_u16()))
  }

  fn at(&self, path: &str) -> String {
    format!("{}{path}", self.service.address)
  }

  /// The grants standing in a room, as the service answers them.
  pub async fn standing(&self, room: i64) -> Result<Vec<Value>, String> {
    let request = reqwest::Client::new().get(self.at("/grants")).query(&[("room", room.to_string())]);
    let said = self.asked(request, "the room's grants").await?;

    Ok(said.get("grants").and_then(Value::as_array).cloned().unwrap_or_default())
  }

  // The senders linked to a user, none where they cannot be read.
  async fn senders_of(&self, user: &str) -> Option<Value> {
    let request = reqwest::Client::new().get(self.at(&format!("/users/{user}/senders")));

    self.asked(request, "a user's senders").await.ok()?.get("senders").cloned()
  }

  // The subject key a holder stands under in the room's grants: the room's members', or the sender's user's, none
  // where the sender is linked to no user.
  async fn subject_of(&self, room: i64, holder: Holder) -> Option<String> {
    match holder {
      Holder::Members => Some(format!("bmo_member:{room}")),
      Holder::Sender(sender) => {
        let request = reqwest::Client::new().get(self.at(&format!("/senders/{sender}/user")));
        let user = self.asked(request, "a sender's user").await.ok()?;

        user.get("user").and_then(Value::as_str).map(|user| format!("user:{user}"))
      }
    }
  }

  async fn changed(&self, method: reqwest::Method, body: &Value, what: &str) -> Result<Value, String> {
    self
      .asked(reqwest::Client::new().request(method, self.at("/grants")).json(body), what)
      .await
  }
}

/// The room's grants as the call answers them: each row as the service keeps it, with the sender ids linked to a
/// user who holds one, so a holder is matched as the room's GRANTS IN FORCE match them.
pub async fn rows(accounts: &Accounts<'_>, room: i64) -> Result<Vec<Value>, String> {
  let mut rows = accounts.standing(room).await?;

  for row in &mut rows {
    let user = row.get("subjectKey").and_then(Value::as_str).and_then(|key| key.strip_prefix("user:"));

    if let Some(user) = user.map(str::to_owned)
      && let Some(senders) = accounts.senders_of(&user).await
      && let Some(row) = row.as_object_mut()
    {
      row.insert("senders".to_owned(), senders);
    }
  }

  Ok(rows)
}

fn written(value: &Value) -> String {
  serde_json::to_string_pretty(value).unwrap_or_default()
}

// The body of a grant or a revocation on the room, made on the message.
fn body(room: i64, message: i64, change: &Change) -> Value {
  let mut body = Map::new();

  body.insert("room".to_owned(), json!(room));
  body.insert("relation".to_owned(), json!(change.relation));
  body.insert("to".to_owned(), change.holder.wire());
  if let Some(words) = &change.words {
    body.insert("words".to_owned(), json!(words));
  }
  if let Some(at) = change.expires_at {
    body.insert("expiresAt".to_owned(), json!(at));
  }
  body.insert("message".to_owned(), json!({ "room": room, "id": message }));

  Value::Object(body)
}

fn holds(row: &Value, relation: &str, subject: &str) -> bool {
  row.get("relation").and_then(Value::as_str) == Some(relation)
    && row.get("subjectKey").and_then(Value::as_str) == Some(subject)
}

/// One call, against the account service named, in the room turn named.
pub async fn act(service: &AccountService, turn: Option<&RoomTurn>, input: GrantInput) -> Answer {
  let Some(turn) = turn else {
    return Answer::failure(
      "this call stands in no room's turn: room_grant acts on the room the turn answers, and on nothing else",
    );
  };
  let accounts = Accounts { service };
  let room = turn.chat;

  if input.action == Action::List {
    return match rows(&accounts, room).await {
      Ok(rows) => Answer::text(written(&json!({ "room": room, "grants": rows }))),
      Err(why) => Answer::failure(why),
    };
  }

  let change = match change_of(&input) {
    Ok(change) => change,
    Err(why) => return Answer::failure(why),
  };
  let message = match message_of(turn, input.message) {
    Ok(message) => message,
    Err(why) => return Answer::failure(why),
  };
  let body = body(room, message, &change);

  match input.action {
    Action::Grant => granted(&accounts, room, &change, &body).await,
    _ => revoked(&accounts, room, &change, &body).await,
  }
}

// A grant made and read back: the row standing for its relation and holder, or a failure where none stands.
async fn granted(accounts: &Accounts<'_>, room: i64, change: &Change, body: &Value) -> Answer {
  let made = match accounts.changed(reqwest::Method::POST, body, "the grant").await {
    Ok(made) => made,
    Err(why) => return Answer::failure(why),
  };
  let Some(subject) = made.get("subject").and_then(Value::as_str).map(str::to_owned) else {
    return Answer::failure(format!(
      "the account service answered the grant with no holder, so it was not read back and is not known to be in \
       force: {}",
      written(&made)
    ));
  };
  let rows = match rows(accounts, room).await {
    Ok(rows) => rows,
    Err(why) => {
      return Answer::failure(format!(
        "the grant was made but not read back, so it is not known to be in force: {why}"
      ));
    }
  };

  match rows.iter().find(|row| holds(row, &change.relation, &subject)) {
    Some(row) => Answer::text(written(&json!({ "room": room, "granted": made, "standing": row }))),
    None => Answer::failure(format!(
      "the account service answered the grant made, but the room's grants do not carry {} for {subject}, so it is \
       not in force: {}",
      change.relation,
      written(&json!({ "granted": made, "grants": rows }))
    )),
  }
}

// A grant taken back and read back: the room's grants with none left for its relation and holder, or a failure
// where nothing was taken back or one still stands.
async fn revoked(accounts: &Accounts<'_>, room: i64, change: &Change, body: &Value) -> Answer {
  let taken = match accounts.changed(reqwest::Method::DELETE, body, "the revocation").await {
    Ok(taken) => taken,
    Err(why) => return Answer::failure(why),
  };
  let subject = accounts.subject_of(room, change.holder).await;
  let rows = match rows(accounts, room).await {
    Ok(rows) => rows,
    Err(why) => {
      return Answer::failure(format!(
        "the revocation was made but not read back, so it is not known to be gone: {why}"
      ));
    }
  };
  let still = subject
    .as_deref()
    .is_some_and(|subject| rows.iter().any(|row| holds(row, &change.relation, subject)));
  let none_went = taken.get("revoked").and_then(Value::as_i64) == Some(0);

  if still {
    return Answer::failure(format!(
      "the room's grants still carry {} for that holder, so it is not taken back: {}",
      change.relation,
      written(&json!({ "revoked": taken, "grants": rows }))
    ));
  }

  if none_went {
    return Answer::failure(format!(
      "no grant of {} stood for that holder in this room, so nothing was taken back: {}",
      change.relation,
      written(&json!({ "revoked": taken, "grants": rows }))
    ));
  }

  Answer::text(written(&json!({ "room": room, "revoked": taken, "grants": rows })))
}

/// One call, in the room turn the session's record names, against the account service the data root names.
pub async fn call(session: &Session, arguments: Value) -> Result<Answer, String> {
  let input: GrantInput =
    serde_json::from_value(arguments).map_err(|why| format!("invalid arguments: {NAME}: {why}"))?;
  let service = AccountService::read(&session.data_root(), |name| std::env::var(name).ok());

  Ok(act(&service, room_turn_of(session).as_ref(), input).await)
}

pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => call(&handed.session, arguments).await.unwrap_or_else(Answer::failure),
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}
