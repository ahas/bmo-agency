//! The owner's warming switch (docs/keepalive-schedule.md, 「Whether pings go」): it stops every keepalive
//! ping of the deployment's while it stands, whatever the kind and the provider. It stands while the store's
//! `keepalive_stopped` setting holds `1`, and a store that holds no such row warms.

use sea_orm::ConnectionTrait;
use store::{Fault, one, run};

// The setting the switch stands in (docs/store.md, 「The settings table」), and what it holds while it stands.
const STOPPED: &str = "keepalive_stopped";
const STANDS: &str = "1";

/// Whether the owner's warming switch stands now.
pub async fn stopped<C: ConnectionTrait>(db: &C) -> Result<bool, Fault> {
  let row = one(db, r#"SELECT "value" FROM "settings" WHERE "key" = $1"#, vec![STOPPED.into()]).await?;
  let value = row
    .map(|row| row.try_get::<String>("", "value"))
    .transpose()
    .map_err(|why| Fault::new(format!("the warming switch: {why}")))?;

  Ok(value.as_deref() == Some(STANDS))
}

/// The switch thrown: standing while `stopped`, gone otherwise.
pub async fn set_stopped<C: ConnectionTrait>(db: &C, stopped: bool) -> Result<(), Fault> {
  if stopped {
    run(
      db,
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)
         ON CONFLICT ("key") DO UPDATE SET "value" = excluded."value""#,
      vec![STOPPED.into(), STANDS.into()],
    )
    .await?;
  } else {
    run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![STOPPED.into()]).await?;
  }

  Ok(())
}

/// Whether keepalive pings go now: unless the owner's warming switch stands.
pub async fn pings_go<C: ConnectionTrait>(db: &C) -> Result<bool, Fault> {
  Ok(!stopped(db).await?)
}
