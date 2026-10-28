//! The gateway's table written from the registry, and the digest that decides whether a running child
//! serves it.

use std::collections::{BTreeMap, HashMap};

use llm_gateway::table::{HOOKS, Member, Table, gateway_table, gateway_table_with_members, table_digest};
use serde_json::{Value, json};
use store::registry::{RegisteredAddress, Registration};

fn registration(name: &str, addresses: Vec<RegisteredAddress>) -> Registration {
  Registration {
    name: name.into(),
    label: name.into(),
    model: "served-model".into(),
    context_tokens: 32_768,
    answer_tokens: 4_096,
    wait_seconds: 90,
    thinking: false,
    walked: false,
    keeps_research: false,
    addresses,
  }
}

fn address(node: &str, url: &str, key_file: Option<&str>) -> RegisteredAddress {
  RegisteredAddress {
    node: node.into(),
    url: url.into(),
    key_file: key_file.map(str::to_owned),
    key_owned: false,
    placed_at: None,
    role_files: false,
  }
}

fn nodes() -> Registration {
  Registration {
    model: "RVN-model".into(),
    wait_seconds: 2_400,
    ..registration(
      "nodes",
      vec![
        address("alpha", "http://127.0.0.1:18021/v1/chat/completions", Some("alpha-llm.token")),
        address("bravo", "http://127.0.0.1:18023/v1/chat/completions", None),
      ],
    )
  }
}

fn single() -> Registration {
  registration(
    "single",
    vec![address(
      "d-vex",
      "https://llm.example.com/v1/chat/completions",
      Some("registered/single-d-vex.key"),
    )],
  )
}

fn table() -> Table {
  gateway_table(&[nodes(), single(), registration("addressless", vec![])])
}

#[test]
fn each_registration_is_served_under_its_name_and_each_node_of_several_on_its_own() {
  let config: Value = serde_json::from_str(&table().text).unwrap();
  let served: Vec<(String, String)> = config["model_list"]
    .as_array()
    .unwrap()
    .iter()
    .map(|entry| {
      (
        entry["model_name"].as_str().unwrap().to_owned(),
        entry["litellm_params"]["api_base"].as_str().unwrap().to_owned(),
      )
    })
    .collect();
  let named = |name: &str, base: &str| (name.to_owned(), base.to_owned());
  assert_eq!(
    served,
    [
      named("nodes", "http://127.0.0.1:18021/v1"),
      named("nodes@alpha", "http://127.0.0.1:18021/v1"),
      named("nodes", "http://127.0.0.1:18023/v1"),
      named("nodes@bravo", "http://127.0.0.1:18023/v1"),
      named("single", "https://llm.example.com/v1"),
    ]
  );
}

#[test]
fn each_address_is_asked_for_the_model_with_its_wait_and_thinking_off_and_keys_named_by_variable() {
  let table = table();
  let config: Value = serde_json::from_str(&table.text).unwrap();
  assert_eq!(
    config["model_list"][0]["litellm_params"],
    json!({
      "model": "openai/RVN-model",
      "api_base": "http://127.0.0.1:18021/v1",
      "api_key": "os.environ/BMO_KEY_ALPHA_LLM_TOKEN",
      "timeout": 2400,
      "stream_timeout": 2400,
      "chat_template_kwargs": { "enable_thinking": false },
    })
  );
  assert_eq!(config["model_list"][2]["litellm_params"]["api_key"], "no-key");
  assert_eq!(
    table.key_files,
    BTreeMap::from([
      ("BMO_KEY_ALPHA_LLM_TOKEN".to_owned(), "alpha-llm.token".to_owned()),
      (
        "BMO_KEY_REGISTERED_SINGLE_D_VEX_KEY".to_owned(),
        "registered/single-d-vex.key".to_owned()
      ),
    ])
  );
  assert_eq!(
    config["litellm_settings"],
    json!({
      "drop_params": true,
      "use_chat_completions_url_for_anthropic_messages": true,
      "callbacks": ["thinking.handler", "sampling.handler", "reminders.handler", "long_tools.handler", "roles.handler"],
      "num_retries": 0,
    })
  );
  assert_eq!(config["general_settings"], json!({ "master_key": "os.environ/LLM_GATEWAY_KEY" }));
}

#[test]
fn the_table_is_written_in_its_fixed_form_byte_for_byte() {
  // The fixture is the table these registrations give, held byte for byte: its key order, its
  // indentation and its closing line break are what LiteLLM is started on.
  assert_eq!(table().text, include_str!("fixtures/table.json"));
}

