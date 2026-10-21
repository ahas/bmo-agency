use agency_server::control::harness::*;
use harness_providers::ARCHIVE_PROVIDERS;

fn names(order: &[OrderEntry]) -> Vec<(&str, bool)> {
  order.iter().map(|one| (one.name.as_str(), one.enabled)).collect()
}

#[test]
fn the_store_keeps_every_provider_this_build_carries() {
  let mut carried: Vec<&str> = harness_providers::providers().iter().map(|one| one.name()).collect();
  let mut stored = ARCHIVE_PROVIDERS.to_vec();

  carried.sort_unstable();
  stored.sort_unstable();
  assert_eq!(carried, stored);
}

#[test]
fn a_saved_order_reads_with_the_providers_it_lacks_appended_on() {
  let read =
    stored_order(Some(r#"[{"name":"codex","enabled":false},{"name":"claude-code","enabled":true}]"#));

  assert_eq!(
    names(&read),
    [
      ("codex", false),
      ("claude-code", true),
      ("openrouter", true),
      ("registered", true),
      ("deepseek", true)
    ]
  );
  assert_eq!(names(&stored_order(Some(r#"["registered","codex"]"#)))[0], ("registered", true));
}

#[test]
fn an_order_of_any_other_shape_reads_as_the_default() {
  let standing = stored_order(None);
  let default = names(&standing);

  assert_eq!(default.len(), 5);
  for stored in [
    "not json",
    r#"{"name":"codex"}"#,
    r#"["codex","codex"]"#,
    r#"["codex",{"name":"registered","enabled":true}]"#,
    r#"["elsewhere"]"#,
    r#"[{"name":"codex","enabled":false},{"name":"claude-code","enabled":false},{"name":"openrouter","enabled":false},{"name":"registered","enabled":false},{"name":"deepseek","enabled":false}]"#,
  ] {
    assert_eq!(names(&stored_order(Some(stored))), default, "{stored}");
  }
}
