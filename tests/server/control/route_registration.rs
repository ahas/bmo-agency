//! A work key's own registration (docs/own-llm.md, 「A key's own registration」): named on the key's route
//! row, it stands in place of the tier's registry cell for that key alone, on the registration's own answer
//! size and thinking unless the cell maps to it; it is refused for a registration with no address, and a
//! registration a key names is not removed.

use agency_server::control::{registry, roads, tiers};

use crate::support::scratch::Scratch;

// Two registrations with an address each, and one with none; the tier the translator is assigned mapping to
// the first on the registry's provider, with an answer ceiling and thinking of its own.
async fn seeded() -> Scratch {
  let scratch = Scratch::new().await;

  for (name, answer, thinking) in [("tier-cell", 4000, 0), ("own-pick", 6000, 1), ("unplaced", 2000, 0)] {
    scratch
      .run(
        r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
              "thinking", "walked", "added_at", "keeps_research")
           VALUES ($1, $1, 'served', 100000, $2, $3, 1, 1, 0)"#,
        vec![name.into(), (answer as i64).into(), (thinking as i64).into()],
      )
      .await;
  }

  for name in ["tier-cell", "own-pick"] {
    scratch
      .run(
        r#"INSERT INTO "registered_model_addresses" ("registration", "node", "position", "url", "key_file", "key_owned")
           VALUES ($1, 'n1', 1, 'http://127.0.0.1:9/v1/chat/completions', NULL, 0)"#,
        vec![name.into()],
      )
      .await;
  }

  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('translator', 'Simple'), ('voice-polish', 'Simple')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort", "answer_tokens", "thinking")
         VALUES ('registered', 'Simple', 'tier-cell', NULL, 2048, true)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "model" = excluded."model", "effort" = excluded."effort",
           "answer_tokens" = excluded."answer_tokens", "thinking" = excluded."thinking""#,
      vec![],
    )
    .await;

  scratch
}

#[tokio::test]
async fn a_key_s_own_registration_stands_for_that_key_alone_on_its_own_settings() {
  let scratch = seeded().await;
  let db = scratch.db();

  roads::set_registration(db, "translator", Some("own-pick"), 1).await.unwrap();

  let pinned = tiers::resolve(db, "translator", registry::PROVIDER).await.unwrap();

  assert_eq!(pinned.model, "own-pick");
  assert_eq!((pinned.thinking, pinned.answer_tokens, pinned.effort.as_deref()), (None, None, None));
  assert_eq!(pinned.tier, "Simple");

  // The registration's own answer size and thinking default stand.
  let settings = registry::run_settings(db, Some(&registry::Mapping::from(&pinned))).await.unwrap();

  assert_eq!(
    (settings.model.as_str(), settings.answer_tokens, settings.thinking),
    ("own-pick", 6000, true)
  );

  // Another key of the same tier keeps the cell, and the key on another provider keeps that provider's.
  let other = tiers::resolve(db, "voice-polish", registry::PROVIDER).await.unwrap();

  assert_eq!(
    (other.model.as_str(), other.answer_tokens, other.thinking),
    ("tier-cell", Some(2048), Some(true))
  );
  assert_ne!(
    tiers::resolve(db, "translator", "claude-code").await.map(|one| one.model),
    Ok("own-pick".into())
  );

  // A caller naming the tier itself — a gate names its own — is answered the key's registration as well.
  let named = tiers::resolve_execution(db, "translator", registry::PROVIDER, Some("Simple"))
    .await
    .unwrap();

  assert_eq!((named.model.as_str(), named.answer_tokens), ("own-pick", None));

  // Naming the cell's own registration keeps the cell's settings; taking the pin away reads the cell again
  // and leaves no row behind.
  roads::set_registration(db, "translator", Some("tier-cell"), 2).await.unwrap();
  assert_eq!(
    tiers::resolve(db, "translator", registry::PROVIDER).await.unwrap().answer_tokens,
    Some(2048)
  );

  roads::set_registration(db, "translator", None, 3).await.unwrap();
  assert!(roads::route_row(db, "translator").await.unwrap().is_none());
  assert_eq!(tiers::resolve(db, "translator", registry::PROVIDER).await.unwrap().model, "tier-cell");
}

#[tokio::test]
async fn a_registration_with_no_address_is_refused_and_one_a_key_names_is_not_removed() {
  let scratch = seeded().await;
  let db = scratch.db();

  assert!(roads::registration_refusal(db, "own-pick").await.unwrap().is_none());
  assert!(roads::registration_refusal(db, "unplaced").await.unwrap().is_some());
  assert!(roads::registration_refusal(db, "nobody").await.unwrap().is_some());

  // A registration that lost its address after the key named it is refused, never given back to the tier.
  roads::set_registration(db, "voice-polish", Some("unplaced"), 1).await.unwrap();
  assert!(
    tiers::resolve(db, "voice-polish", registry::PROVIDER)
      .await
      .unwrap_err()
      .contains("unplaced")
  );

  roads::set_registration(db, "translator", Some("own-pick"), 1).await.unwrap();

  assert_eq!(
    registry::referrers(db, "own-pick").await.unwrap(),
    ["work key translator's own registration"]
  );
  assert_eq!(roads::key_registration(db, "translator").await.unwrap().as_deref(), Some("own-pick"));
}
