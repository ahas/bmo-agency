//! A gate's places (docs/agent-control.md, 「A gate's places」): on a road whose program carries its own
//! turns, a gate's turn goes into a place kept for the program its ask composes rather than into its agent's
//! one pane, so each program stays standing between judgements and the judgements of different programs run
//! at once. A place is named after the agent, a digest of the program and a slot; it takes one turn at a
//! time, a program stands in as many as its overlapping turns need up to a bound, and every program's places
//! together stand up to another. A place unused longest makes room where that bound is reached, and the
//! heads' clock ends each one unused past the idle bound. Each place keeps a record of what it is and when a
//! turn last let it go, so a server started again takes the standing places over and the leftovers sweep
//! counts them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use harness_providers::providers;
use policy::gate_places::GATE_PLACES;
use policy::providers::Turns;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::Ground;
use super::locks::{agent_lock_path, hold_within, try_hold};
use crate::door::inbox::record_path;
use crate::lines::place::{Place, places_dir};
use crate::now_ms;

/// The record each gate place keeps beside its program.
pub const RECORD: &str = "gate-place.json";

// How many hex digits of a program's digest name its places.
const KEY_CHARS: usize = 10;

// How long ending a place waits for a turn still holding its lock.
const END_WAIT: Duration = Duration::from_secs(60);

/// What a gate's ask composes its program of: the conversation's agent, the road, the model, the effort,
/// whether it thinks, the instructions and the folder.
#[derive(Serialize)]
pub struct Asked<'a> {
  pub agent: &'a str,
  pub provider: &'a str,
  pub model: &'a str,
  pub effort: &'a str,
  pub thinking: bool,
  pub system: &'a str,
  pub cwd: &'a str,
}

impl Asked<'_> {
  /// The program's key: the digest of everything it is composed of, cut to the length a place is named by.
  pub fn key(&self) -> String {
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(self).unwrap_or_default()));

    digest[..KEY_CHARS].to_owned()
  }
}

/// A place's name: its agent, its program's key and its slot.
pub fn place_name(agent: &str, program: &str, slot: usize) -> String {
  format!("{agent}-{program}-{slot}")
}

/// What a gate place records of itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
  pub agent: String,
  pub program: String,
  pub slot: usize,
  pub used_at: i64,
}

impl Record {
  fn write(&self, data: &Path) {
    let place = Place::of(data, &place_name(&self.agent, &self.program, self.slot));
    let written = agent_host::disk::private_dir(&place.path)
      .and_then(|()| agent_host::disk::publish(&place.path.join(RECORD), self));

    if let Err(why) = written {
      eprintln!("gate place {}: its record was not written: {why}", place.path.display());
    }
  }
}

/// Every gate place recorded under a data root, by name.
pub fn recorded(data: &Path) -> BTreeMap<String, Record> {
  let Ok(entries) = std::fs::read_dir(places_dir(data)) else {
    return BTreeMap::new();
  };

  entries
    .filter_map(|entry| entry.ok())
    .filter_map(|entry| {
      let record: Record = agent_host::disk::read_json(&entry.path().join(RECORD))?;

      Some((entry.file_name().to_string_lossy().into_owned(), record))
    })
    .filter(|(name, record)| *name == place_name(&record.agent, &record.program, record.slot))
    .collect()
}

/// Whether a name is a gate place's, as its record says.
pub fn recorded_place(data: &Path, name: &str) -> bool {
  agent_host::disk::read_json::<Record>(&Place::of(data, name).path.join(RECORD))
    .is_some_and(|record| place_name(&record.agent, &record.program, record.slot) == name)
}

/// The bounds the places stand within.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
  pub places: usize,
  pub per_program: usize,
  pub idle_ms: i64,
}

impl Bounds {
  pub fn policy() -> Self {
    Self {
      places: GATE_PLACES.places,
      per_program: GATE_PLACES.per_program,
      idle_ms: GATE_PLACES.idle_ms(),
    }
  }
}

#[derive(Clone, Debug)]
struct Held {
  agent: String,
  program: String,
  slot: usize,
  busy: bool,
  used_at: i64,
}

/// A place a turn took: its name, whether it was stood afresh for the turn, and the place of another
/// program ended to make room for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seated {
  pub name: String,
  pub slot: usize,
  pub fresh: bool,
  pub ended: Option<String>,
}

/// Which places stand, which are at work and when each was last let go: the decisions alone, with no place
/// touched.
#[derive(Debug, Default)]
pub struct Board {
  held: BTreeMap<String, Held>,
  adopted: bool,
}

