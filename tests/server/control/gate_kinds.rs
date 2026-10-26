//! The gate kinds the gate declares (docs/api.md, 「Gate kinds」): the order kept in the store, the
//! switches an agent's turn reads from it, and the reasoning a gate's name does not own.

use agency_server::control::agent_switches::{agent_gates, owns_reasoning, switch_gate, turn_gates_of};
use agency_server::conversational::Shape;
use agency_server::gate_kinds::{self, Kind};
use bmo_gate_client::Gate;
use serde_json::json;

use crate::scratch::Scratch;

fn gate(name: &str, part: &str, step: i64) -> Gate {
  Gate {
    gate_name: name.into(),
    part: part.into(),
    step,
    condition: None,
    switchable: false,
    laya_probe: false,
    default_laya_mode: None,
    answer_tokens: None,
  }
}

fn declared() -> Vec<Gate> {
  vec![
    gate("system-documents", "pre", 0),
    gate("sanitizer", "pre", 1),
    gate("sanitizer", "post", 0),
    gate("output-format", "post", 0),
    gate("translator", "apart", 1),
  ]
}

#[tokio::test]
async fn the_gate_s_declaration_is_kept_in_its_order_and_written_over_whole() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  gate_kinds::store(db, &declared()).await.unwrap();

  let kinds = gate_kinds::declared(db).await.unwrap();

  assert_eq!(
    kinds.iter().map(|kind| (kind.part.as_str(), kind.key.as_str())).collect::<Vec<_>>(),
    [
      ("pre", "system-documents"),
      ("pre", "sanitizer"),
      ("post", "sanitizer"),
      ("post", "output-format"),
      ("apart", "translator"),
    ]
  );
  assert!(gate_kinds::holds(db, "translator").await.unwrap());
  assert!(!gate_kinds::holds(db, "no-such-gate").await.unwrap());

  gate_kinds::store(db, &[gate("room-rules", "pre", 0)]).await.unwrap();

  assert_eq!(
    gate_kinds::declared(db).await.unwrap(),
    vec![Kind {
      key: "room-rules".into(),
      part: "pre".into(),
      step: 0,
      condition: None,
    }]
  );
  assert!(
    !gate_kinds::holds(db, "translator").await.unwrap(),
    "a declaration replaces the one standing"
  );
}

#[tokio::test]
async fn a_chat_turn_reads_the_declared_pre_and_post_parts_once_each_in_their_steps() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  gate_kinds::store(db, &declared()).await.unwrap();

  let turn = gate_kinds::turn(&gate_kinds::declared(db).await.unwrap());
  let gates = turn.gates();

  assert_eq!(
    gates,
    vec![("pre", "system-documents"), ("pre", "sanitizer"), ("post", "output-format"),],
    "a gate in both parts keeps the first part it stands in, and no off-turn gate is a turn's"
  );
  assert_eq!(turn.pre.len(), 2, "the two pre-gates stand on two steps");
  assert_eq!(turn.post.len(), 1, "the post-gates start together");

  assert_eq!(
    turn_gates_of(db, Some(Shape::Window)).await.unwrap().unwrap(),
    vec![
      ("pre", "system-documents".to_owned()),
      ("pre", "sanitizer".to_owned()),
      ("post", "output-format".to_owned()),
    ]
  );
  assert!(
    turn_gates_of(db, Some(Shape::Pieces)).await.unwrap().unwrap().is_empty(),
    "no gate picks for it"
  );
  assert!(turn_gates_of(db, None).await.unwrap().is_none());
}

#[tokio::test]
async fn an_agent_switches_the_gates_its_turn_passes_and_a_gate_owns_no_reasoning() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "default_answerer")
         VALUES ('chatter', 'chat', 'agent', 'window', 1), ('sanitizer', 'gate', NULL, NULL, 0)"#,
      vec![],
    )
    .await;
  gate_kinds::store(db, &declared()).await.unwrap();

  let switches = agent_gates(db, "chatter").await.unwrap().unwrap();

  assert_eq!(
    switches
      .iter()
      .map(|switch| (switch.part, switch.key.as_str(), switch.on))
      .collect::<Vec<_>>(),
    [
      ("pre", "system-documents", true),
      ("pre", "sanitizer", true),
      ("post", "output-format", true)
    ]
  );

  assert!(switch_gate(db, "chatter", &json!("sanitizer"), &json!(false)).await.unwrap().is_ok());
  assert!(
    switch_gate(db, "chatter", &json!("document-seek"), &json!(false))
      .await
      .unwrap()
      .is_err(),
    "an off-turn gate is no gate of a turn"
  );

  let off = agent_gates(db, "chatter").await.unwrap().unwrap();

  assert!(!off.iter().find(|switch| switch.key == "sanitizer").unwrap().on);

  assert!(
    !owns_reasoning(db, "sanitizer", None).await.unwrap(),
    "a gate is a judgement under its tier"
  );
  assert!(owns_reasoning(db, "chatter", None).await.unwrap());
  assert!(!owns_reasoning(db, "chatter", Some("base")).await.unwrap());
}
