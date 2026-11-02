//! The seeds agency's server settles its store with: the account on the machine's own Claude Code login,
//! the first seed's mapping of every tier on each provider the store begins with, and every work key at the
//! tier it starts on, no recollector's among them; given once, so a second settling writes nothing more.

use agency_server::seeds::{base_prompt_seeds, deployment, role_seeds};
use harness_providers::Machine;
use policy::base_prompts::BASE_PROMPTS;
use policy::roles::ROLES;
use store::seed::settle;

use crate::support::scratch::Scratch;

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
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "model_tier_mappings""#, vec![]).await, 24);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "model_tier_mappings" WHERE "tier" = 'Supervisor' AND "provider" = 'claude-code' AND "model" = 'claude-opus-5'"#,
        vec![]
      )
      .await,
    1
  );

  for (key, tier) in [
    ("bmo", "Supervisor"),
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
fn no_recollector_is_seeded() {
  let seeds = deployment(&machine(&[("HOME", "/home/someone")]));
  let keys: Vec<&str> = seeds.tiers.keys.iter().map(|key| key.key.as_str()).collect();

  for retired in ["digest", "people-digest", "people-standing", "session-summary"] {
    assert!(!keys.contains(&retired), "{retired} is seeded");
  }
}

#[tokio::test]
async fn a_ladder_that_gained_a_tier_maps_it_and_moves_the_keys_an_older_ladder_placed() {
  let scratch = Scratch::blank().await;
  let seeds = deployment(&machine(&[("HOME", "/home/someone")]));

  settle(scratch.db(), &seeds).await.unwrap();

  // A tier an earlier ladder held, with the mapping and the assignment that stood on it, before the
  // ladder that replaced it is settled.
  scratch.run(r#"DELETE FROM "model_tier_meta" WHERE "version" = 4"#, vec![]).await;
  scratch
    .run(r#"DELETE FROM "model_tier_mappings" WHERE "tier" = 'Supervisor'"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'BMO', 'an-old-model', 'medium')"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"UPDATE "model_tier_assignments" SET "tier" = 'BMO' WHERE "key" = 'bmo'"#, vec![])
    .await;

  settle(scratch.db(), &seeds).await.unwrap();

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "model_tier_mappings" WHERE "tier" = 'BMO'"#, vec![])
      .await,
    0
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "model_tier_assignments" WHERE "key" = 'bmo' AND "tier" = 'Supervisor'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "model_tier_mappings" WHERE "tier" = 'Supervisor' AND "provider" = 'claude-code'"#,
        vec![]
      )
      .await,
    1
  );
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
  assert!(seeds.iter().any(|seed| seed.name == "dsh" && seed.label == "DeepSeek Harness based"));
}

#[test]
fn a_seed_whose_file_cannot_be_read_is_left_out() {
  assert!(base_prompt_seeds(std::path::Path::new("/nowhere")).is_empty());
}

#[test]
fn every_declared_role_is_seeded_with_its_prompt_file_s_text() {
  let code = std::env::current_dir().unwrap();
  let seeds = role_seeds(&code);

  assert_eq!(seeds.len(), ROLES.roles.len());
  for (seed, declared) in seeds.iter().zip(&ROLES.roles) {
    let prompt = declared
      .prompt
      .as_ref()
      .map(|file| std::fs::read_to_string(code.join("prompts").join("roles").join(file)).unwrap());

    assert_eq!(
      (&seed.name, &seed.label, &seed.bundles),
      (&declared.name, &declared.label, &declared.bundles)
    );
    assert_eq!(seed.max_concurrent, declared.max_concurrent.map(|most| most as i64));
    assert_eq!(seed.prompt, prompt);
    assert_eq!(seed.sessions, declared.sessions.word());
  }
  assert!(seeds.iter().any(|seed| seed.name == "video" && seed.prompt.is_some()));

  // A role whose prompt file cannot be read waits for a settling that can; one naming none is given.
  let elsewhere: Vec<String> =
    role_seeds(std::path::Path::new("/nowhere")).into_iter().map(|seed| seed.name).collect();

  assert_eq!(elsewhere, ["chat", "worker", "searcher", "gate", "upkeep"]);
}
