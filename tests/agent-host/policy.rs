//! The provider order read at a child's start, as the launcher reads it.

use agent_host::policy::provider_on;

fn known() -> Vec<String> {
  ["claude-code", "codex", "openrouter", "registered", "gained"]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn an_order_is_read_as_the_launcher_reads_it() {
  let order = r#"[{"name":"claude-code","enabled":true},{"name":"codex","enabled":false}]"#;

  assert!(provider_on(Some(order), "claude-code", None));
  assert!(!provider_on(Some(order), "codex", None));
  // Saved before the build gained it: on.
  assert!(provider_on(Some(order), "registered", Some(&known())));
  assert!(provider_on(None, "codex", None));
  assert!(provider_on(Some("not json"), "codex", None));
  // A name twice, or a provider the build does not know: every provider on.
  assert!(provider_on(
    Some(r#"[{"name":"codex","enabled":false},{"name":"codex","enabled":false}]"#),
    "codex",
    None
  ));
  assert!(provider_on(
    Some(r#"[{"name":"gone","enabled":true},{"name":"codex","enabled":false}]"#),
    "codex",
    Some(&known())
  ));
  // Plain names are every one on.
  assert!(provider_on(Some(r#"["codex"]"#), "codex", None));
}

#[test]
fn an_order_leaving_none_on_reads_as_every_provider_on() {
  let all = known();
  let off: Vec<String> = all.iter().map(|name| format!(r#"{{"name":"{name}","enabled":false}}"#)).collect();
  let order = format!("[{}]", off.join(","));

  assert!(provider_on(Some(&order), "codex", Some(&all)));
  assert!(provider_on(Some(&order), "codex", None));
}
