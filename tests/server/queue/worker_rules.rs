//! What a job worker may be: a name of the small alphabet and nobody else's, a label within its bound, a
//! directory that stands, and support settings that make a pair; each refusal in the policy's English.

use agency_server::queue::worker_rules::{
  Adding, name_fault, new_worker_fault, remove_fault, support_fault,
};
use agency_server::queue::workers::Worker;
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;

fn worker(name: &str, support_of: Option<&str>, builtin: bool) -> Worker {
  Worker {
    name: name.into(),
    label: name.into(),
    cwd: "/work".into(),
    builtin,
    support_count: 0,
    support_keepalive: 0,
    support_of: support_of.map(str::to_owned),
    support_slot: None,
    node: None,
  }
}

#[test]
fn a_name_is_the_small_alphabet_and_nobody_elses() {
  assert!(name_fault("infra", &[]).is_none());
  assert_eq!(name_fault("INFRA", &[]).as_deref(), Some(QUEUE_WORDS.workers.name_shape.as_str()));
  assert_eq!(
    name_fault("search", &["search".into()]),
    Some(fill_marks(&QUEUE_WORDS.workers.name_taken, &[("name", "search")]))
  );
}

fn adding(cwd: &str, made: bool) -> Adding<'_> {
  Adding {
    name: "infra",
    label: "Infra",
    cwd,
    made,
    preamble: None,
    dev_command: Some("bun dev"),
  }
}

#[test]
fn a_new_workers_directory_must_stand_unless_it_is_to_be_made() {
  let dir = tempfile::tempdir().unwrap();
  let missing = dir.path().join("missing").to_string_lossy().into_owned();

  assert_eq!(
    new_worker_fault(&adding(&missing, false), &[]).as_deref(),
    Some(QUEUE_WORDS.workers.path_missing.as_str())
  );
  assert!(new_worker_fault(&adding(&missing, true), &[]).is_none());
  assert!(new_worker_fault(&adding(&dir.path().to_string_lossy(), false), &[]).is_none());
  assert_eq!(
    new_worker_fault(
      &Adding {
        dev_command: Some("a\nb"),
        ..adding(&missing, true)
      },
      &[]
    )
    .as_deref(),
    Some(QUEUE_WORDS.workers.dev_one_line.as_str())
  );
}

#[test]
fn the_built_in_and_a_support_worker_are_not_removed_and_support_settings_make_a_pair() {
  assert_eq!(
    remove_fault(None, "ghost"),
    Some(fill_marks(&QUEUE_WORDS.workers.missing, &[("name", "ghost")]))
  );
  assert!(remove_fault(Some(&worker("worker", None, true)), "worker").is_some());
  assert!(remove_fault(Some(&worker("infra-w2", Some("infra"), false)), "infra-w2").is_some());
  assert!(remove_fault(Some(&worker("infra", None, false)), "infra").is_none());

  let base = worker("infra", None, false);

  assert!(support_fault(&base, 2.0, 1.0).is_none());
  assert_eq!(
    support_fault(&base, 1.0, 2.0),
    Some(fill_marks(&QUEUE_WORDS.workers.keepalive_over, &[("keepalive", "2"), ("count", "1")]))
  );
  assert!(support_fault(&base, 1.5, 0.0).is_some());
}
