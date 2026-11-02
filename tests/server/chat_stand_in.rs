//! The chat's turn across rooms on a stand-in (docs/cache-fallback.md, 「While a fallback stands, the room is
//! not silent」): the turn composed for Codex on the kept thread and for OpenRouter's loop through a tap, the
//! turn's variables handed to the run and kept out of the program's own, none of Claude Code's launch
//! variables; a walk that had to answer and met no stand-in failing the turn, one that need not leaving it to
//! Claude Code; and a standing fallback cleared and its end published once Claude Code answers again.

use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::PermissionsExt;

use agency_server::Service;
use agency_server::chat_stand_in::{FallbackEnded, fallback_ended, stand_in_turn};
use agency_server::companion::run::NoAnswer;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::lines::chat::{ChatRun, Door, RoomContext, claude_folder, compose_stand_in};
use agency_server::lines::standing::GatewayModel;
use agency_server::lines::worker::Reasoning;
use agency_server::places::Ground;
use agency_server::run_log::{RunLog, Target};
use agency_server::stand_in::Step;
use harness_providers::Machine;
use policy::providers::claude_code::CLAUDE_CODE;
use sea_orm::DatabaseConnection;

use crate::support::scratch::Scratch;

fn ground_on(db: DatabaseConnection, home: &std::path::Path, set: &[(&str, &str)]) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service = Service::new(db, Config::read(|name| variables.get(name).cloned()).unwrap());
  let mut env = vec![
    ("HOME".to_owned(), home.to_string_lossy().into_owned()),
    ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
  ];

  env.extend(set.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())));

  Ground::new(&service, &Machine::with_env(env, home.join("data")), None)
}

// The chat's run as the chat line composed it for a turn, with one of Claude Code's launch variables and
// its thinking switch among its variables.
fn chat_run(home: &std::path::Path) -> ChatRun {
  let launch = CLAUDE_CODE.launch.variables.keys().next().cloned().unwrap_or_default();

  // The chat line's one MCP configuration, which Codex's command reads its servers out of, and the
  // system prompt dsh's overlay carries as its persona.
  std::fs::write(
    home.join("chat.mcp.json"),
    r#"{"mcpServers":{"bmo":{"type":"http","url":"http://127.0.0.1:3290/mcp/bmo","headers":{"Authorization":"Bearer ${BMO_MCP_TOKEN}"}}}}"#,
  )
  .unwrap();
  std::fs::write(home.join("system.md"), "You are the chat agent.\n").unwrap();

  ChatRun {
    line: "chat",
    agent: "chat-agent".into(),
    model: "claude-model".into(),
    effort: "high".into(),
    tier: None,
    reasoning: Reasoning::default(),
    thinking: true,
    system_path: home.join("system.md"),
    system_parts: None,
    room: RoomContext {
      chat_id: -5,
      text: "the room's block".into(),
      mark: "[a]".into(),
    },
    tools_shown: "Bash,Read".into(),
    tools_allowed: "Bash,Read,mcp__bmo".into(),
    mcp_config: home.join("chat.mcp.json"),
    mcp_tools: None,
    prompt: "what is new".into(),
    persona_unused: None,
    fold: false,
    ping: false,
    resume: Some("claude-session".into()),
    cwd: home.to_string_lossy().into_owned(),
    env: BTreeMap::from([
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("BMO_TURN_CHAT".to_owned(), "-5".to_owned()),
      (launch, "1".to_owned()),
      ("MAX_THINKING_TOKENS".to_owned(), "0".to_owned()),
    ]),
    cards: Default::default(),
  }
}

fn step(provider: &str, model: &str) -> Step {
  Step {
    provider: provider.into(),
    model: model.into(),
    effort: "medium".into(),
    thinking: true,
    answer_tokens: None,
  }
}

