//! The session a run apart's role keeps (docs/sessions.md, 「A run apart's kept session」; docs/agent-roles.md,
//! 「A role's sessions」): the setting read off the role the run's agent holds, or the one the run names for
//! itself where its agent stands no row; the command's key blind to its session and moved by every other
//! word, the system prompt's text among them; and a kept session taken by one run at a time while it is warm.

use std::collections::BTreeMap;

use agency_server::apart_sessions::{keep, key, setting, take};
use agency_server::lines::claude::Command;
use agency_server::now_ms;
use policy::roles::Sessions;

use crate::scratch::Scratch;

fn command(model: &str, prompt: &std::path::Path, session: &str) -> Command {
  Command {
    args: [
      "claude",
      "--model",
      model,
      "--system-prompt-file",
      &prompt.to_string_lossy(),
      "--session-id",
      session,
    ]
    .map(str::to_owned)
    .to_vec(),
    cwd: "/work/search".into(),
    env: BTreeMap::new(),
  }
}

#[test]
fn a_command_s_key_is_blind_to_its_session_and_moved_by_its_model_and_its_prompt_s_text() {
  let folder = tempfile::tempdir().unwrap();
  let prompt = folder.path().join("system.md");
  let home = std::path::Path::new("/logins/a");

  std::fs::write(&prompt, "Search well.").unwrap();

  let first = key("searcher", "search", &command("sonnet", &prompt, "one"), home);

  assert_eq!(first, key("searcher", "search", &command("sonnet", &prompt, "two"), home));
  assert_ne!(first, key("searcher", "search", &command("opus", &prompt, "one"), home));
  assert_ne!(first, key("searcher", "browse", &command("sonnet", &prompt, "one"), home));
  assert_ne!(
    first,
    key(
      "searcher",
      "search",
      &command("sonnet", &prompt, "one"),
      std::path::Path::new("/logins/b")
    )
  );

  std::fs::write(&prompt, "Search better.").unwrap();

  assert_ne!(first, key("searcher", "search", &command("sonnet", &prompt, "one"), home));
}

#[tokio::test]
async fn a_kept_session_is_taken_by_one_run_while_warm_and_dropped_once_cold() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let now = now_ms();

  keep(db, "k1", "searcher", "search", "session-1", now).await.unwrap();

  assert_eq!(
    take(db, "k1", "claude-code", "sonnet", now).await.unwrap().as_deref(),
    Some("session-1")
  );
  assert_eq!(
    take(db, "k1", "claude-code", "sonnet", now).await.unwrap(),
    None,
    "one run holds it at a time"
  );

  // Kept again by the run that answered, and past the cache lifetime by the next one's start.
  keep(db, "k1", "searcher", "search", "session-2", now - 2 * 3_600_000).await.unwrap();

  assert_eq!(
    take(db, "k1", "claude-code", "sonnet", now).await.unwrap(),
    None,
    "a cold session is not resumed"
  );
  assert_eq!(
    scratch.count(r#"SELECT count(*) FROM "apart_sessions""#, vec![]).await,
    0,
    "and is dropped"
  );
}

#[tokio::test]
async fn the_setting_is_the_role_s_its_agent_holds_or_the_run_s_own_and_none_keeps_nothing() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "position", "created_at", "updated_at", "sessions")
         VALUES ('looking', 'Looking', '[]', 0, 1, 1, 'fresh'), ('keeping', 'Keeping', '[]', 1, 1, 1, 'keep')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "role") VALUES ('looker', 'runner', 'looking'),
           ('keeper', 'runner', 'keeping')"#,
      vec![],
    )
    .await;

  let db = scratch.db();

  assert_eq!(
    setting(db, "looker", Some("keeping")).await,
    Sessions::Fresh,
    "the agent's row answers first"
  );
  assert_eq!(setting(db, "keeper", None).await, Sessions::Keep);
  assert_eq!(
    setting(db, "no-row", Some("keeping")).await,
    Sessions::Keep,
    "a run with no row names its own"
  );
  assert_eq!(setting(db, "no-row", None).await, Sessions::Fresh);
  assert_eq!(
    setting(db, "no-row", Some("nobody")).await,
    Sessions::Fresh,
    "a role the store lacks keeps nothing"
  );
}
