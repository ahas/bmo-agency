use std::collections::BTreeMap;
use std::path::Path;

use agency_server::control::claude_accounts::{Account, Credential, EndpointModels};
use agency_server::lines::claude::*;

fn account(id: &str, credential: Credential) -> Account {
  Account {
    id: id.into(),
    name: id.into(),
    place: 0,
    enabled: true,
    credential,
    needs_login_at: None,
    user_id: None,
    base_prompt: None,
  }
}

#[test]
fn a_directory_login_clears_any_token_and_a_token_login_hands_its_token() {
  let dir = tempfile::tempdir().unwrap();
  let token = dir.path().join("token");

  std::fs::write(&token, "sk-test\n").unwrap();

  let directory = |home: &Path| account("a", Credential::Directory { home: home.into() });
  let tokened = |file: &Path| account("a", Credential::Token { file: file.into() });
  let directory = account_env(Some(&directory(dir.path())), &Runs::Here).unwrap();
  let handed = account_env(Some(&tokened(&token)), &Runs::Here).unwrap();

  assert_eq!(directory["CLAUDE_CODE_OAUTH_TOKEN"], None);
  assert_eq!(directory["CLAUDE_CONFIG_DIR"].as_deref(), dir.path().to_str());
  assert_eq!(handed["CLAUDE_CODE_OAUTH_TOKEN"].as_deref(), Some("sk-test"));
  assert_eq!(handed.len(), 1);
  assert!(account_env(Some(&tokened(&dir.path().join("gone"))), &Runs::Here).is_err());
}

#[test]
fn an_endpoint_login_hands_its_home_its_url_its_key_and_its_models_and_clears_every_other_credential() {
  let dir = tempfile::tempdir().unwrap();
  let key = dir.path().join("endpoint.key");
  let endpoint = |key_file: &Path| {
    account(
      "endpoint",
      Credential::Endpoint {
        home: dir.path().join("home"),
        url: "https://endpoint.test/anthropic".into(),
        key_file: key_file.into(),
        models: EndpointModels {
          opus: Some("big-model".into()),
          sonnet: Some("small-model".into()),
          haiku: None,
        },
      },
    )
  };

  std::fs::write(&key, "sk-endpoint\n").unwrap();

  let env = account_env(Some(&endpoint(&key)), &Runs::Here).unwrap();

  assert_eq!(env["CLAUDE_CONFIG_DIR"].as_deref(), dir.path().join("home").to_str());
  assert_eq!(env["ANTHROPIC_BASE_URL"].as_deref(), Some("https://endpoint.test/anthropic"));
  assert_eq!(env["ANTHROPIC_AUTH_TOKEN"].as_deref(), Some("sk-endpoint"));
  assert_eq!(env["ANTHROPIC_DEFAULT_OPUS_MODEL"].as_deref(), Some("big-model"));
  assert_eq!(env["ANTHROPIC_DEFAULT_SONNET_MODEL"].as_deref(), Some("small-model"));
  assert_eq!(env["ANTHROPIC_DEFAULT_HAIKU_MODEL"], None);
  assert_eq!(env["ANTHROPIC_API_KEY"], None);
  assert_eq!(env["CLAUDE_CODE_OAUTH_TOKEN"], None);

  // A key that cannot be read is refused, and so is a run from a work node.
  std::fs::write(dir.path().join("empty.key"), "\n").unwrap();
  assert!(account_env(Some(&endpoint(&dir.path().join("empty.key"))), &Runs::Here).is_err());
  assert!(account_env(Some(&endpoint(&dir.path().join("gone.key"))), &Runs::Here).is_err());

  let node = Runs::OnNode {
    data_root: dir.path().into(),
  };

  assert!(account_env(Some(&endpoint(&key)), &node).is_err());

  // Over the launching process's own, the endpoint's credentials are what the program is handed.
  let process = BTreeMap::from([
    ("ANTHROPIC_API_KEY".to_owned(), "sk-process".to_owned()),
    ("CLAUDE_CODE_OAUTH_TOKEN".to_owned(), "sk-ant-oat01-process".to_owned()),
  ]);
  let program = command_env(&process, Some(&endpoint(&key)), &Runs::Here, true).unwrap();

  assert_eq!(program["ANTHROPIC_API_KEY"], None);
  assert_eq!(program["CLAUDE_CODE_OAUTH_TOKEN"], None);
  assert_eq!(program["ANTHROPIC_AUTH_TOKEN"].as_deref(), Some("sk-endpoint"));
}

#[test]
fn a_pane_on_an_endpoint_account_is_handed_its_url_its_key_and_its_models_by_name() {
  let dir = tempfile::tempdir().unwrap();
  let key = dir.path().join("endpoint.key");

  std::fs::write(&key, "sk-endpoint\n").unwrap();

  let endpoint = account(
    "endpoint",
    Credential::Endpoint {
      home: dir.path().join("home"),
      url: "https://endpoint.test/anthropic".into(),
      key_file: key.clone(),
      models: EndpointModels {
        opus: Some("big-model".into()),
        sonnet: None,
        haiku: Some("tiny-model".into()),
      },
    },
  );
  let process = BTreeMap::from([("PATH".to_owned(), "/bin".to_owned())]);
  let command = Command {
    args: vec![],
    cwd: "/w".into(),
    env: command_env(&process, Some(&endpoint), &Runs::Here, true).unwrap(),
  };
  let env = line_environment(&command, "bmo-standby-worker", Some(&endpoint));

  assert_eq!(env["CLAUDE_CONFIG_DIR"], dir.path().join("home").to_string_lossy());
  assert_eq!(env["ANTHROPIC_BASE_URL"], "https://endpoint.test/anthropic");
  assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "sk-endpoint");
  assert_eq!(env["ANTHROPIC_DEFAULT_OPUS_MODEL"], "big-model");
  assert_eq!(env["ANTHROPIC_DEFAULT_HAIKU_MODEL"], "tiny-model");
  assert!(!env.contains_key("ANTHROPIC_DEFAULT_SONNET_MODEL"));
  assert!(!env.contains_key("ANTHROPIC_API_KEY"));
  assert!(!env.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
}

