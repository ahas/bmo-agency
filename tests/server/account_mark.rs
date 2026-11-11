//! A supervisor of an account keeps its prompt apart (docs/agent-ownership.md, 「What the supervisor's row
//! carries」): its system prompt is led by a mark made from its account alone, another account's by another mark,
//! and a supervisor of no account carries none.

use crate::support::scratch::Scratch;
use agency_server::supervisors::{account_led, account_mark};

async fn supervisor(scratch: &Scratch, name: &str, account: Option<i64>) {
  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "owner_account", "created_at", "updated_at")
         VALUES ($1, 'room', 'pieces', $2, 0, 0)"#,
      vec![name.into(), account.into()],
    )
    .await;
}

#[tokio::test]
async fn an_accounts_supervisor_leads_its_prompt_with_its_accounts_mark_alone() {
  let scratch = Scratch::new().await;

  supervisor(&scratch, "agent-a", Some(7)).await;
  supervisor(&scratch, "agent-b", Some(8)).await;
  supervisor(&scratch, "agent-c", None).await;

  let led = account_led(scratch.db(), "agent-a", "the prompt".to_owned()).await;

  assert_eq!(led, format!("{}\nthe prompt", account_mark(7)));
  assert!(!account_mark(7).contains("account"), "the mark says nothing of the account: {led}");
  assert_ne!(
    account_led(scratch.db(), "agent-b", "the prompt".to_owned()).await.lines().next(),
    led.lines().next()
  );
  assert_eq!(account_led(scratch.db(), "agent-c", "the prompt".to_owned()).await, "the prompt");
  assert_eq!(account_led(scratch.db(), "nobody", "the prompt".to_owned()).await, "the prompt");
}
