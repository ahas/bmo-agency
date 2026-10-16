//! What a job worker may be, judged before anything about it is written (docs/worker-slots.md): its
//! name, its label, the directory it works in, its preamble and its dev server's command, its support
//! workers, and what may be changed or removed of one already standing. Each refusal is said in the
//! words the console shows, which the policy's queue words hold.

use std::path::Path;
use std::sync::LazyLock;

use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;
use policy::workers::WORKERS;
use regex::Regex;

use super::workers::{Changes, Worker, support_name};
use crate::text::{grouped, length};

static WORKER_NAME: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&WORKERS.name_pattern).expect("the shared policy's worker name pattern reads")
});

/// Why a name cannot be taken, or none: the names taken are every agent's, since one log is kept per
/// name.
pub fn name_fault(name: &str, taken: &[String]) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if !WORKER_NAME.is_match(name) {
    return Some(words.name_shape.clone());
  }

  taken
    .iter()
    .any(|one| one == name)
    .then(|| fill_marks(&words.name_taken, &[("name", name)]))
}

fn label_fault(label: &str) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if label.trim().is_empty() {
    return Some(words.label_empty.clone());
  }

  (length(label) > WORKERS.label_most)
    .then(|| fill_marks(&words.label_long, &[("most", &WORKERS.label_most.to_string())]))
}

/// Why a directory cannot be worked in, or none. It has to be a directory that is there; whether it is
/// a checkout decides what the worker's jobs may do, not whether it may be one.
fn path_fault(path: &str) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if path.trim().is_empty() {
    return Some(words.path_empty.clone());
  }

  let at = Path::new(path);

  if !at.is_absolute() {
    return Some(words.path_relative.clone());
  }

  if !at.exists() {
    return Some(words.path_missing.clone());
  }

  (!at.is_dir()).then(|| words.path_not_directory.clone())
}

// What stands in the way of the directory made for a worker added with none.
fn made_directory_fault(path: &str) -> Option<String> {
  let at = Path::new(path);

  (at.exists() && !at.is_dir()).then(|| fill_marks(&QUEUE_WORDS.workers.made_blocked, &[("path", path)]))
}

// The two workbench fields: a preamble bounded the way a note is, and a dev command of one line.
fn bench_fault(preamble: Option<&str>, dev_command: Option<&str>) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if let Some(preamble) = preamble
    && length(preamble) > WORKERS.preamble_most
  {
    return Some(fill_marks(&words.preamble_long, &[("most", &grouped(WORKERS.preamble_most))]));
  }

  let command = dev_command?;

  if command.contains(['\r', '\n']) {
    return Some(words.dev_one_line.clone());
  }

  (length(command) > WORKERS.dev_command_most)
    .then(|| fill_marks(&words.dev_long, &[("most", &grouped(WORKERS.dev_command_most))]))
}

/// A worker to be added, as the request names it.
pub struct Adding<'a> {
  pub name: &'a str,
  pub label: &'a str,
  pub cwd: &'a str,
  /// Whether its directory is one to be made for it.
  pub made: bool,
  pub preamble: Option<&'a str>,
  pub dev_command: Option<&'a str>,
}

/// What is wrong with adding a worker, or none. A directory still to be made is judged only on what
/// stands in its way.
pub fn new_worker_fault(worker: &Adding<'_>, taken: &[String]) -> Option<String> {
  name_fault(worker.name, taken)
    .or_else(|| label_fault(worker.label))
    .or_else(|| if worker.made { made_directory_fault(worker.cwd) } else { path_fault(worker.cwd) })
    .or_else(|| bench_fault(worker.preamble, worker.dev_command))
}

/// What is wrong with changing one, or none. A support worker's settings follow from its base, and the
/// built-in worker's directory is not the owner's to move.
pub fn change_fault(target: &Worker, changes: &Changes) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if let Some(base) = target.support_of.as_deref()
    && changes.beyond_heal()
  {
    return Some(fill_marks(&words.support_change, &[("name", &target.name), ("base", base)]));
  }

  if changes.cwd.is_some() && target.builtin {
    return Some(fill_marks(&words.builtin_move, &[("name", &target.name)]));
  }

  bench_fault(
    changes.preamble.as_ref().and_then(Option::as_deref),
    changes.dev_command.as_ref().and_then(Option::as_deref),
  )
  .or_else(|| changes.label.as_deref().and_then(label_fault))
  .or_else(|| changes.cwd.as_deref().and_then(path_fault))
}

/// Why a worker cannot be removed, or none.
pub fn remove_fault(target: Option<&Worker>, asked: &str) -> Option<String> {
  let words = &QUEUE_WORDS.workers;
  let Some(target) = target else {
    return Some(fill_marks(&words.missing, &[("name", asked)]));
  };

  if let Some(base) = target.support_of.as_deref() {
    return Some(fill_marks(&words.support_remove, &[("name", &target.name), ("base", base)]));
  }

  target.builtin.then(|| fill_marks(&words.builtin_remove, &[("name", &target.name)]))
}

/// What is wrong with a base's support settings, judged as the pair they make, or none.
pub fn support_fault(base: &Worker, count: f64, keepalive: f64) -> Option<String> {
  let words = &QUEUE_WORDS.workers;

  if let Some(of) = base.support_of.as_deref().filter(|of| !of.is_empty()) {
    return Some(fill_marks(&words.support_settings, &[("name", &base.name), ("base", of)]));
  }

  let most = WORKERS.support_most as f64;

  if count.fract() != 0.0 || count < 0.0 || count > most {
    return Some(fill_marks(&words.support_count, &[("most", &WORKERS.support_most.to_string())]));
  }

  if keepalive.fract() != 0.0 || keepalive < 0.0 {
    return Some(words.keepalive_count.clone());
  }

  if keepalive > count {
    return Some(fill_marks(
      &words.keepalive_over,
      &[("keepalive", &keepalive.to_string()), ("count", &count.to_string())],
    ));
  }

  let last = support_name(&base.name, count as i64 + WORKERS.first_support_slot - 1);

  (count > 0.0 && !WORKER_NAME.is_match(&last))
    .then(|| fill_marks(&words.support_name_long, &[("name", &base.name), ("last", &last)]))
}
