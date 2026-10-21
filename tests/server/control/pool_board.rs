//! The pool's board: a tile's work in its request's own words, and the kind it is shown as.

use agency_server::control::pool::Request;
use agency_server::control::pool_board::*;
use serde_json::json;

fn request(source: &str, turn: bool, state: &str) -> Request {
  Request {
    sequence: 1,
    token: "token".into(),
    request_key: "work-order:12:stage-1:scout".into(),
    owner: "work-order:12".into(),
    source: source.into(),
    role: "scout".into(),
    work_key: "role:scout".into(),
    generation: 1,
    ended_at: None,
    outcome: None,
    reservation: None,
    provider: "codex".into(),
    model: "model-a".into(),
    effort: Some("low".into()),
    tier: "Light".into(),
    capacity: 4,
    state: state.into(),
    enqueued_at: 100,
    leased_at: Some(200),
    child_id: Some("run-1".into()),
    yielding: false,
    place_key: None,
    turn,
    session: None,
  }
}

#[test]
fn a_tile_shows_its_request_in_the_request_s_own_words() {
  let work = work_of(&request("cowork", false, "active"));

  assert_eq!(work["kind"], "cowork");
  assert_eq!(work["title"], "work-order:12");
  assert_eq!(work["detail"], "work-order:12:stage-1:scout");
  assert_eq!(work["since"], 200);
  assert_eq!(work["log"], json!({ "run": "run-1" }));
  assert_eq!(work["stop"], json!({ "kind": "unit", "id": "run-1" }));
  assert_eq!(work_of(&request("cowork", false, "waiting"))["since"], 100);
}

#[test]
fn a_request_is_shown_as_a_turn_a_branch_a_native_call_or_its_own_source() {
  assert_eq!(kind_of(&request("worker", true, "active")), "turn");
  assert_eq!(kind_of(&request("cowork", false, "active")), "cowork");
  assert_eq!(kind_of(&request("native", false, "active")), "native");
  assert_eq!(kind_of(&request("search", false, "active")), "search");
}
