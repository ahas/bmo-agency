//! The places a thrown-away session on a road whose program carries its own turns goes out in
//! (docs/api.md, 「sessions」): one place a session, named after it and marked as its own by a record in it,
//! so it stands under no agent's row and no sweep of leftovers takes it down while the session stands; ended
//! with the session, and every one the process before this one left ended as a server starts, since a
//! thrown-away session does not outlive the process that started it.

use std::path::Path;
use std::time::Duration;

use agent_session::SessionId;
use serde_json::{Value, json};

use crate::lines::place::{Place, places_dir, token_of};
use crate::lines::token;
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, hold_within};

/// The record a thrown-away session's place keeps while the session stands, naming the session.
pub const PLACE_RECORD: &str = "apart-place.json";

// How long the end of a session's place waits for a turn still holding it.
const END_WAIT: Duration = Duration::from_secs(30);

/// The place a thrown-away session's turns go into, by the session's id: its agent and its number.
pub fn place_name(session: &SessionId) -> String {
  let named: String = session
    .0
    .chars()
    .map(|one| if one.is_ascii_alphanumeric() || one == '-' { one } else { '-' })
    .collect();

  format!("apart-{named}")
}

/// Whether a name is a thrown-away session's place, as its record says.
pub fn is_apart_place(data: &Path, name: &str) -> bool {
  agent_host::disk::read_json::<Value>(&Place::of(data, name).path.join(PLACE_RECORD))
    .is_some_and(|record| record["place"] == name)
}

/// Every thrown-away session's place standing now, by its name.
pub fn apart_places(data: &Path) -> Vec<String> {
  let Ok(entries) = std::fs::read_dir(places_dir(data)) else {
    return vec![];
  };

  entries
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| is_apart_place(data, name))
    .collect()
}

/// A session's place made ready for its turns: its folder and the record that names it.
pub fn mark(ground: &Ground, name: &str, session: &SessionId) -> Result<(), String> {
  let path = Place::of(&ground.roots.data, name).path;

  std::fs::create_dir_all(&path).map_err(|why| format!("{}: {why}", path.display()))?;
  std::fs::write(path.join(PLACE_RECORD), json!({ "place": name, "session": session.0 }).to_string())
    .map_err(|why| format!("{}: {why}", path.display()))
}

/// A session's place ended: its program released, the session token it carried taken back, its workspace
/// closed and its folder taken away, under its lock.
pub async fn end(ground: &Ground, name: &str) {
  let data = &ground.roots.data;
  let Some(_held) = hold_within(agent_lock_path(data, name), END_WAIT).await else {
    return eprintln!("thrown-away place {name}: a turn still holds it, so it was not ended");
  };
  let place = Place::of(data, name);

  if let Some(token) = place.release().await.as_ref().and_then(token_of) {
    token::revoke(ground, token).await;
  }

  if let Some(home) = ground.home_of(name, name) {
    let _ = home.close().await;
  }

  let _ = std::fs::remove_dir_all(&place.path);
}

/// Every thrown-away session's place the process before this one left, ended as a server starts: how many.
pub async fn sweep(ground: &Ground) -> usize {
  let left = apart_places(&ground.roots.data);

  for name in &left {
    end(ground, name).await;
  }

  left.len()
}