#[tokio::test]
async fn a_codex_step_goes_out_on_the_kept_thread_with_the_turns_variables_beside_the_command() {
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(DatabaseConnection::Disconnected, home.path(), &[]);
  let run = chat_run(home.path());
  let thread = Door::Codex {
    thread: Some("thread-1".into()),
  };
  let composed =
    compose_stand_in(&ground, &ground.machine(), &run, &step("codex", "gpt-x"), &thread, &[7, 8], 1_000)
      .await
      .unwrap();
  let args = &composed.command.args;
  let handed = composed.run_env.clone().unwrap();
  let launch = CLAUDE_CODE.launch.variables.keys().next().unwrap();

  assert_eq!(args[0], "codex");
  assert!(args.windows(2).any(|pair| pair == ["resume", "thread-1"]), "{args:?}");
  assert!(args.windows(2).any(|pair| pair == ["--model", "gpt-x"]), "{args:?}");
  assert_eq!(composed.session, "thread-1");
  assert_eq!((composed.model.as_str(), composed.effort.as_deref()), ("gpt-x", Some("medium")));
  assert_eq!(handed.get("BMO_TURN_CHAT").map(String::as_str), Some("-5"));
  assert_eq!(handed.get("BMO_TURN_MESSAGES").map(String::as_str), Some("7,8"));
  assert_eq!(handed.get("BMO_TURN_BEGAN").map(String::as_str), Some("1000"));
  assert!(!handed.contains_key(launch) && !handed.contains_key("MAX_THINKING_TOKENS"));
  assert!(
    !composed.command.env.contains_key("BMO_TURN_CHAT"),
    "the program's identity holds no turn"
  );

  let given = composed.given.unwrap();

  assert_eq!(
    (given["prompt"].as_str(), given["resumeSessionId"].as_str()),
    (Some("what is new"), Some("thread-1"))
  );
  assert_eq!(
    (given["model"].as_str(), given["promptDelivery"].as_str()),
    (Some("gpt-x"), Some("input"))
  );
}

#[tokio::test]
async fn a_codex_step_with_no_kept_thread_opens_one() {
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(DatabaseConnection::Disconnected, home.path(), &[]);
  let composed = compose_stand_in(
    &ground,
    &ground.machine(),
    &chat_run(home.path()),
    &step("codex", "gpt-x"),
    &Door::Codex { thread: None },
    &[],
    1,
  )
  .await
  .unwrap();

  assert!(!composed.command.args.iter().any(|arg| arg == "resume"));
  assert_eq!(composed.given.unwrap()["resumeSessionId"], serde_json::Value::Null);
}

#[tokio::test]
async fn an_openrouter_member_step_stands_claude_code_on_the_gateway() {
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(DatabaseConnection::Disconnected, home.path(), &[]);
  let home_dir = ground.roots.data.join("claude-roads/openrouter/vendor-model");
  let key = ground.roots.data.join("env/llm-gateway.key");

  std::fs::create_dir_all(key.parent().unwrap()).unwrap();
  std::fs::write(&key, "sk-gateway\n").unwrap();
  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();

  let held = GatewayModel {
    name: "vendor/model".into(),
    label: "Vendor Model".into(),
    home: home_dir.clone(),
    context_tokens: 131_072,
    answer_tokens: 16_384,
    thinking: true,
    registration: None,
    road: Some("openrouter"),
  };
  let composed = compose_stand_in(
    &ground,
    &ground.machine(),
    &chat_run(home.path()),
    &step("openrouter", "vendor/model"),
    &Door::Gateway {
      held: Box::new(held),
      placement: None,
    },
    &[3],
    1,
  )
  .await
  .unwrap();
  let args = &composed.command.args;
  let env = &composed.command.env;

  assert_eq!(args.first().map(String::as_str), Some("claude"));
  assert_eq!(env.get("ANTHROPIC_MODEL").map(String::as_str), Some("vendor/model"));
  assert_eq!(env.get("ANTHROPIC_BASE_URL").map(String::as_str), Some("http://127.0.0.1:18030"));
  assert_eq!(
    env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
    Some(home_dir.to_string_lossy().as_ref())
  );
  assert_eq!(composed.window.map(|window| window.context_tokens), Some(131_072));
}
// A store whose provider order holds the stand-ins alone, or Claude Code beside them, for the chat agent.
async fn store_with(order: &[(&str, bool)]) -> Scratch {
  let scratch = Scratch::new().await;
  let entries: Vec<OrderEntry> = order
    .iter()
    .map(|(name, enabled)| OrderEntry {
      name: (*name).to_owned(),
      enabled: *enabled,
    })
    .collect();

  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ('chat-agent', 'chat')"#, vec![])
    .await;
  set_provider_order(scratch.db(), &entries).await.unwrap();
  scratch
}