impl Board {
  /// A place for a turn of `program`: a free one of its own, the lowest slot first; else a new slot where
  /// the program stands in fewer than its bound, making room by the place of another program unused
  /// longest where every program's places together are at theirs; else none, and the turn waits.
  pub fn take(&mut self, agent: &str, program: &str, now: i64, bounds: Bounds) -> Option<Seated> {
    let own: Vec<(&String, &Held)> = self
      .held
      .iter()
      .filter(|(_, held)| held.agent == agent && held.program == program)
      .collect();

    if let Some((name, held)) = own.iter().filter(|(_, held)| !held.busy).min_by_key(|(_, held)| held.slot)
    {
      let seated = Seated {
        name: (*name).clone(),
        slot: held.slot,
        fresh: false,
        ended: None,
      };

      if let Some(held) = self.held.get_mut(&seated.name) {
        held.busy = true;
      }

      return Some(seated);
    }

    if own.len() >= bounds.per_program.max(1) {
      return None;
    }

    let taken: BTreeSet<usize> = own.iter().map(|(_, held)| held.slot).collect();
    let slot = (1..).find(|slot| !taken.contains(slot)).unwrap_or(1);
    let ended = if self.held.len() >= bounds.places.max(1) {
      let unused = self
        .held
        .iter()
        .filter(|(_, held)| !held.busy && !(held.agent == agent && held.program == program))
        .min_by_key(|(_, held)| held.used_at)
        .map(|(name, _)| name.clone())?;

      self.held.remove(&unused);
      Some(unused)
    } else {
      None
    };
    let name = place_name(agent, program, slot);

    self.held.insert(
      name.clone(),
      Held {
        agent: agent.to_owned(),
        program: program.to_owned(),
        slot,
        busy: true,
        used_at: now,
      },
    );

    Some(Seated {
      name,
      slot,
      fresh: true,
      ended,
    })
  }

  /// A place a turn let go, free for the next.
  pub fn let_go(&mut self, name: &str, now: i64) {
    if let Some(held) = self.held.get_mut(name) {
      held.busy = false;
      held.used_at = now;
    }
  }

  /// The places unused past the idle bound, taken off the board to be ended.
  pub fn idle(&mut self, now: i64, bounds: Bounds) -> Vec<String> {
    let idle: Vec<String> = self
      .held
      .iter()
      .filter(|(_, held)| !held.busy && now - held.used_at >= bounds.idle_ms)
      .map(|(name, _)| name.clone())
      .collect();

    for name in &idle {
      self.held.remove(name);
    }

    idle
  }

  /// A place a server before this one stood, free, as its record says it was last let go.
  pub fn adopt(&mut self, name: &str, record: &Record) {
    self.held.entry(name.to_owned()).or_insert_with(|| Held {
      agent: record.agent.clone(),
      program: record.program.clone(),
      slot: record.slot,
      busy: false,
      used_at: record.used_at,
    });
  }

  /// The places standing, by name.
  pub fn standing(&self) -> Vec<String> {
    self.held.keys().cloned().collect()
  }

  /// The agents the places stand for.
  pub fn agents(&self) -> BTreeSet<String> {
    self.held.values().map(|held| held.agent.clone()).collect()
  }

  /// How many places are at work.
  pub fn busy(&self) -> usize {
    self.held.values().filter(|held| held.busy).count()
  }

  fn record(&self, name: &str) -> Option<Record> {
    self.held.get(name).map(|held| Record {
      agent: held.agent.clone(),
      program: held.program.clone(),
      slot: held.slot,
      used_at: held.used_at,
    })
  }
}

/// The places as one server keeps them, and the word that one came free.
#[derive(Debug, Default)]
pub struct GatePlaces {
  board: Mutex<Board>,
  freed: tokio::sync::Notify,
}

/// A place a turn holds: let go, and its record written, when the turn is done with it.
pub struct Seat {
  pub name: String,
  places: Arc<GatePlaces>,
  data: PathBuf,
}

impl Drop for Seat {
  fn drop(&mut self) {
    let record = {
      let mut board = self.places.board();

      board.let_go(&self.name, now_ms());
      board.record(&self.name)
    };

    if let Some(record) = record {
      record.write(&self.data);
    }

    self.places.freed.notify_waiters();
  }
}

