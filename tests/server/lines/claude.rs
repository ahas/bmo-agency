use agency_server::lines::claude::*;

#[test]
fn a_directory_login_clears_any_token_and_a_token_login_hands_its_token() {
  let dir = tempfile::tempdir().unwrap();
  let token = dir.path().join("token");

  std::fs::write(&token, "sk-test\n").unwrap();

  let account = |kind: &str, path: &Path| Account {
    id: "a".into(),
    name: "a".into(),
    place: 0,
    enabled: true,
    credential_kind: kind.into(),
    credential_path: path.to_string_lossy().into_owned(),
    needs_login_at: None,
    user_id: None,
  };
  let directory = account_env(Some(&account("directory", dir.path())), &Runs::Here).unwrap();
  let tokened = account_env(Some(&account("token", &token)), &Runs::Here).unwrap();

  assert_eq!(directory["CLAUDE_CODE_OAUTH_TOKEN"], None);
  assert_eq!(directory["CLAUDE_CONFIG_DIR"].as_deref(), dir.path().to_str());
  assert_eq!(tokened["CLAUDE_CODE_OAUTH_TOKEN"].as_deref(), Some("sk-test"));
  assert!(account_env(Some(&account("token", &dir.path().join("gone"))), &Runs::Here).is_err());
}

#[test]
fn a_directory_login_on_a_node_is_handed_its_token_for_the_nodes_beside_its_directory() {
  let data = tempfile::tempdir().unwrap();
  let login = tempfile::tempdir().unwrap();
  let account = Account {
    id: "spare".into(),
    name: "spare login".into(),
    place: 0,
    enabled: true,
    credential_kind: "directory".into(),
    credential_path: login.path().to_string_lossy().into_owned(),
    needs_login_at: None,
    user_id: None,
  };
  let place = data.path().join("agent-places").join("worker");
  let runs = || Runs::of_place(data.path(), &place);

  std::fs::create_dir_all(&place).unwrap();
  assert_eq!(runs(), Runs::Here);
  agency_server::watches::links::mark_place_node(&place, Some("node-a")).unwrap();

  // No token made for the nodes yet: the directory alone, the node's own login in it.
  let bare = account_env(Some(&account), &runs()).unwrap();

  assert_eq!(bare["CLAUDE_CODE_OAUTH_TOKEN"], None);

  let file = account.node_token_file(data.path());

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
  let env = line_environment(&command, "bmo-standby-worker");

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