fn log() -> RunLog {
  RunLog::new(Target {
    agent: "chat-agent".into(),
    run: "chat:-5:1".into(),
    chat_id: Some(-5),
    job_id: None,
  })
}

#[tokio::test]
async fn a_walk_that_must_answer_and_meets_no_stand_in_fails_the_turn() {
  let home = tempfile::tempdir().unwrap();
  let scratch =
    store_with(&[("claude-code", false), ("codex", true), ("openrouter", true), ("registered", true)])
      .await;
  let ground = ground_on(scratch.db().clone(), home.path(), &[]);
  let walked = stand_in_turn(&ground, &mut log(), &chat_run(home.path()), "chat-agent", &[], 1, None).await;

  assert!(matches!(walked, Err(NoAnswer::Failed(_))), "{walked:?}");
}

#[tokio::test]
async fn a_walk_that_meets_no_stand_in_before_an_open_claude_leaves_the_turn_to_claude() {
  let home = tempfile::tempdir().unwrap();
  let scratch =
    store_with(&[("claude-code", true), ("codex", true), ("openrouter", true), ("registered", true)]).await;
  let ground = ground_on(scratch.db().clone(), home.path(), &[]);
  let walked = stand_in_turn(&ground, &mut log(), &chat_run(home.path()), "chat-agent", &[], 1, None).await;

  assert!(matches!(walked, Ok(None)), "{walked:?}");
}

#[tokio::test]
async fn claude_answering_again_clears_a_standing_fallback_and_publishes_its_end_once() {
  let home = tempfile::tempdir().unwrap();
  let scratch = Scratch::new().await;
  let ground = ground_on(scratch.db().clone(), home.path(), &[("BMO_OWNER_ID", "42")]);
  let mut ended = ground.events.subscribe::<FallbackEnded>();

  fallback_ended(&ground).await;
  assert_eq!(ended.ready(), None, "no fallback stood");

  for (key, value) in [("fallback_since", "1"), ("fallback_provider", "codex")] {
    scratch
      .run(
        r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)"#,
        vec![key.into(), value.into()],
      )
      .await;
  }

  fallback_ended(&ground).await;

  assert_eq!(
    ended.ready(),
    Some(FallbackEnded {
      owner_id: 42,
      previous: Some("codex".into())
    })
  );
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "settings" WHERE "key" LIKE 'fallback_%'"#, vec![])
      .await,
    0
  );

  fallback_ended(&ground).await;
  assert_eq!(ended.ready(), None);
}

#[test]
fn the_chat_s_own_launch_stands_on_claude_code_in_its_agent_s_home_and_a_named_folder_in_that_folder() {
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(DatabaseConnection::Disconnected, home.path(), &[]);
  let named = chat_run(home.path());

  // A caller's folder is the caller's.
  assert_eq!(claude_folder(&ground, &named).unwrap(), named.cwd);

  // The chat's own launch names the deploy tree's link, which on Claude Code is the agent's own home instead, a
  // real folder made as it is first needed.
  let own = ChatRun {
    cwd: ground.roots.deploy.to_string_lossy().into_owned(),
    ..named
  };
  let folder = claude_folder(&ground, &own).unwrap();

  assert_eq!(
    std::path::PathBuf::from(&folder),
    ground.roots.data.join("companion-homes").join("chat-agent")
  );
  assert!(std::path::Path::new(&folder).is_dir());
}
