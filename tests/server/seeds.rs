//! The seeds agency's server settles its store with: the account on the machine's own Claude Code login,
//! the first seed's mapping of every tier on each provider the store begins with, and every work key at the
//! tier it starts on; given once, so a second settling writes nothing more.

use agency_server::seeds::{base_prompt_seeds, deployment};
use harness_providers::Machine;
use policy::base_prompts::BASE_PROMPTS;
use store::seed::settle;

use crate::scratch::Scratch;

fn machine(vars: &[(&str, &str)]) -> Machine {
  Machine::with_env(vars.iter().map(|(name, value)| ((*name).to_owned(), (*value).to_owned())), "/d")
}

#[test]
fn the_account_is_the_machines_own_login_directory() {
  let home = deployment(&machine(&[("HOME", "/home/someone")])).account.unwrap();
  let named = deployment(&machine(&[("HOME", "/home/someone"), ("CLAUDE_CONFIG_DIR", "/logins/main")]))
    .account
    .unwrap();

  assert_eq!(
    (
      home.id.as_str(),
      home.name.as_str(),
      home.credential_kind.as_str(),
      home.credential_path.as_str()
    ),
    ("default", "Default account", "directory", "/home/someone/.claude")
  );
  assert_eq!(named.credential_path, "/logins/main");
}

#[tokio::test]
async fn a_fresh_store_is_given_the_account_the_tiers_and_the_keys_once() {
  let scratch = Scratch::blank().await;
  let seeds = deployment(&machine(&[("HOME", "/home/someone")]));

  settle(scratch.db(), &seeds).await.unwrap();
  settle(scratch.db(), &seeds).await.unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "claude_accounts" WHERE "id" = 'default' AND "credential_path" = '/home/someone/.claude'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "claude_accounts""#, vec![]).await, 1);
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "model_tier_mappings""#, vec![]).await, 18);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "model_tier_mappings" WHERE "tier" = 'BMO' AND "provider" = 'claude-code' AND "model" = 'claude-opus-5'"#,
        vec![]
      )
      .await,
    1
  );

  for (key, tier) in [
    ("bmo", "BMO"),
    ("job-worker", "Frontier"),
    ("output-format", "Simple"),
    ("role:scout", "Light"),
  ] {
    assert_eq!(
      scratch
        .count(
          r#"SELECT count(*) FROM "model_tier_assignments" WHERE "key" = $1 AND "tier" = $2"#,
          vec![key.into(), tier.into()]
        )
        .await,
      1,
      "{key} starts on {tier}"
    );
  }
}

#[test]
fn every_declared_base_prompt_is_seeded_with_its_files_text() {
  let code = std::env::current_dir().unwrap();
  let seeds = base_prompt_seeds(&code);

  assert_eq!(seeds.len(), BASE_PROMPTS.seed.len());
  for (seed, declared) in seeds.iter().zip(&BASE_PROMPTS.seed) {
    let file = std::fs::read_to_string(code.join("prompts").join("base").join(&declared.file)).unwrap();

    assert_eq!(
      (seed.name.as_str(), seed.label.as_str()),
      (declared.name.as_str(), declared.label.as_str())
    );
    assert_eq!(seed.text, file);
  }
  assert!(seeds.iter().any(|seed| seed.name == "claude-code" && seed.label == "Claude Code based"));
}

#[test]
fn a_seed_whose_file_cannot_be_read_is_left_out() {
  assert!(base_prompt_seeds(std::path::Path::new("/nowhere")).is_empty());
}
