//! The seeds agency's server settles its store with (docs/store.md, 「Its seeds」): the values agency's
//! policy declares — the account, the ladder, the first seed's mapping of every tier, every work key with
//! the tier it starts on, and the catalogue's entries — with the account's credential the machine's own
//! Claude Code login directory.

use harness_providers::Machine;
use harness_providers::claude_code::ClaudeCode;
use policy::seeds::SEEDS;
use policy::tiers::TIERS;
use policy::work_keys::WORK_KEYS;
use store::seed::{AccountSeed, CatalogueSeed, Seeds, TierMapping, TierPlan, WorkKey};

/// The kind of credential the seeded account's login is: a Claude Code login directory.
const DIRECTORY: &str = "directory";

/// The seeds the deployment's store is settled with on this machine.
pub fn deployment(machine: &Machine) -> Seeds {
  let profiles = WORK_KEYS.profile.iter().map(|profile| WorkKey {
    key: profile.key.clone(),
    tier: profile.tier.clone(),
  });
  let beyond = SEEDS.key.iter().map(|key| WorkKey {
    key: key.key.clone(),
    tier: key.tier.clone(),
  });

  Seeds {
    account: Some(AccountSeed {
      id: SEEDS.account.id.clone(),
      name: SEEDS.account.name.clone(),
      credential_kind: DIRECTORY.to_owned(),
      credential_path: ClaudeCode::machine_config_dir(machine).to_string_lossy().into_owned(),
    }),
    tiers: TierPlan {
      ladder: TIERS.ladder.clone(),
      mappings: SEEDS
        .mapping
        .iter()
        .map(|mapping| TierMapping {
          provider: mapping.provider.clone(),
          tier: mapping.tier.clone(),
          model: mapping.model.clone(),
          effort: mapping.effort.clone(),
          answer_tokens: mapping.answer_tokens,
        })
        .collect(),
      keys: profiles.chain(beyond).collect(),
      later: vec![],
    },
    catalogue: SEEDS
      .catalogue
      .iter()
      .map(|entry| CatalogueSeed {
        name: entry.name.clone(),
        label: entry.label.clone(),
        file: entry.file.clone(),
        bytes: entry.bytes,
        sha256: entry.sha256.clone(),
        source: entry.source.clone(),
        engine: entry.engine.clone(),
        registration: entry.registration.clone(),
        thinking: entry.thinking,
        temperature: entry.temperature,
        top_p: entry.top_p,
        presence_penalty: entry.presence_penalty,
        max_tokens: entry.max_tokens,
        note: entry.note.clone(),
      })
      .collect(),
  }
}