impl Seat {
  /// The place's session let go once its turn has answered: the program ended, its session record removed,
  /// and the loop left to start the same program again on a fresh session. The place stays held until it
  /// has, so no turn goes into a program going down.
  pub async fn start_afresh(self) {
    let place = Place::of(&self.data, &self.name);
    let Some(running) = place.program_alive() else { return };
    let session = place.path.join("session.json");

    if let Err(why) = std::fs::remove_file(&session)
      && why.kind() != std::io::ErrorKind::NotFound
    {
      eprintln!("gate place {}: its session was not let go: {why}", self.name);
      return;
    }

    place.end(running.pid).await;

    for _ in 0..100 {
      if !place.alive(running.pid, running.birth.as_deref()) {
        break;
      }

      tokio::time::sleep(Duration::from_millis(50)).await;
    }

    println!("gate place {}: its session passed its bound and starts afresh", self.name);
  }
}

impl GatePlaces {
  fn board(&self) -> MutexGuard<'_, Board> {
    self.board.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The places a server before this one left standing, taken over once.
  fn adopted(&self, data: &Path) -> MutexGuard<'_, Board> {
    let mut board = self.board();

    if !board.adopted {
      board.adopted = true;

      for (name, record) in recorded(data) {
        board.adopt(&name, &record);
      }
    }

    board
  }

  /// A place for a turn of `program`, waited for where every one it may take is at work. A place of another
  /// program ended to make room is ended before the place is handed over.
  pub async fn take(self: &Arc<Self>, ground: &Ground, agent: &str, program: &str) -> Seat {
    let data = ground.roots.data.clone();

    loop {
      let freed = self.freed.notified();
      let mut freed = std::pin::pin!(freed);

      freed.as_mut().enable();

      let seated = self.adopted(&data).take(agent, program, now_ms(), Bounds::policy());

      if let Some(seated) = seated {
        if let Some(ended) = &seated.ended {
          end_place(ground, ended, "made room for another program").await;
        }

        let seat = Seat {
          name: seated.name.clone(),
          places: self.clone(),
          data: data.clone(),
        };

        if seated.fresh
          && let Some(record) = self.board().record(&seated.name)
        {
          record.write(&data);
        }

        return seat;
      }

      freed.await;
    }
  }

  /// Every place unused past the idle bound ended, and every agent's own pane program on a road whose
  /// program carries its own turns that nothing has been put into for as long.
  pub async fn fold_idle(&self, ground: &Ground) {
    let data = &ground.roots.data;
    let bounds = Bounds::policy();
    let (idle, agents) = {
      let mut board = self.adopted(data);

      (board.idle(now_ms(), bounds), board.agents())
    };

    for name in &idle {
      end_place(ground, name, "stood unused past its bound").await;
    }

    for agent in &agents {
      fold_own_pane(ground, agent, bounds.idle_ms).await;
    }
  }
}

/// A gate place ended: its program released, its workspace closed and its record removed, under its lock, so
/// a turn standing it again waits until it is down.
pub async fn end_place(ground: &Ground, name: &str, why: &str) {
  let data = &ground.roots.data;
  let Some(_held) = hold_within(agent_lock_path(data, name), END_WAIT).await else {
    return eprintln!("gate place {name}: a turn still holds it, so it was not ended");
  };
  let place = Place::of(data, name);

  place.release().await;

  if let Some(home) = ground.home_of(name, name) {
    let _ = home.close().await;
  }

  let _ = std::fs::remove_file(place.path.join(RECORD));
  println!("gate place {name}: ended, {why}");
}

// Whether a program stands on the pane program of a road whose program carries its own turns.
fn inbox_program(args: &[String]) -> bool {
  let Some(file) = args.first().and_then(|first| Path::new(first).file_name()) else {
    return false;
  };

  providers()
    .iter()
    .filter_map(|provider| provider.declared().resident.as_ref())
    .any(|resident| {
      resident.turns == Turns::Inbox && resident.program.as_deref().is_some_and(|program| file == program)
    })
}

// The agent's own pane program on such a road released where nothing has gone into it for the idle bound:
// gate turns stand in places of their own, so one left in the agent's pane serves none of them.
async fn fold_own_pane(ground: &Ground, agent: &str, idle_ms: i64) {
  let data = &ground.roots.data;
  let place = Place::of(data, agent);
  let Some(program) = place.program() else { return };

  if !inbox_program(&program.command.args) {
    return;
  }

  let touched = std::fs::metadata(record_path(&place.path))
    .and_then(|meta| meta.modified())
    .ok()
    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
    .map_or(0, |since| since.as_millis() as i64);

  if now_ms() - touched < idle_ms {
    return;
  }

  let Some(_held) = try_hold(&agent_lock_path(data, agent)) else { return };

  if place.release().await.is_some() {
    println!("{agent}: its pane program served no gate place for its idle bound, and was taken down");
  }
}
