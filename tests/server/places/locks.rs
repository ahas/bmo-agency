//! The file locks the server holds: named as the TypeScript side names them, and held until let go.

use std::path::Path;
use std::time::{Duration, Instant};

use agency_server::places::locks::{held, lock_file_name, try_hold};

#[test]
fn a_lock_file_is_named_as_the_typescript_side_names_it() {
  assert_eq!(
    lock_file_name("/home/tester/repos/project"),
    "home-tester-repos-project-a40274a70664.lock"
  );
  assert_eq!(lock_file_name("/"), "root-8a5edab28263.lock");
  assert_eq!(lock_file_name("dev-pane:worker-w2"), "dev-pane-worker-w2-7fdd551d2021.lock");
  assert_eq!(
    lock_file_name(
      "job-hold:/home/tester/repos/a very/long/path/that/goes/on/and/on/past/sixty-four/characters/of/slug/text/here"
    ),
    "that-goes-on-and-on-past-sixty-four-characters-of-slug-text-here-28c916cfda36.lock"
  );
}

#[test]
fn a_held_lock_reads_held_until_it_is_let_go() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("locks").join("a.lock");

  assert!(!held(&path));

  let holding = try_hold(&path).unwrap();

  assert!(held(&path));
  assert!(try_hold(&path).is_none());
  drop(holding);
  assert!(let_go_within(&path, Duration::from_secs(5)));
}

// Another test of this process may fork a child while the lock is open, and the child keeps a copy of
// the descriptor, and the lock with it, until it execs; so the lock reads free once that copy is gone,
// which a loaded machine can take a moment to reach.
fn let_go_within(path: &Path, wait: Duration) -> bool {
  let until = Instant::now() + wait;

  loop {
    if !held(path) {
      return true;
    }

    if Instant::now() >= until {
      return false;
    }

    std::thread::sleep(Duration::from_millis(10));
  }
}
