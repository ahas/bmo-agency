//! The standing branch checkouts' list as the routes write it (docs/role-slots.md, 「The list」), and the
//! provider a native co-worker's program stands on.

use super::harness::named;

/// The provider a native co-worker's slot and model stand on where its program stands on no provider that
/// lends co-workers of its own: Claude Code, whose program a native call runs inside.
pub const NATIVE_PROVIDER: &str = "claude-code";

/// The provider the native co-workers of a program standing on `provider` take their slots and models on:
/// that provider where it lends co-workers, Claude Code where it lends none or is not one of this build.
pub fn native_provider(provider: &str) -> &'static str {
  named(provider)
    .ok()
    .filter(|one| one.declared().lends_coworkers)
    .map_or(NATIVE_PROVIDER, |one| one.name())
}

/// What a place's line was written with now noted under its key: the provider, model, effort and upstream,
/// the place keeping the latest moment it was written, which a later write never moves back.
pub async fn place_written(
  db: &sea_orm::DatabaseConnection,
  place: &str,
  provider: &str,
  model: &str,
  effort: Option<&str>,
  upstream: Option<&str>,
) -> Result<(), crate::Refusal> {
  store::run(
    db,
    r#"INSERT INTO "place_writes" ("place", "provider", "model", "effort", "upstream", "written_at")
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT ("place") DO UPDATE SET "provider" = excluded."provider", "model" = excluded."model",
         "effort" = excluded."effort", "upstream" = excluded."upstream",
         "written_at" = GREATEST("place_writes"."written_at", excluded."written_at")"#,
    vec![
      place.into(),
      provider.into(),
      model.into(),
      effort.unwrap_or_default().into(),
      upstream.into(),
      crate::now_ms().into(),
    ],
  )
  .await?;

  Ok(())
}
