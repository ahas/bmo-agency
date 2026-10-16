//! A designated agent's session in each room it answers (docs/companions.md, 「The session」): the room
//! sessions, the turns kept on each generation, the agents' windows in the rooms and the resets asked of a
//! room's session, as the TypeScript side's room session store reads and writes them.

use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, TransactionTrait};

use store::{all, one, run};

use super::guard::Spent;
use crate::lines::fault;
use crate::{Refusal, now_ms};

/// The harness session a room's turns go on in, as the turn that last answered left it: its id, the provider
/// and model it was opened on as `<provider>::<model>`, and the place it stands on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Harness {
  pub id: String,
  pub road: String,
  pub placement: Option<String>,
}

/// What one agent's session in one room stands at.
#[derive(Clone, Debug)]
pub struct RoomSession {
  pub agent: String,
  pub chat_id: i64,
  pub generation: i64,
  pub harness: Option<Harness>,
  pub last_message_id: Option<i64>,
  pub system_hash: Option<String>,
  /// The note the generation before wrote as it closed at the handoff mark.
  pub handoff: Option<String>,
  pub spent: Spent,
}

/// The provider and model a harness session was opened on, as its road holds them.
pub fn harness_road(provider: &str, model: &str) -> String {
  format!("{provider}::{model}")
}

fn session_of(row: &QueryResult) -> Result<RoomSession, Refusal> {
  let id: Option<String> = row.try_get("", "harness_session")?;
  let road: Option<String> = row.try_get("", "harness_road")?;

  Ok(RoomSession {
    agent: row.try_get("", "agent")?,
    chat_id: row.try_get("", "chat_id")?,
    generation: row.try_get("", "generation")?,
    harness: match (id, road) {
      (Some(id), Some(road)) => Some(Harness {
        id,
        road,
        placement: row.try_get("", "placement")?,
      }),
      _ => None,
    },
    last_message_id: row.try_get("", "last_message_id")?,
    system_hash: row.try_get("", "system_hash")?,
    handoff: row.try_get("", "handoff_note")?,
    spent: Spent {
      calls: row.try_get("", "spent_calls")?,
      ms: row.try_get("", "spent_ms")?,
    },
  })
}

fn now() -> i64 {
  now_ms() / 1000
}

/// The agent's session in the room, or none where it has never had one.
pub async fn session(
  db: &impl ConnectionTrait,
  agent: &str,
  chat_id: i64,
) -> Result<Option<RoomSession>, Refusal> {
  one(
    db,
    "SELECT * FROM room_sessions WHERE agent = $1 AND chat_id = $2",
    vec![agent.into(), chat_id.into()],
  )
  .await?
  .as_ref()
  .map(session_of)
  .transpose()
}

/// Every session an agent holds, room by room.
pub async fn sessions_of(db: &impl ConnectionTrait, agent: &str) -> Result<Vec<RoomSession>, Refusal> {
  all(db, "SELECT * FROM room_sessions WHERE agent = $1 ORDER BY chat_id", vec![agent.into()])
    .await?
    .iter()
    .map(session_of)
    .collect()
}

/// A new generation of the agent's session in the room: the first where there was none, the next where its
/// turns go on in a fresh harness session. It holds no harness session until its first turn answers; the
/// last message taken stays, since opening takes nothing new.
pub async fn open_generation(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
) -> Result<RoomSession, Refusal> {
  let row = one(
    db,
    "INSERT INTO room_sessions (agent, chat_id, generation, opened_at) VALUES ($1, $2, 1, $3)
     ON CONFLICT (agent, chat_id) DO UPDATE SET generation = room_sessions.generation + 1, opened_at = excluded.opened_at,
       harness_session = NULL, harness_road = NULL, placement = NULL, handoff_note = NULL
     RETURNING *",
    vec![agent.into(), chat_id.into(), now().into()],
  )
  .await?
  .ok_or_else(|| fault("the room session was not written"))?;

  session_of(&row)
}

/// The next generation of a session that handed itself over at the handoff mark, holding the note the
/// closing one wrote until its first turn answers.
pub async fn hand_over(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  note: &str,
) -> Result<RoomSession, Refusal> {
  let row = one(
    db,
    "UPDATE room_sessions SET generation = generation + 1, opened_at = $1, harness_session = NULL,
       harness_road = NULL, placement = NULL, handoff_note = $2
     WHERE agent = $3 AND chat_id = $4 RETURNING *",
    vec![now().into(), note.into(), agent.into(), chat_id.into()],
  )
  .await?
  .ok_or_else(|| fault("the room session to hand over is gone"))?;

  session_of(&row)
}

/// The system prompt a session's next turn is sent with, by its hash: a session that held another records
/// when it changed; one that held none records the hash and changes nothing else.
pub async fn mark_system(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  hash: &str,
) -> Result<RoomSession, Refusal> {
  let row = one(
    db,
    "UPDATE room_sessions SET
       system_reset_at = CASE WHEN system_hash IS NOT NULL AND system_hash <> $1 THEN $2 ELSE system_reset_at END,
       system_hash = $1
     WHERE agent = $3 AND chat_id = $4 RETURNING *",
    vec![hash.into(), now().into(), agent.into(), chat_id.into()],
  )
  .await?
  .ok_or_else(|| fault("the room session to mark is gone"))?;

  session_of(&row)
}

