use std::path::Path;

use agency_server::verbs::agents::*;
use serde_json::json;

#[test]
fn a_moment_is_told_in_minutes_under_the_hour_and_hours_above() {
  assert_eq!(ago(None, 10_000), "never");
  assert_eq!(ago(Some(10_000 - 89), 10_000), "1m ago");
  assert_eq!(ago(Some(10_000 - 90), 10_000), "2m ago");
  assert_eq!(ago(Some(10_000 - 3_570), 10_000), "1h ago");
  assert_eq!(ago(Some(10_000 - 5_400), 10_000), "2h ago");
}

#[test]
fn a_checkout_on_a_node_is_named_beside_its_base_and_written_from_the_home() {
  let home = Path::new("/home/someone");

  assert_eq!(node_checkout_path("/home/someone/repos/app", "n3", home), "~/repos/app-n3");
  assert_eq!(node_checkout_path("/srv/app/", "n3", home), "/srv/app-n3");
}

#[test]
fn a_count_lets_its_grouping_go_and_hands_a_word_on_as_it_stands() {
  assert_eq!(count(Some("65,536")), Some(json!(65_536)));
  assert_eq!(count(Some("many")), Some(json!("many")));
  assert_eq!(count(None), None);
  assert!(number("").abs() < f64::EPSILON);
  assert!(number("x").is_nan());
}
