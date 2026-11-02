//! A room's session taken afresh, or handed over, clears what the turns before it spent (docs/companions.md,
//! 「A turn that goes nowhere」): a stale spent figure never judges the next turn over the bound at once.

use crate::support::scratch::Scratch;
use agency_server::companion::sessions::{hand_over, open_generation};

#[tokio::test]
async fn a_fresh_generation_clears_the_spent_figures() {
  let scratch = Scratch::new().await;

  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ('kakia-chat', 'chat')"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_sessions" ("agent", "chat_id", "generation", "opened_at", "spent_calls", "spent_ms")
         VALUES ('kakia-chat', -9000000000002, 1, 0, 3, 1801569)"#,
      vec![],
    )
    .await;

  let session = open_generation(scratch.db(), "kakia-chat", -9_000_000_000_002).await.unwrap();

  assert_eq!((session.spent.calls, session.spent.ms), (0, 0));
  assert_eq!(session.generation, 2);

  scratch
    .run(
      r#"UPDATE "room_sessions" SET "spent_calls" = 5, "spent_ms" = 900000
         WHERE "agent" = 'kakia-chat'"#,
      vec![],
    )
    .await;
  let handed = hand_over(scratch.db(), "kakia-chat", -9_000_000_000_002, "a note").await.unwrap();

  assert_eq!((handed.spent.calls, handed.spent.ms), (0, 0));
}

// A session the program cannot resume is let go like one it no longer holds: the CLI names both cases,
// and a turn is asked again once on a fresh session either way.
#[test]
fn a_session_the_program_cannot_continue_is_read_as_lost() {
  use agency_server::lines::companion::{LOST_SESSION, STALE_SESSION, lost_session};

  assert!(lost_session(&format!("Error: {LOST_SESSION} abc")));
  assert!(lost_session(&format!(
    "Error: {STALE_SESSION}. Provide a prompt to continue the conversation."
  )));
  assert!(!lost_session("the pane went away before it answered"));
}
