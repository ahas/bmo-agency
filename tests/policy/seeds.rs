use std::collections::BTreeSet;

use policy::seeds::SEEDS;
use policy::tiers::TIERS;
use policy::work_keys::WORK_KEYS;

// The providers the store begins with, each mapped in every tier by the first seed.
const FIRST: [&str; 3] = ["claude-code", "codex", "openrouter"];

#[test]
fn the_first_seed_maps_every_tier_once_on_each_provider_the_store_begins_with() {
  for tier in &TIERS.ladder {
    for provider in FIRST {
      let mapped = SEEDS
        .mapping
        .iter()
        .filter(|mapping| &mapping.tier == tier && mapping.provider == provider)
        .count();

      assert_eq!(mapped, 1, "{tier} on {provider}");
    }
  }

  assert_eq!(SEEDS.mapping.len(), TIERS.ladder.len() * FIRST.len());
  assert!(SEEDS.mapping.iter().all(|mapping| !mapping.model.is_empty()));
}

#[test]
fn the_keys_beyond_the_profiles_start_on_tiers_of_the_ladder_and_name_no_profile() {
  let profiles: BTreeSet<&str> = WORK_KEYS.profile.iter().map(|profile| profile.key.as_str()).collect();
  let mut seen = BTreeSet::new();

  for key in &SEEDS.key {
    assert!(TIERS.holds(&key.tier), "{} starts on {}", key.key, key.tier);
    assert!(!profiles.contains(key.key.as_str()), "{} is a profile's", key.key);
    assert!(seen.insert(key.key.as_str()), "{} twice", key.key);
  }
}

#[test]
fn the_account_is_the_machines_login_and_the_repository_seeds_no_model_file() {
  assert_eq!((SEEDS.account.id.as_str(), SEEDS.account.name.as_str()), ("default", "Default account"));
  assert!(SEEDS.catalogue.is_empty());
}
