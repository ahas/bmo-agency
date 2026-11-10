//! The roles policy (docs/agent-roles.md, 「The roles the policy seeds」): the fourteen seeded roles with their
//! lines, caps, sessions, descriptions, callable marks and orders, the defaults nothing names, the
//! supervisor bundles and the shape of a role's name.

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
      "coder",
      "doppelganger",
      "helper",
      "producer",
      "builder",
      "verifier",
      "scout",
      "screener",
      "voyager",
      "seeker",
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
    ("coder", 3),
    ("doppelganger", 2),
    ("helper", 2),
    ("producer", 2),
    ("builder", 8),
    ("verifier", 6),
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
  let follows: Vec<&str> = ROLES
    .roles
    .iter()
    .filter(|role| role.order.is_empty())
    .map(|role| role.name.as_str())
    .collect();

  assert_eq!(
    follows,
    ["coder", "doppelganger"],
    "the roles whose road follows the supervisor that asks them"
  );

  for role in &ROLES.roles {
    let providers: BTreeSet<&str> = role.order.iter().map(|entry| entry.provider.as_str()).collect();

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

  assert_eq!(callable, ["verifier", "scout", "screener"]);
}

#[test]
fn the_defaults_nothing_names_are_seeded_roles_and_each_bundle_a_role_gives_is_declared() {
  for role in [&ROLES.job, &ROLES.delegate, &ROLES.renewal, &ROLES.translate] {
    assert!(ROLES.holds(role), "{role} is seeded");
  }

  assert!(ROLES.default_role("doppelganger"));
  assert!(!ROLES.default_role("producer"), "the producer is a role one names, and may go");
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
fn every_seeded_role_carries_the_label_a_log_keeps_of_it() {
  for role in &ROLES.roles {
    assert!(!role.label.trim().is_empty(), "{} carries no label", role.name);
    assert!(
      role.label.chars().count() <= ROLES.label_most,
      "{}'s label is past the policy's length",
      role.name
    );
  }
}

#[test]
fn every_role_declares_its_description_once_and_a_co_worker_its_definition() {
  for role in &ROLES.roles {
    match (&role.definition, &role.description) {
      (Some(_), Some(_)) => panic!("{} states a description beside its definition's", role.name),
      (Some(_), None) => {
        assert!(role.prompt.is_none(), "{} names a prompt beside its definition", role.name)
      }
      (None, Some(description)) => {
        assert!(!description.trim().is_empty(), "{}'s description is empty", role.name)
      }
      (None, None) => panic!("{} states no description and names no definition", role.name),
    }
    assert!(
      !role.callable || role.definition.is_some(),
      "{} is callable, so a co-worker definition gives its prompt and description",
      role.name
    );
  }
}

#[test]
fn the_helper_goes_out_on_the_strongest_road_first_and_the_double_on_none() {
  let helper = ROLES.role("helper").unwrap();

  assert_eq!(
    (helper.order[0].provider.as_str(), helper.order[0].model.as_str()),
    ("claude-code", "claude-fable-5-2"),
    "a problem a supervisor could not settle goes to a stronger model than its own"
  );
  assert!(
    ROLES.role("doppelganger").unwrap().order.is_empty(),
    "the double follows the asking supervisor"
  );
  assert!(!ROLES.default_role("helper"), "the helper is a role one names");
}
