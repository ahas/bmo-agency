//! The units each account reaches (docs/api.md, 「Who a caller acts for」): a row an account, a unit and whether the
//! account heads it, written whole by the service in front of the agency, which keeps the organisation. The agency
//! keeps no organisation of its own and asks nobody: a unit's reach is this table and nothing else, and an agency
//! whose table is empty lets no account reach any unit.

use sea_orm::{ConnectionTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use store::{all, one, run};

use crate::Refusal;

/// One account's reach of one unit: whether its people read the unit's resources, and whether they head the unit
/// and so may change them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reached {
  pub account: i64,
  pub unit: i64,
  pub write: bool,
}

/// Every row the table holds, by account and then unit.
pub async fn every(db: &impl ConnectionTrait) -> Result<Vec<Reached>, Refusal> {
  let rows = all(db, r#"SELECT "account", "unit", "write" FROM "unit_reach" ORDER BY "account", "unit""#, vec![]).await?;

  rows
    .into_iter()
    .map(|row| {
      Ok(Reached {
        account: row.try_get("", "account")?,
        unit: row.try_get("", "unit")?,
        write: row.try_get("", "write")?,
      })
    })
    .collect()
}

/// The table replaced whole by the rows given, in one transaction.
pub async fn replace<C: ConnectionTrait + TransactionTrait>(db: &C, rows: &[Reached]) -> Result<(), Refusal> {
  let tx = db.begin().await?;

  run(&tx, r#"DELETE FROM "unit_reach""#, vec![]).await?;

  for row in rows {
    run(
      &tx,
      r#"INSERT INTO "unit_reach" ("account", "unit", "write") VALUES ($1, $2, $3)
         ON CONFLICT ("account", "unit") DO UPDATE SET "write" = "unit_reach"."write" OR excluded."write""#,
      vec![row.account.into(), row.unit.into(), row.write.into()],
    )
    .await?;
  }

  tx.commit().await?;

  Ok(())
}

/// Whether an account heads a unit: its row says it may change the unit's resources.
pub async fn heads(db: &impl ConnectionTrait, account: i64, unit: i64) -> Result<bool, Refusal> {
  let row = one(
    db,
    r#"SELECT "write" FROM "unit_reach" WHERE "account" = $1 AND "unit" = $2"#,
    vec![account.into(), unit.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<bool>("", "write")).transpose()?.unwrap_or(false))
}

/// Whether an account reaches a unit at all, a head or not.
pub async fn reaches(db: &impl ConnectionTrait, account: i64, unit: i64) -> Result<bool, Refusal> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "held" FROM "unit_reach" WHERE "account" = $1 AND "unit" = $2"#,
      vec![account.into(), unit.into()],
    )
    .await?
    .is_some(),
  )
}

/// The units an account reaches, those it heads among them marked, by unit.
pub async fn of(db: &impl ConnectionTrait, account: i64) -> Result<Vec<Reached>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "account", "unit", "write" FROM "unit_reach" WHERE "account" = $1 ORDER BY "unit""#,
    vec![account.into()],
  )
  .await?;

  rows
    .into_iter()
    .map(|row| {
      Ok(Reached {
        account: row.try_get("", "account")?,
        unit: row.try_get("", "unit")?,
        write: row.try_get("", "write")?,
      })
    })
    .collect()
}
