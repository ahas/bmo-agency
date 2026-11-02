//! The places a thrown-away session on a road whose program carries its own turns goes out in
//! (docs/api.md, 「sessions」): one place a session, named after it and marked as its own by a record in it,
//! so it stands under no agent's row and no sweep of leftovers takes it down while the session stands; ended
//! with the session, and every one the process before this one left ended as a server starts, since a
//! thrown-away session does not outlive the process that started it.

use std::path::Path;

use agent_session::SessionId;
use serde_json::{Value, json};

use crate::lines::place::{Place, places_dir, token_of};
use crate::lines::token;
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, hold};

/// The record a thrown-away session's place keeps while the session stands, naming the session.
pub const PLACE_RECORD: &str = "apart-place.json";

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

/// The agent whose thrown-away session a place is, as its record names the session: the name before the
/// session's `@`; none for a name that is no such place.
pub fn session_agent(data: &Path, name: &str) -> Option<String> {
  let record = agent_host::disk::read_json::<Value>(&Place::of(data, name).path.join(PLACE_RECORD))?;

  if record["place"] != name {
    return None;
  }

  record["session"]
    .as_str()
    .and_then(|session| session.split('@').next())
    .filter(|agent| !agent.is_empty())
    .map(str::to_owned)
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
/// closed and its folder taken away, under its lock, taken once a turn still holding it ends however long
/// that takes.
pub async fn end(ground: &Ground, name: &str) {
  let data = &ground.roots.data;
  let _held = match hold(agent_lock_path(data, name)).await {
    Ok(held) => held,
    Err(fault) => return eprintln!("thrown-away place {name}: its lock could not be taken ({fault})"),
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
/// What each held is written to the log first — the program its record names, whether that process runs,
/// and what its pane's head was last answered — since taking the place down takes that evidence with it.
pub async fn sweep(ground: &Ground) -> usize {
  let left = apart_places(&ground.roots.data);

  for name in &left {
    let found = crate::door::liveness::place_found(&Place::of(&ground.roots.data, name));

    println!("agency: thrown-away place {name} as the last process left it: {found}");
    end(ground, name).await;
  }

  left.len()
}
