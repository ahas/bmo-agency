//! What a companion's reset tells the units holding the rest of it, by the names and shapes the boundary
//! gives them.

use agency_server::companion::reset::{CompanionCutOff, CompanionReset};
use agency_server::events::Event;
use serde_json::json;

#[test]
fn a_cut_off_and_a_reset_are_told_in_their_boundary_shapes() {
  assert_eq!(CompanionCutOff::NAME, "companion-reset.created");
  assert_eq!(CompanionReset::NAME, "companion-reset.created");
  assert_eq!(
    serde_json::to_value(CompanionCutOff {
      agent: "pal".into(),
      point: 1_790_000_000_000,
    })
    .unwrap(),
    json!({ "agent": "pal", "point": 1_790_000_000_000_i64 })
  );
  assert_eq!(
    serde_json::to_value(CompanionReset {
      agent: "pal".into(),
      ranges: vec!["conversation".into()],
      alone: vec![-7],
      shared: vec![12],
      by: "console".into(),
      at: 1_790_000_000,
    })
    .unwrap(),
    json!({
      "agent": "pal",
      "ranges": ["conversation"],
      "alone": [-7],
      "shared": [12],
      "by": "console",
      "at": 1_790_000_000,
    })
  );
}
