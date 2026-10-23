use policy::roles::*;

#[test]
fn each_role_gives_its_bundles_and_every_bundle_a_role_names_is_declared() {
  assert_eq!(ROLES.bundles_of("chat"), ["room.chat", "room.send", "web.read"]);
  assert_eq!(ROLES.bundles_of("worker"), ["graph.read", "queue.read", "web.read"]);
  assert_eq!(
    ROLES.bundles_of("searcher"),
    ["web.read", "web.sites", "web.images", "web.branches", "web.rulebooks"]
  );
  assert!(ROLES.bundles_of("gate").is_empty());
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
