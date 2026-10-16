//! The conversational agents as their rows declare them (docs/agents.md, 「One runner」): the scope of each
//! one's session, the shape of its turns, and which of them answers the rooms that name nobody. Every
//! reader that needs to know how an agent talks, or who answers a room, asks here, so nothing chooses a
//! path by an agent's name or by a kind of its own.

use sea_orm::{ConnectionTrait, QueryResult};

use crate::Refusal;
use store::{all, one};

/// Where a conversational agent's session stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
  /// One session across every room it answers, each turn carrying its room's block.
  Agent,
  /// One session in each room it answers.
  Room,
}

/// What a conversational agent's turn is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
  /// The chat documents as its system prompt, the window's round prompt and the re-asks after an answer.
  Window,
  /// Its settings' pieces as its system prompt, the companion blocks as its turn, and no re-ask.
  Pieces,
}

impl Scope {
  fn named(word: &str) -> Option<Self> {
    match word {
      "agent" => Some(Self::Agent),
      "room" => Some(Self::Room),
      _ => None,
    }
  }

  /// The word its row keeps it under.
  pub fn word(self) -> &'static str {
    match self {
      Self::Agent => "agent",
      Self::Room => "room",
    }
  }
}

impl Shape {
  fn named(word: &str) -> Option<Self> {
    match word {
      "window" => Some(Self::Window),
      "pieces" => Some(Self::Pieces),
      _ => None,
    }
  }

  /// The word its row keeps it under.
  pub fn word(self) -> &'static str {
    match self {
      Self::Window => "window",
      Self::Pieces => "pieces",
    }
  }
}

/// A conversational agent's settings, as its row holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
  pub scope: Scope,
  pub shape: Shape,
  /// Whether it answers every room that names no answerer.
  pub default_answerer: bool,
}

impl Settings {
  /// The settings a row holds, or none where the row is no conversational agent's. A row an older writer
  /// left with no settings is read by the kind it was written under, as the settling read every row
  /// standing when the settings arrived.
  pub fn of(row: &QueryResult) -> Result<Option<Self>, Refusal> {
    let kind: String = row.try_get("", "kind")?;
    let scope: Option<String> = row.try_get("", "session_scope")?;
    let shape: Option<String> = row.try_get("", "turn_shape")?;
    let default_answerer: i64 = row.try_get("", "default_answerer")?;

    Ok(Self::read(&kind, scope.as_deref(), shape.as_deref(), default_answerer))
  }

  /// The settings a row's columns hold, as `of` reads them, for a reader holding the columns themselves.
  pub fn read(kind: &str, scope: Option<&str>, shape: Option<&str>, default_answerer: i64) -> Option<Self> {
    let Some(scope) = scope.and_then(Scope::named) else {
      return unsettled(kind);
    };

    Some(Self {
      scope,
      shape: shape.and_then(Shape::named).unwrap_or(match scope {
        Scope::Agent => Shape::Window,
        Scope::Room => Shape::Pieces,
      }),
      default_answerer: default_answerer != 0,
    })
  }

  /// The settings the console gives a companion it adds, and the seed one it declares.
  pub fn companion_added() -> Self {
    Self {
      scope: Scope::Room,
      shape: Shape::Pieces,
      default_answerer: false,
    }
  }

  /// Whether it is a companion: a conversational agent other than the one answering the unnamed rooms.
  pub fn companion(&self) -> bool {
    !self.default_answerer
  }
}

// The settings of a row an older writer left without them, by the kind it was written under: what the
// settling gave every row of those kinds standing when the settings arrived.
fn unsettled(kind: &str) -> Option<Settings> {
  match kind {
    "chat" => Some(Settings {
      scope: Scope::Agent,
      shape: Shape::Window,
      default_answerer: true,
    }),
    "companion" => Some(Settings::companion_added()),
    _ => None,
  }
}

/// A conversational agent's settings by its name, or none where it has no row or its row is no
/// conversational agent's.
pub async fn settings(db: &impl ConnectionTrait, agent: &str) -> Result<Option<Settings>, Refusal> {
  match one(
    db,
    r#"SELECT "kind", "session_scope", "turn_shape", "default_answerer" FROM "agents" WHERE "name" = $1"#,
    vec![agent.into()],
  )
  .await?
  {
    Some(row) => Settings::of(&row),
    None => Ok(None),
  }
}

