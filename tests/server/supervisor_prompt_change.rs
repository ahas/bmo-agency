//! A supervisor's session when the documents its prompt is composed from move (docs/supervisors.md, 「A
//! session in each room」): the next turn is composed from the new documents, the session hands itself over
//! and the room's turns and logs stay whole, and a supervisor whose documents did not change keeps its
//! session.

use sha2::{Digest, Sha256};

use crate::support::rows::supervisor;
use crate::support::scratch::Scratch;
use agency_server::lines::Trees;
use agency_server::lines::chat::{ChatDocuments, chat_system};
use agency_server::supervisor_turn::sessions::{self, prompt_moved};

fn hash(text: &str) -> String {
  format!("{:x}", Sha256::digest(text.as_bytes()))
}

// A tree of the chat role's documents, each one line, over which the window's prompt is composed.
fn trees(dir: &std::path::Path) -> Trees {
  let write = |path: &str, text: &str| {
    let at = dir.join(path);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, format!("{text}\n")).unwrap();
  };

  write("prompts/agents/chat/context.md", "the supervisor's opening");
  write("prompts/agents/chat/manual.md", "the supervisor's manual");
  write("prompts/agents/pieces/frame.md", "the frame\n\n{{tools}}");
  write("prompts/agents/pieces/turn.md", "the turn");
  write("prompts/agents/pieces/tools.md", "tools for the turn");
  write("prompts/rules/absolute-rules.md", "the absolute rules");
  for name in ["answer-content", "answer-shape", "plain-writing", "agent-words", "self-preservation"] {
    write(&format!("prompts/rules/{name}.md"), &format!("the {name} rulebook"));
  }

  Trees {
    code: dir.to_path_buf(),
    agents: dir.join("agents"),
    source: dir.to_path_buf(),
    data: dir.join("data"),
    home: dir.to_path_buf(),
  }
}

#[test]
fn the_next_turn_is_composed_from_the_document_that_moved() {
  let dir = tempfile::tempdir().unwrap();
  let trees = trees(dir.path());
  let compose = || chat_system(&trees, &ChatDocuments::default(), None, None, None, None).unwrap();
  let before = compose();

  assert!(before.contains("the supervisor's opening"), "{before}");

  std::fs::write(
    dir.path().join("prompts/agents/chat/context.md"),
    "the supervisor's opening, with the moved line\n",
  )
  .unwrap();

  let after = compose();

  assert!(after.contains("with the moved line"), "{after}");
  assert_ne!(hash(&before), hash(&after), "the composed prompt's hash follows the document");
}

#[test]
fn a_supervisor_whose_documents_did_not_move_keeps_its_prompt() {
  use harness_machine::supervisor_prompt::{Composition, Roots, Supervisor, system};

  let dir = tempfile::tempdir().unwrap();
  let _ = trees(dir.path());
  let roots = Roots {
    assets: dir.path().to_path_buf(),
    data: dir.path().join("data"),
  };
  // The pieces role reads the rulebooks and its own documents, never the chat's.
  let compose = || {
    system(
      &roots,
      Supervisor {
        name: "pieces-agent",
        prompts: None,
      },
      &Composition::default(),
      None,
      false,
    )
    .unwrap()
  };
  let before = compose();

  std::fs::write(
    dir.path().join("prompts/agents/chat/context.md"),
    "the chat's opening, moved, which the pieces role never reads\n",
  )
  .unwrap();

  let after = compose();

  assert_eq!(
    hash(&before),
    hash(&after),
    "a chat document is not part of the pieces supervisor's prompt"
  );
}

#[test]
fn a_prompt_that_moved_hands_over_a_held_session_and_one_that_did_not_keeps_it() {
  // A session the agent already had, whose prompt was recorded.
  assert!(prompt_moved(true, Some("old"), "new"), "a changed prompt hands the session over");
  assert!(
    !prompt_moved(true, Some("new"), "new"),
    "a prompt that did not change keeps the session"
  );
  // A session with no recorded prompt may have read anything, so it hands over once.
  assert!(prompt_moved(true, None, "new"), "an unrecorded prompt hands over once");
  // A generation with no session yet has nothing to fold.
  assert!(!prompt_moved(false, Some("old"), "new"), "a fresh generation keeps its own way");
}

