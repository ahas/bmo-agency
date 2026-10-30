use policy::roles::*;

#[test]
fn each_role_gives_its_bundles_and_every_bundle_a_role_names_is_declared() {
  assert_eq!(ROLES.bundles_of("chat"), ["room.chat", "room.send", "web.read"]);
  assert_eq!(ROLES.bundles_of("worker"), ["graph.read", "queue.read", "web.read"]);
  assert_eq!(ROLES.bundles_of("searcher"), ["web.read", "web.sites", "web.branches", "web.rulebooks"]);
  assert!(ROLES.bundles_of("gate").is_empty());
  assert_eq!(ROLES.bundles_of("video"), ["queue.read", "web.read", "room.send"]);
  assert_eq!(ROLES.role("video").and_then(|role| role.prompt.as_deref()), Some("video.md"));
  assert!(ROLES.bundles_of("nobody").is_empty());
  assert!(!ROLES.holds("companion"));
  assert!(ROLES.holds(&ROLES.coworker));
  assert_eq!(ROLES.bundle_label("room.send"), "Send");
  assert!(ROLES.bundles_of("chat").contains(&ROLES.send));

  for role in &ROLES.roles {
    for bundle in &role.bundles {
      assert!(
        ROLES.bundles.iter().any(|one| &one.name == bundle),
        "{} names {bundle}, which is no bundle",
        role.name
      );
    }
  }
}

#[test]
fn each_kind_holds_a_declared_role_where_its_writer_names_none() {
  let kinds: Vec<(&str, &str)> =
    ROLES.kinds.iter().map(|(kind, role)| (kind.as_str(), role.as_str())).collect();

  assert_eq!(
    kinds,
    [("chat", "chat"), ("gate", "gate"), ("runner", "searcher"), ("worker", "worker"),]
  );
  assert!(ROLES.kinds.values().all(|role| ROLES.holds(role)));
}

#[test]
fn every_seeded_role_has_a_name_and_label_a_role_added_on_the_control_port_may_have() {
  let pattern = regex::Regex::new(&ROLES.name_pattern).unwrap();

  for role in &ROLES.roles {
    assert!(
      pattern.is_match(&role.name) && role.name.chars().count() <= ROLES.name_most,
      "{}",
      role.name
    );
    assert!(
      !role.label.is_empty() && role.label.chars().count() <= ROLES.label_most,
      "{}",
      role.label
    );
    assert!(ROLES.bundles.iter().all(|bundle| ROLES.declares_bundle(&bundle.name)));
  }

  for name in ["video", "code-review", "a1-b2"] {
    assert!(pattern.is_match(name), "{name}");
  }
  for name in ["Video", "code_review", "-lead", "trail-", "two--hyphens", "1st"] {
    assert!(!pattern.is_match(name), "{name}");
  }
  assert!(ROLES.kind_default("searcher") && !ROLES.kind_default("video"));
  assert!(!ROLES.declares_bundle("web.everything"));
}