/// Whether an agent is a companion: a conversational agent whose row does not make it the default answerer.
pub async fn is_companion(db: &impl ConnectionTrait, agent: &str) -> Result<bool, Refusal> {
  Ok(settings(db, agent).await?.is_some_and(|settings| settings.companion()))
}

/// The agent answering every room that names nobody, by its row. An archive whose rows were never settled
/// names none, and that agent is then `own`, the deployment's own agent as its configuration names it.
pub async fn default_answerer(db: &impl ConnectionTrait, own: &str) -> Result<String, Refusal> {
  let row = one(
    db,
    r#"SELECT "name" FROM "agents"
       WHERE "default_answerer" <> 0 AND "session_scope" IS NOT NULL AND "resting_since" IS NULL
       ORDER BY "name" LIMIT 1"#,
    vec![],
  )
  .await?;

  Ok(match row {
    Some(row) => row.try_get("", "name")?,
    None => own.to_owned(),
  })
}

/// Whether an agent is the default answerer.
pub async fn answers_unnamed(db: &impl ConnectionTrait, own: &str, agent: &str) -> Result<bool, Refusal> {
  Ok(default_answerer(db, own).await? == agent)
}

/// The awake agents a room names among its answerers, by name.
pub async fn named_answerers(db: &impl ConnectionTrait, chat_id: i64) -> Result<Vec<String>, Refusal> {
  all(
    db,
    r#"SELECT "a"."name" FROM "room_answerers" "r" JOIN "agents" "a" ON "a"."name" = "r"."agent"
       WHERE "r"."chat_id" = $1 AND "a"."resting_since" IS NULL ORDER BY "a"."name""#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<String>("", "name").map_err(Refusal::from))
  .collect()
}

/// Who answers in a room: the agents it names, or the default answerer where it names nobody awake.
pub async fn answerers(db: &impl ConnectionTrait, own: &str, chat_id: i64) -> Result<Vec<String>, Refusal> {
  let named = named_answerers(db, chat_id).await?;

  Ok(if named.is_empty() { vec![default_answerer(db, own).await?] } else { named })
}

/// Whether an agent answers in a room: a room that names it among its awake answerers, and for the default
/// answerer every room that names nobody.
pub async fn answers(
  db: &impl ConnectionTrait,
  own: &str,
  agent: &str,
  chat_id: i64,
) -> Result<bool, Refusal> {
  Ok(answerers(db, own, chat_id).await?.iter().any(|name| name == agent))
}

/// The agent that says the bot's own lines in a room: the default answerer
/// where it answers there, and otherwise the first of the room's awake answerers by name.
pub async fn voice_of(db: &impl ConnectionTrait, own: &str, chat_id: i64) -> Result<String, Refusal> {
  let default = default_answerer(db, own).await?;
  let named = named_answerers(db, chat_id).await?;

  Ok(match named.first() {
    Some(first) if !named.contains(&default) => first.clone(),
    _ => default,
  })
}

/// The awake conversational agents whose settings pass a test, by name in order.
pub async fn agents_where(
  db: &impl ConnectionTrait,
  test: impl Fn(&Settings) -> bool,
) -> Result<Vec<String>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "name", "kind", "session_scope", "turn_shape", "default_answerer" FROM "agents"
       WHERE "resting_since" IS NULL ORDER BY "name""#,
    vec![],
  )
  .await?;
  let mut named = Vec::new();

  for row in rows {
    if Settings::of(&row)?.is_some_and(|settings| test(&settings)) {
      named.push(row.try_get("", "name")?);
    }
  }

  Ok(named)
}

/// Whether the default answerer keeps one session across its rooms (the `agent` scope).
pub async fn one_session(db: &impl ConnectionTrait, own: &str) -> Result<bool, Refusal> {
  one_session_of(db, &default_answerer(db, own).await?).await
}

/// Whether an agent keeps one session across its rooms (the `agent` scope).
pub async fn one_session_of(db: &impl ConnectionTrait, agent: &str) -> Result<bool, Refusal> {
  Ok(settings(db, agent).await?.is_some_and(|settings| settings.scope == Scope::Agent))
}
