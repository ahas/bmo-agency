//! The roles policy (docs/agent-roles.md, 「The roles the policy seeds」): the fourteen seeded roles with their
//! lines, caps, sessions, callable marks and orders, the defaults nothing names, the supervisor bundles and the
//! shape of a role's name.

use std::collections::BTreeSet;

use policy::roles::*;
use regex::Regex;

#[test]
fn the_policy_seeds_the_fourteen_roles_in_the_order_a_screen_lists_them() {
  let names: Vec<&str> = ROLES.roles.iter().map(|role| role.name.as_str()).collect();

  assert_eq!(
    names,
    [
      "worker",
      "delegator",
      "helper",
      "video",
      "builder",
      "verifier",
      "digest",
      "scout",
      "screener",
      "searcher",
      "browser",
      "translator",
      "transliterator",
      "upkeep"
    ]
  );
}

#[test]
fn each_role_starts_at_the_cap_the_owner_decided() {
  let caps: Vec<(&str, i64)> =
    ROLES.roles.iter().map(|role| (role.name.as_str(), role.max_concurrent)).collect();

  for (name, cap) in [
    ("worker", 3),
    ("delegator", 2),
    ("helper", 2),
    ("builder", 8),
    ("verifier", 6),
    ("digest", 6),
    ("scout", 12),
    ("screener", 12),
    ("upkeep", 1),
  ] {
    assert!(caps.contains(&(name, cap)), "{name} starts at {cap}: {caps:?}");
  }
}

#[test]
fn the_worker_goes_out_on_deepseeks_flash_model_first() {
  let worker = ROLES.role("worker").unwrap();

  assert_eq!(
    (worker.order[0].provider.as_str(), worker.order[0].model.as_str()),
    ("deepseek", "deepseek-flash")
  );
}

#[test]
fn every_order_names_a_provider_once_and_every_role_names_its_line() {
  let delegator = ROLES.roles.iter().find(|role| role.name == ROLES.delegate).unwrap();

  assert!(delegator.order.is_empty(), "the double's order is empty, so it follows its supervisor");

  for role in &ROLES.roles {
    let providers: BTreeSet<&str> = role.order.iter().map(|entry| entry.provider.as_str()).collect();

    assert!(
      !role.order.is_empty() || role.name == ROLES.delegate,
      "{} names an order; only the supervisor's double follows its supervisor",
      role.name
    );
    assert_eq!(providers.len(), role.order.len(), "{} names a provider twice", role.name);
    assert!(
      ["worker", "search", "browse", "translate"].contains(&role.line.as_str()),
      "{}",
      role.name
    );

    for entry in &role.order {
      assert!(
        harness_providers::provider(&entry.provider).is_some(),
        "{} names {}",
        role.name,
        entry.provider
      );
    }
  }
}

#[test]
fn only_the_co_worker_roles_are_callable_and_the_builder_is_the_orders_own_turn() {
  let callable: Vec<&str> =
    ROLES.roles.iter().filter(|role| role.callable).map(|role| role.name.as_str()).collect();

  assert_eq!(callable, ["verifier", "digest", "scout", "screener"]);
}

#[test]
fn the_defaults_nothing_names_are_seeded_roles_and_each_bundle_a_role_gives_is_declared() {
  for role in [&ROLES.job, &ROLES.delegate, &ROLES.renewal, &ROLES.translate] {
    assert!(ROLES.holds(role), "{role} is seeded");
  }

  assert!(ROLES.default_role("delegator"));
  assert!(!ROLES.default_role("helper"), "the helper is a role one names, and may go");
  assert_eq!(ROLES.supervisor, ["room.chat", "room.send", "web.read"]);
  assert!(ROLES.supervisor.contains(&ROLES.send));
  assert_eq!(ROLES.quota_share, 0.01);

  for role in &ROLES.roles {
    for bundle in &role.bundles {
      assert!(ROLES.declares_bundle(bundle), "{} names {bundle}, which is no bundle", role.name);
    }
  }
}

#[test]
fn a_roles_name_is_kebab_case() {
  let shape = Regex::new(&ROLES.name_pattern).unwrap();

  for role in &ROLES.roles {
    assert!(shape.is_match(&role.name), "{}", role.name);
  }

  assert!(!shape.is_match("Worker"));
  assert!(!shape.is_match("co_worker"));
}

#[test]
fn every_seeded_role_carries_a_label_a_screen_shows_where_its_pack_holds_no_word_for_its_code() {
  for role in &ROLES.roles {
    assert!(!role.label.trim().is_empty(), "{} carries no label", role.name);
    assert!(
      role.label.chars().count() <= ROLES.label_most,
      "{}'s label is past the policy's length",
      role.name
    );
  }
}