#[tokio::test]
async fn a_hand_over_keeps_the_rooms_conversation_and_logs_whole() {
  let scratch = Scratch::new().await;
  let agent = "kakia-chat";
  let chat = -9_000_000_000_002;

  supervisor(
    scratch.db(),
    agent,
    &[("session_scope", "room".into()), ("turn_shape", "pieces".into())],
  )
  .await;
  scratch
    .run(
      r#"INSERT INTO "room_sessions" ("agent", "chat_id", "generation", "opened_at", "harness_session", "harness_road", "system_hash")
         VALUES ($1, $2, 1, 1, 's-1', 'registered::m', 'old')"#,
      vec![agent.into(), chat.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_session_turns" ("agent", "chat_id", "generation", "position", "role", "text", "at")
         VALUES ($1, $2, 1, 0, 'user', 'what the room said', 1), ($1, $2, 1, 1, 'assistant', 'what it answered', 2)"#,
      vec![agent.into(), chat.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("agent", "run", "chat_id", "at", "kind", "text")
         VALUES ($1, 'chat:1:1', $2, 1, 'text', 'a line the room keeps')"#,
      vec![agent.into(), chat.into()],
    )
    .await;

  let handed = sessions::hand_over(scratch.db(), agent, chat, "the note the old session wrote")
    .await
    .unwrap();

  assert_eq!(handed.generation, 2);
  assert!(handed.harness.is_none(), "a new generation holds no harness session yet");
  assert_eq!(handed.handoff.as_deref(), Some("the note the old session wrote"));

  // Nothing of the room's conversation or logs was taken with it.
  let turns = scratch
    .count(
      r#"SELECT COUNT(*) FROM "room_session_turns" WHERE "agent" = $1 AND "chat_id" = $2 AND "generation" = 1"#,
      vec![agent.into(), chat.into()],
    )
    .await;
  let logs = scratch
    .count(
      r#"SELECT COUNT(*) FROM "agent_log" WHERE "agent" = $1 AND "chat_id" = $2"#,
      vec![agent.into(), chat.into()],
    )
    .await;

  assert_eq!(turns, 2);
  assert_eq!(logs, 1);
}

#[tokio::test]
async fn a_prompt_that_moved_lets_an_agents_one_session_go_and_keeps_its_logs() {
  use agency_server::herdr_backend::{agent_session, keep_agent_session};

  let scratch = Scratch::new().await;
  let agent = "chat-agent";

  supervisor(scratch.db(), agent, &[("session_id", "s-1".into())]).await;
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("agent", "run", "chat_id", "at", "kind", "text")
         VALUES ($1, 'chat:1:1', -5, 1, 'text', 'a line the room keeps')"#,
      vec![agent.into()],
    )
    .await;

  // What a turn whose prompt moved does to the session: it lets it go and records the prompt it will be
  // sent with, leaving the room's logs where they stand.
  keep_agent_session(scratch.db(), agent, None).await.unwrap();
  sessions::mark_agent_system(scratch.db(), agent, "the new prompt").await.unwrap();

  assert_eq!(agent_session(scratch.db(), agent).await.unwrap(), None);
  assert_eq!(
    sessions::agent_system_hash(scratch.db(), agent).await.unwrap().as_deref(),
    Some("the new prompt")
  );
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "agent_log" WHERE "agent" = $1"#, vec![agent.into()])
      .await,
    1
  );
}

#[tokio::test]
async fn a_supervisors_one_session_keeps_the_hash_its_prompt_was_sent_with() {
  let scratch = Scratch::new().await;
  let agent = "chat-agent";

  supervisor(scratch.db(), agent, &[]).await;
  assert_eq!(sessions::agent_system_hash(scratch.db(), agent).await.unwrap(), None);

  sessions::mark_agent_system(scratch.db(), agent, "a hash").await.unwrap();

  assert_eq!(
    sessions::agent_system_hash(scratch.db(), agent).await.unwrap().as_deref(),
    Some("a hash")
  );
}
