//! An account's node (docs/person-nodes.md, 「Which node is a person's」): the record agency keeps of the node an
//! account's runs stand on, set by the service that binds the account to its machine; the supervisors and role
//! runs placed there by it; and the wait of a run of it while the node's link is not heard.

use std::path::Path;
use std::time::Duration;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use store::{all, one, run};

use crate::Refusal;
use crate::peers::PeerFault;
use crate::peers::node::{NodeKind, NodeService};

/// One account's node as its record holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonNode {
  pub owner_account: String,
  pub node: String,
  /// When it was set, in epoch milliseconds.
  pub set_at: i64,
}

fn of_row(row: &sea_orm::QueryResult) -> Result<PersonNode, sea_orm::DbErr> {
  Ok(PersonNode {
    owner_account: row.try_get("", "owner_account")?,
    node: row.try_get("", "node")?,
    set_at: row.try_get("", "set_at")?,
  })
}

/// Every account's node, by account.
pub async fn every(db: &DatabaseConnection) -> Result<Vec<PersonNode>, Refusal> {
  let rows = all(db, r#"SELECT * FROM "person_nodes" ORDER BY "owner_account""#, vec![]).await?;

  Ok(rows.iter().map(of_row).collect::<Result<_, _>>()?)
}

/// The node an account's runs stand on, or none where the account has none.
pub async fn node_of<C: ConnectionTrait>(db: &C, account: &str) -> Result<Option<String>, Refusal> {
  let row = one(
    db,
    r#"SELECT "node" FROM "person_nodes" WHERE "owner_account" = $1"#,
    vec![account.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<String>("", "node")).transpose()?)
}

/// An account's node set, replacing the one it had.
pub async fn set(db: &DatabaseConnection, account: &str, node: &str) -> Result<PersonNode, Refusal> {
  let at = crate::now_ms();

  run(
    db,
    r#"INSERT INTO "person_nodes" ("owner_account", "node", "set_at") VALUES ($1, $2, $3)
       ON CONFLICT ("owner_account") DO UPDATE SET "node" = $2, "set_at" = $3"#,
    vec![account.into(), node.into(), at.into()],
  )
  .await?;

  Ok(PersonNode {
    owner_account: account.to_owned(),
    node: node.to_owned(),
    set_at: at,
  })
}

/// An account's node taken away; whether it had one.
pub async fn clear(db: &DatabaseConnection, account: &str) -> Result<bool, Refusal> {
  Ok(run(db, r#"DELETE FROM "person_nodes" WHERE "owner_account" = $1"#, vec![account.into()]).await? > 0)
}

/// Why a node cannot be an account's, or none where the node service keeps it as a hub node reserved for no
/// service: a person's machine.
pub async fn refusal(nodes: &dyn NodeService, node: &str) -> Result<Option<String>, PeerFault> {
  let kept = nodes.node_reads().await?.into_iter().find(|read| read.name == node);

  Ok(match kept {
    None => Some(format!("{node} is no node the node service keeps")),
    Some(read) if read.kind != NodeKind::Jake => {
      Some(format!("{node} is no hub node, so it is no person's machine"))
    }
    Some(read) => read
      .role
      .map(|role| format!("{node} is reserved for {role}, so it is no person's machine")),
  })
}

/// The node each supervisor stands on, by name: its owner account's, none for a supervisor of no account or of
/// an account with no node.
pub async fn supervisors_placed(db: &DatabaseConnection) -> Result<Vec<(String, Option<String>)>, Refusal> {
  let rows = all(
    db,
    r#"SELECT s."name", p."node" FROM "supervisors" s
       LEFT JOIN "person_nodes" p ON p."owner_account" = s."owner_account"::text
       ORDER BY s."name""#,
    vec![],
  )
  .await?;

  Ok(
    rows
      .iter()
      .map(|row| Ok((row.try_get("", "name")?, row.try_get("", "node")?)))
      .collect::<Result<_, sea_orm::DbErr>>()?,
  )
}

/// The node a role run stands on: its seat's account's, read off the seat its slot holds at the folder it works
/// in; none for a run of no account or of an account with no node.
pub async fn run_node<C: ConnectionTrait>(
  db: &C,
  role: Option<&str>,
  cwd: &str,
) -> Result<Option<String>, Refusal> {
  let seat = crate::role_agents::seated_at(db, role, Some(cwd)).await?;
  let Some(account) = seat.and_then(|seat| seat.owner_account) else {
    return Ok(None);
  };

  node_of(db, &account).await
}

/// Waits, asking every `every`, until `heard` says the node's link has been heard from.
pub async fn until_heard(heard: impl Fn() -> bool, every: Duration) {
  while !heard() {
    tokio::time::sleep(every).await;
  }
}

/// Waits until a node's link has been heard from, as the link writes its state under the data root, saying once
/// that it waits; a node heard from goes on at once.
pub async fn wait_heard(data_root: &Path, node: &str, run: &str) {
  let heard = || crate::watches::links::link_heard(data_root, node);

  if heard() {
    return;
  }

  println!("{run}: waits for {node}, whose link is not heard");
  until_heard(heard, Duration::from_secs(policy::person_nodes::PERSON_NODES.wait_seconds)).await;
  println!("{run}: {node} is heard, so it goes on");
}
