//! The translator runner (docs/launch-lines.md, 「Translate runner」; docs/api.md, 「translations」): its system
//! prompt is the translator role's prompt and then the purpose's instructions, its input the target's line and
//! the text, one request with thinking off on the registry's provider, and its route refusing what is no
//! translation.

use std::time::Duration;

use agency_server::lines::Trees;
use agency_server::translate::{
  KEY, LINE, Purpose, ROLE, gateway_request, input, noted_system, system_prompt, too_large,
};
use policy::providers::claude_code::CLAUDE_CODE;
use policy::providers::registered::REGISTERED;
use policy::roles::ROLES;
use policy::run_lines::RUN_LINES;
use policy::work_keys::WORK_KEYS;
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

fn trees() -> Trees {
  let code = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

  Trees {
    code: code.clone(),
    agents: code.join("prompts").join("agents"),
    source: code.clone(),
    data: code.clone(),
    home: code,
  }
}

#[test]
fn a_translation_s_prompt_is_the_role_s_then_its_purpose_s() {
  let role =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/roles/translator.md")).unwrap();

  for (purpose, file) in [
    (Purpose::Document, "instructions.md"),
    (Purpose::Worker, "worker.md"),
    (Purpose::Record, "record.md"),
  ] {
    let own =
      std::fs::read_to_string(format!("{}/prompts/agents/translator/{file}", env!("CARGO_MANIFEST_DIR")))
        .unwrap();
    let system = system_prompt(&trees(), Some(&role), purpose).unwrap();

    assert!(system.starts_with(role.trim()), "{purpose:?}");
    assert!(system.contains(own.trim()), "{purpose:?}");
  }

  assert!(system_prompt(&trees(), None, Purpose::Record).unwrap().contains("Plain writing"));
  assert!(
    system_prompt(&trees(), None, Purpose::Document).unwrap().starts_with(
      std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/prompts/agents/translator/instructions.md"
      ))
      .unwrap()
      .trim()
    )
  );
  assert_eq!(Purpose::read("document"), Some(Purpose::Document));
  assert_eq!(Purpose::read("gate"), None);
  assert_eq!(input(" Korean ", "the text"), "Translate into Korean.\n\nthe text");
  // The caller's note stands in the system prompt, never in the input a translation model would render.
  assert_eq!(noted_system("the translator's prompt\n", None), "the translator's prompt\n");
  assert_eq!(
    noted_system("the translator's prompt\n", Some(" Keep it to one line. ")),
    "the translator's prompt\n\nA note on the part that follows, for you alone: Keep it to one line."
  );

  // A refusal for a part past the model's window is told apart from any other failure.
  assert!(too_large(
    "the gateway answered 400: \"This model's maximum context length is 32768 tokens\""
  ));
  assert!(too_large("Prompt is too long"));
  assert!(!too_large("the gateway answered no words"));
}

#[test]
fn the_translator_is_a_runner_with_a_role_of_its_own_and_a_line_two_providers_carry() {
  let role = ROLES.role(ROLE).expect("the translator role");

  assert!(role.bundles.is_empty(), "no web, no tool");
  assert_eq!(role.prompt.as_deref(), Some("translator.md"));

  let profile = WORK_KEYS.profile(KEY).expect("the translate key");

  assert_eq!((profile.kind.as_str(), profile.line.as_str()), ("runner", LINE));
  assert_eq!(profile.default_route_mode, "registered", "the registered model first");
  assert!(RUN_LINES.line(LINE).is_some());
  assert!(REGISTERED.declared.carries(LINE) && CLAUDE_CODE.declared.carries(LINE));
}

#[test]
fn on_the_registry_s_provider_a_translation_is_one_request_with_thinking_off() {
  let request = gateway_request("a-model", "the system", "Translate into English.\n\nthe text");

  assert_eq!(request["model"], "a-model");
  assert_eq!(
    request["messages"],
    json!([
      { "role": "system", "content": "the system" },
      { "role": "user", "content": "Translate into English.\n\nthe text" },
    ])
  );
  assert_eq!(request["chat_template_kwargs"]["enable_thinking"], false);
  assert_eq!(request["max_tokens"], 2048);
  assert_eq!(request["temperature"], 0.3);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_route_refuses_what_is_no_translation() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let refused = |body: Value| {
    let port = server.port;

    async move {
      let answer = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/translation-runs"))
        .bearer_auth(token)
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .unwrap();

      (answer.status().as_u16(), answer.json::<Value>().await.unwrap_or(Value::Null))
    }
  };

  let (status, said) = refused(json!({ "purpose": "gate", "language": "English", "text": "x" })).await;

  assert_eq!((status, said["code"].as_str()), (400, Some("bad-request")), "{said}");

  let (status, said) = refused(json!({ "purpose": "document", "language": "English", "text": " " })).await;

  assert_eq!((status, said["code"].as_str()), (400, Some("bad-request")), "{said}");
}
