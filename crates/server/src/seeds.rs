//! The seeds agency's server settles its store with (docs/store.md, 「Its seeds」): the values agency's
//! policy declares — the account, the ladder, the first seed's mapping of every tier, every work key with
//! the tier it starts on, every provider's fold ratio, the catalogue's entries and the base prompts — with the account's credential
//! the machine's own Claude Code login directory and each base prompt's text its seed file in the code
//! tree.

use std::path::Path;

use harness_providers::Machine;
use harness_providers::claude_code::ClaudeCode;
use policy::base_prompts::BASE_PROMPTS;
use policy::seeds::SEEDS;
use policy::tiers::TIERS;
use policy::work_keys::WORK_KEYS;
use store::seed::{
  AccountSeed, BasePromptSeed, CatalogueSeed, CoworkerTier, FoldRatioSeed, LaterSeed, Seeds, TierMapping,
  TierPlan, WorkKey,
};

use crate::control::claude_accounts::Credential;
use crate::control::fold_ratios;
use crate::places::machine_code_root;

/// The seeds the deployment's store is settled with on this machine.
pub fn deployment(machine: &Machine) -> Seeds {
  // The seeded account's login is the machine's own Claude Code login directory.
  let login = Credential::Directory {
    home: ClaudeCode::machine_config_dir(machine),
  };
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
      credential_kind: login.kind().word().to_owned(),
      credential_path: login.path().to_string_lossy().into_owned(),
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
          thinking: mapping.thinking,
          effort: mapping.effort.clone(),
          answer_tokens: mapping.answer_tokens,
        })
        .collect(),
      keys: profiles.chain(beyond).collect(),
      later: SEEDS
        .later
        .iter()
        .map(|later| LaterSeed {
          provider: later.provider.clone(),
          mappings: later
            .mapping
            .iter()
            .map(|mapping| TierMapping {
              provider: later.provider.clone(),
              tier: mapping.tier.clone(),
              model: mapping.model.clone(),
              thinking: mapping.thinking,
              effort: mapping.effort.clone(),
              answer_tokens: mapping.answer_tokens,
            })
            .collect(),
        })
        .collect(),
      coworker: TIERS
        .coworker
        .iter()
        .map(|tier| CoworkerTier {
          tier: tier.tier.clone(),
          capacity: tier.capacity,
        })
        .collect(),
    },
    // Each provider's own declared share, else the general one.
    fold_ratios: harness_providers::providers()
      .into_iter()
      .map(|provider| FoldRatioSeed {
        provider: provider.name().to_owned(),
        ratio: fold_ratios::fallback(provider.name()).0,
      })
      .collect(),
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
    base_prompts: base_prompt_seeds(&machine_code_root(machine)),
  }
}

/// The base prompts the policy declares, each with its seed file's text under the code tree. One whose
/// file cannot be read is said and left out, so it is given by the first settling that can read it.
pub fn base_prompt_seeds(code: &Path) -> Vec<BasePromptSeed> {
  BASE_PROMPTS
    .seed
    .iter()
    .filter_map(|seed| {
      let file = code.join("prompts").join("base").join(&seed.file);

      match std::fs::read_to_string(&file) {
        Ok(text) => Some(BasePromptSeed {
          name: seed.name.clone(),
          label: seed.label.clone(),
          text,
          agents: seed.agents.clone(),
        }),
        Err(why) => {
          eprintln!("agency: the base prompt seed {} is not given: {}: {why}", seed.name, file.display());

          None
        }
      }
    })
    .collect()
}
