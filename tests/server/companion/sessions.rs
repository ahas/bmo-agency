//! A room's session taken afresh, or handed over, clears what the turns before it spent (docs/companions.md,
//! 「A turn that goes nowhere」): a stale spent figure never judges the next turn over the bound at once.

use crate::scratch::Scratch;
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