#[test]
fn a_directory_login_on_a_node_is_handed_its_token_for_the_nodes_beside_its_directory() {
  let data = tempfile::tempdir().unwrap();
  let login = tempfile::tempdir().unwrap();
  let account = account(
    "spare",
    Credential::Directory {
      home: login.path().into(),
    },
  );
  let place = data.path().join("agent-places").join("worker");
  let runs = || Runs::of_place(data.path(), &place);

  std::fs::create_dir_all(&place).unwrap();
  assert_eq!(runs(), Runs::Here);
  agency_server::watches::links::mark_place_node(&place, Some("node-a")).unwrap();

  // No token made for the nodes yet: the directory alone, the node's own login in it.
  let bare = account_env(Some(&account), &runs()).unwrap();

  assert_eq!(bare["CLAUDE_CODE_OAUTH_TOKEN"], None);

  let file = account.node_token_file(data.path()).unwrap();

  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, "sk-ant-oat01-node\n").unwrap();

  let on_node = account_env(Some(&account), &runs()).unwrap();
  let here = account_env(Some(&account), &Runs::Here).unwrap();

  assert_eq!(on_node["CLAUDE_CODE_OAUTH_TOKEN"].as_deref(), Some("sk-ant-oat01-node"));
  assert_eq!(on_node["CLAUDE_CONFIG_DIR"].as_deref(), login.path().to_str());
  assert_eq!(here["CLAUDE_CODE_OAUTH_TOKEN"], None);
}

#[test]
fn the_pane_is_handed_the_lines_variables_by_name_and_never_the_launchers_whole_environment() {
  let process = BTreeMap::from([
    ("PATH".to_owned(), "/bin".to_owned()),
    ("BMO_DATA_DIR".to_owned(), "/data".to_owned()),
    ("BMO_DEPLOY_DIR".to_owned(), "/srv/deploy-releases/0123456789ab".to_owned()),
    ("BMO_TASK_LOCKED".to_owned(), String::new()),
    ("CLAUDECODE".to_owned(), "1".to_owned()),
    ("HOME".to_owned(), "/fixture/home".to_owned()),
  ]);
  let command = Command {
    args: vec![],
    cwd: "/w".into(),
    env: command_env(&process, None, &Runs::Here, false).unwrap(),
  };
  let env = line_environment(&command, "bmo-standby-worker", None);

  assert_eq!(env["PATH"], "/bin");
  assert_eq!(env["BMO_DATA_DIR"], "/data");
  // The job's release reaches the pane, so its hooks and the scripts it calls read the same tree.
  assert_eq!(env["BMO_DEPLOY_DIR"], "/srv/deploy-releases/0123456789ab");
  assert_eq!(env["BMO_RUN_NAME"], "bmo-standby-worker");
  assert_eq!(env["CLAUDE_CODE_DISABLE_GIT_INSTRUCTIONS"], "1");
  assert!(!env.contains_key("CLAUDECODE"));
  assert!(!env.contains_key("HOME"));
  assert!(!env.contains_key("BMO_TASK_LOCKED"));
  assert!(!env.contains_key("MAX_THINKING_TOKENS"));
}

#[test]
fn an_endpoint_launch_maps_the_agent_model_to_its_family_and_refuses_an_unknown_one() {
  let dir = tempfile::tempdir().unwrap();
  let key = dir.path().join("gateway.key");
  std::fs::write(&key, "sk-gateway\n").unwrap();
  let gateway = account(
    "gateway-members",
    Credential::Endpoint {
      home: dir.path().join("home"),
      url: "http://127.0.0.1:18030".into(),
      key_file: key,
      models: EndpointModels {
        opus: Some("deepseek-flash".into()),
        sonnet: Some("openrouter-deepseek-flash".into()),
        haiku: None,
      },
    },
  );
  let command = endpoint_launch(&gateway, "deepseek-flash", "high").unwrap();

  assert_eq!(command.args, ["claude", "--model", "opus", "--effort", "high"]);
  assert_eq!(command.env["ANTHROPIC_BASE_URL"].as_deref(), Some("http://127.0.0.1:18030"));
  assert_eq!(command.env["ANTHROPIC_AUTH_TOKEN"].as_deref(), Some("sk-gateway"));
  assert_eq!(command.env["ANTHROPIC_DEFAULT_OPUS_MODEL"].as_deref(), Some("deepseek-flash"));
  assert_eq!(
    command.env["ANTHROPIC_DEFAULT_SONNET_MODEL"].as_deref(),
    Some("openrouter-deepseek-flash")
  );
  assert_eq!(command.cwd, dir.path().join("home").to_string_lossy());

  let unknown = endpoint_launch(&gateway, "not-in-the-map", "high").unwrap_err();
  assert!(unknown.detail.contains("maps no family"), "{}", unknown.detail);
  let directory = account(
    "a-directory",
    Credential::Directory {
      home: dir.path().into(),
    },
  );
  assert!(endpoint_launch(&directory, "deepseek-flash", "high").is_err());
}