#[test]
fn a_key_given_again_is_a_new_table_and_the_same_table_otherwise() {
  let table = table();
  let mut keys = HashMap::from([("alpha-llm.token", "first"), ("registered/single-d-vex.key", "k")]);
  let digest = |keys: &HashMap<&str, &str>, table: &Table| {
    table_digest(table, |file| keys.get(file).map(|key| (*key).to_owned()), &[])
  };
  let before = digest(&keys, &table);
  assert_eq!(digest(&keys, &table), before);
  assert_eq!(digest(&keys, &gateway_table(&[nodes(), single()])), before);
  keys.insert("alpha-llm.token", "second");
  assert_ne!(digest(&keys, &table), before);
  assert!(!table.text.contains("first"));
}

#[test]
fn a_changed_hook_is_a_new_table() {
  let table = table();
  let hooks = |reminders: &str| -> Vec<(String, Vec<u8>)> {
    HOOKS
      .iter()
      .map(|hook| {
        (
          (*hook).to_owned(),
          if *hook == "reminders" { reminders } else { "same" }.as_bytes().to_vec(),
        )
      })
      .collect()
  };
  let digest = |hooks: &[(String, Vec<u8>)]| table_digest(&table, |_| None, hooks);
  assert_eq!(digest(&hooks("first")), digest(&hooks("first")));
  assert_ne!(digest(&hooks("first")), digest(&hooks("second")));
}

#[test]
fn an_address_that_keeps_role_files_says_so_on_every_entry_it_stands_behind_and_no_other_does() {
  let mut kept = address("alpha", "http://127.0.0.1:18021/v1/chat/completions", None);

  kept.role_files = true;

  let table = gateway_table(&[registration(
    "nodes",
    vec![kept, address("bravo", "http://127.0.0.1:18023/v1/chat/completions", None)],
  )]);
  let read: serde_json::Value = serde_json::from_str(&table.text).unwrap();
  let infos: Vec<(String, serde_json::Value)> = read["model_list"]
    .as_array()
    .unwrap()
    .iter()
    .map(|entry| (entry["model_name"].as_str().unwrap().to_owned(), entry["model_info"].clone()))
    .collect();
  let on = serde_json::json!({ "role_files": true });

  assert_eq!(
    infos,
    [
      ("nodes".to_owned(), on.clone()),
      ("nodes@alpha".to_owned(), on),
      ("nodes".to_owned(), serde_json::Value::Null),
      ("nodes@bravo".to_owned(), serde_json::Value::Null),
    ]
  );
  assert!(
    read["litellm_settings"]["callbacks"]
      .as_array()
      .unwrap()
      .contains(&"roles.handler".into())
  );
}

#[test]
fn members_are_served_under_their_own_names_on_their_providers() {
  let members = [
    Member {
      name: "deepseek-flash".into(),
      road: Some("deepseek".into()),
      label: Some("DeepSeek V4.1 Flash".into()),
      provider: "deepseek".into(),
      model: "deepseek-flash".into(),
      api_base: Some("https://api.deepseek.com/v1".into()),
      key_file: "deepseek.key".into(),
      wait_seconds: 300,
      context_tokens: Some(1_048_576),
      answer_tokens: Some(32_768),
      thinking: Some(true),
    },
    Member {
      name: "openrouter-deepseek-flash".into(),
      road: Some("openrouter".into()),
      label: Some("DeepSeek V4.1 Flash on OpenRouter".into()),
      provider: "openrouter".into(),
      model: "deepseek/deepseek-v4.1-flash-1017".into(),
      api_base: Some("https://openrouter.ai/api/v1".into()),
      key_file: "openrouter.key".into(),
      wait_seconds: 300,
      context_tokens: Some(1_048_576),
      answer_tokens: Some(32_768),
      thinking: Some(true),
    },
  ];
  let table = gateway_table_with_members(&[], &members).unwrap();
  let config: Value = serde_json::from_str(&table.text).unwrap();
  let entries = config["model_list"].as_array().unwrap();

  assert_eq!(entries.len(), 2);
  assert_eq!(entries[0]["model_name"], "deepseek-flash");
  assert_eq!(entries[0]["litellm_params"]["model"], "deepseek/deepseek-flash");
  assert_eq!(entries[1]["model_name"], "openrouter-deepseek-flash");
  assert_eq!(entries[1]["litellm_params"]["model"], "openrouter/deepseek/deepseek-v4.1-flash-1017");
  assert_eq!(entries[1]["litellm_params"]["api_key"], "os.environ/BMO_KEY_OPENROUTER_KEY");
  assert_eq!(
    table.key_files.get("BMO_KEY_OPENROUTER_KEY").map(String::as_str),
    Some("openrouter.key")
  );
}

#[test]
fn a_member_naming_a_registration_refuses_the_table() {
  let member = Member {
    name: "taken".into(),
    road: None,
    label: None,
    provider: "deepseek".into(),
    model: "taken-model".into(),
    api_base: None,
    key_file: "deepseek.key".into(),
    wait_seconds: 300,
    context_tokens: None,
    answer_tokens: None,
    thinking: None,
  };
  let refused = gateway_table_with_members(&[registration("taken", vec![])], &[member]);

  assert!(refused.is_err());
}