/// What runs on the words not yet taken have spent so far, written as a run goes so a restart keeps it.
pub async fn mark_spent(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  spent: Spent,
) -> Result<(), Refusal> {
  run(
    db,
    "UPDATE room_sessions SET spent_calls = $1, spent_ms = $2 WHERE agent = $3 AND chat_id = $4",
    vec![spent.calls.into(), spent.ms.into(), agent.into(), chat_id.into()],
  )
  .await
  .map(drop)
  .map_err(Refusal::from)
}

/// Whether a generation keeps any turn.
pub async fn keeps_turns(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  generation: i64,
) -> Result<bool, Refusal> {
  Ok(
    one(
      db,
      "SELECT 1 AS one FROM room_session_turns WHERE agent = $1 AND chat_id = $2 AND generation = $3 LIMIT 1",
      vec![agent.into(), chat_id.into(), generation.into()],
    )
    .await?
    .is_some(),
  )
}

/// One turn's part as it is kept: what the agent was given, or what it answered.
pub struct Kept<'a> {
  pub role: &'static str,
  pub text: &'a str,
}

/// One turn kept on the generation it went out on, in order after what the generation holds, with the harness
/// session it answered in and the last message it took written onto the session. A turn kept has taken its
/// words, so what the runs on them spent is cleared; one that answered in a harness session has taken the
/// handoff note it opened with.
pub async fn record_turn(
  db: &DatabaseConnection,
  session: &RoomSession,
  turns: &[Kept<'_>],
  harness: Option<&Harness>,
  last_message_id: Option<i64>,
) -> Result<(), Refusal> {
  let (agent, chat_id, generation) = (session.agent.as_str(), session.chat_id, session.generation);
  let transaction = db.begin().await?;
  let last: Option<i64> = one(
    &transaction,
    "SELECT MAX(position) AS position FROM room_session_turns WHERE agent = $1 AND chat_id = $2 AND generation = $3",
    vec![agent.into(), chat_id.into(), generation.into()],
  )
  .await?
  .map(|row| row.try_get("", "position"))
  .transpose()?
  .flatten();
  let mut position = last.unwrap_or(0);
  let at = now();

  for turn in turns {
    position += 1;
    run(
      &transaction,
      "INSERT INTO room_session_turns (agent, chat_id, generation, position, role, text, at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
      vec![
        agent.into(),
        chat_id.into(),
        generation.into(),
        position.into(),
        turn.role.into(),
        turn.text.into(),
        at.into(),
      ],
    )
    .await?;
  }

  run(
    &transaction,
    "UPDATE room_sessions SET spent_calls = 0, spent_ms = 0 WHERE agent = $1 AND chat_id = $2",
    vec![agent.into(), chat_id.into()],
  )
  .await?;

  if let Some(harness) = harness {
    run(
      &transaction,
      "UPDATE room_sessions SET harness_session = $1, harness_road = $2, placement = $3, handoff_note = NULL
       WHERE agent = $4 AND chat_id = $5",
      vec![
        harness.id.as_str().into(),
        harness.road.as_str().into(),
        harness.placement.clone().into(),
        agent.into(),
        chat_id.into(),
      ],
    )
    .await?;
  }

  if let Some(last) = last_message_id {
    run(
      &transaction,
      "UPDATE room_sessions SET last_message_id = $1 WHERE agent = $2 AND chat_id = $3",
      vec![last.into(), agent.into(), chat_id.into()],
    )
    .await?;
  }

  transaction.commit().await?;

  Ok(())
}

/// When the agent's window in the room closes, in seconds; zero where it holds none.
pub async fn window_open_until(
  db: &impl ConnectionTrait,
  chat_id: i64,
  agent: &str,
) -> Result<i64, Refusal> {
  Ok(
    one(
      db,
      "SELECT open_until FROM room_windows WHERE chat_id = $1 AND agent = $2",
      vec![chat_id.into(), agent.into()],
    )
    .await?
    .map(|row| row.try_get("", "open_until"))
    .transpose()?
    .unwrap_or(0),
  )
}

/// The agent's window in the room opened until then, never shortened by a later opening.
pub async fn open_window(
  db: &DatabaseConnection,
  chat_id: i64,
  agent: &str,
  until: i64,
) -> Result<(), Refusal> {
  run(
    db,
    "INSERT INTO room_windows (chat_id, agent, open_until) VALUES ($1, $2, $3)
     ON CONFLICT (chat_id, agent) DO UPDATE SET open_until = GREATEST(room_windows.open_until, excluded.open_until)",
    vec![chat_id.into(), agent.into(), until.into()],
  )
  .await
  .map(drop)
  .map_err(Refusal::from)
}

/// Whether a reset of the room's session was asked and not yet carried out.
pub async fn reset_waiting(db: &DatabaseConnection, agent: &str, chat_id: i64) -> Result<bool, Refusal> {
  Ok(
    one(
      db,
      "SELECT id FROM session_resets WHERE agent = $1 AND chat_id IS NOT DISTINCT FROM $2 AND done_at IS NULL ORDER BY id LIMIT 1",
      vec![agent.into(), chat_id.into()],
    )
    .await?
    .is_some(),
  )
}

/// Every reset waiting for the room's session settled as carried out now.
pub async fn settle_reset(db: &DatabaseConnection, agent: &str, chat_id: i64) -> Result<(), Refusal> {
  run(
    db,
    "UPDATE session_resets SET done_at = $1 WHERE agent = $2 AND chat_id IS NOT DISTINCT FROM $3 AND done_at IS NULL",
    vec![now().into(), agent.into(), chat_id.into()],
  )
  .await
  .map(drop)
  .map_err(Refusal::from)
}
