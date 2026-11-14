//! An account's node (docs/person-nodes.md, 「Which node is a person's」): the record agency keeps of the node an
//! account's runs stand on, set by the service that binds the account to its machine, else the account's isolation
//! node on this machine (docs/account-isolation.md); the supervisors and role runs placed there by it; the wait of a
//! run of it while the node's link is not heard; and who reaches a node (「Who reaches a node」).

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

/// The node an account's runs stand on: the one its record names, else its isolation node; none for the
/// administrator's account, whose runs are the owner's own (docs/account-isolation.md, 「The scope」).
pub async fn node_of<C: ConnectionTrait>(db: &C, account: &str) -> Result<Option<String>, Refusal> {
  let row = one(
    db,
    r#"SELECT "node" FROM "person_nodes" WHERE "owner_account" = $1"#,
    vec![account.into()],
  )
  .await?;
  let recorded = row.map(|row| row.try_get::<String>("", "node")).transpose()?;

  placed_for(db, Some(account), recorded).await
}

/// The node the runs of an account stand on, given the node its record names: that node, else the account's
/// isolation node; none for no account and for the administrator's.
pub async fn placed_for<C: ConnectionTrait>(
  db: &C,
  account: Option<&str>,
  recorded: Option<String>,
) -> Result<Option<String>, Refusal> {
  let Some(account) = account else { return Ok(None) };

  if recorded.is_some() {
    return Ok(recorded);
  }

  if crate::account_isolation::owners_own(db, account).await? {
    return Ok(None);
  }

  Ok(Some(crate::account_isolation::node_name(account)))
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
/// the administrator's account.
pub async fn supervisors_placed(db: &DatabaseConnection) -> Result<Vec<(String, Option<String>)>, Refusal> {
  let rows = all(
    db,
    r#"SELECT s."name", s."owner_account"::text AS "account", p."node" FROM "supervisors" s
       LEFT JOIN "person_nodes" p ON p."owner_account" = s."owner_account"::text
       ORDER BY s."name""#,
    vec![],
  )
  .await?;
  let mut placed = Vec::new();

  for row in &rows {
    let account: Option<String> = row.try_get("", "account")?;
    let node = placed_for(db, account.as_deref(), row.try_get("", "node")?).await?;

    placed.push((row.try_get("", "name")?, node));
  }

  Ok(placed)
}

/// Every account a supervisor or a role agent belongs to, each once: the accounts whose runs may be placed.
pub async fn accounts_placed(db: &DatabaseConnection) -> Result<Vec<String>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "owner_account"::text AS "account" FROM "supervisors" WHERE "owner_account" IS NOT NULL
       UNION SELECT "owner_account" AS "account" FROM "role_agents" WHERE "owner_account" IS NOT NULL
       ORDER BY "account""#,
    vec![],
  )
  .await?;

  Ok(rows.iter().map(|row| row.try_get("", "account")).collect::<Result<_, _>>()?)
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

/// The accounts whose record names `node`, and the account an isolation node is named for, by account; none for a
/// node that is no person's.
pub async fn owners_of<C: ConnectionTrait>(db: &C, node: &str) -> Result<Vec<String>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "owner_account" FROM "person_nodes" WHERE "node" = $1 ORDER BY "owner_account""#,
    vec![node.into()],
  )
  .await?;
  let mut owners: Vec<String> =
    rows.iter().map(|row| row.try_get("", "owner_account")).collect::<Result<_, _>>()?;

  if let Some(isolation) = crate::account_isolation::Isolation::of_node(node)
    && !owners.contains(&isolation.account)
  {
    owners.push(isolation.account);
    owners.sort();
  }

  Ok(owners)
}

/// A caller as its tool session's record names it: the record's holder, and the role and the folder its labels
/// hold, where they hold them.
#[derive(Clone, Copy, Debug)]
pub struct Caller<'a> {
  pub agent: &'a str,
  pub role: Option<&'a str>,
  pub checkout: Option<&'a str>,
}

/// What a caller acts as: the account it acts for, none for the deployment's own, and whether it holds the
/// administrator's grant, which an administrator's supervisor alone does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActingFor {
  pub account: Option<String>,
  pub administrator: bool,
}

impl ActingFor {
  /// The administrator's grants a tool session's record for this caller holds: the policy's for an
  /// administrator's supervisor, none for any other.
  pub fn grants(&self) -> Vec<String> {
    if self.administrator {
      policy::person_nodes::PERSON_NODES.administrator_grants.clone()
    } else {
      Vec::new()
    }
  }
}

/// What a caller acts as. Its account is the supervisor's it is or a double stands for, else the role agent's
/// of its name, else the seat's its run holds at its role and folder, each by its `ownerAccount`. It holds the
/// administrator's grant where it is a supervisor of no account or of the administrator's account. A thrown-away session's place is read as its
/// session's agent.
pub async fn acting_for<C: ConnectionTrait>(
  db: &C,
  data_root: &Path,
  caller: Caller<'_>,
) -> Result<ActingFor, Refusal> {
  let agent =
    crate::apart_places::session_agent(data_root, caller.agent).unwrap_or_else(|| caller.agent.to_owned());

  if let Some(supervisor) = crate::supervisors::named(db, &agent).await? {
    let account = supervisor.owner_account.map(|account| account.to_string());
    let administrator = match &account {
      None => true,
      Some(account) => crate::account_isolation::owners_own(db, account).await?,
    };

    return Ok(ActingFor {
      administrator,
      account,
    });
  }

  let account = match crate::role_agents::named(db, &agent).await? {
    Some(seat) => seat.owner_account,
    None if caller.role.is_none() && caller.checkout.is_none() => None,
    None => crate::role_agents::seated_at(db, caller.role, caller.checkout)
      .await?
      .and_then(|seat| seat.owner_account),
  };

  Ok(ActingFor {
    account,
    administrator: false,
  })
}

/// Whether a caller reaches a node (docs/api.md, 「person-nodes」).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
  pub node: String,
  pub owners: Vec<String>,
  pub agent: String,
  /// The account the caller acts for; none for the deployment's own.
  pub account: Option<String>,
  /// Whether it holds the administrator's grant, which reaches every node.
  pub administrator: bool,
  pub reaches: bool,
}

/// Whether `caller` reaches `node`: where it holds the administrator's grant, or its account owns the node.
pub async fn access<C: ConnectionTrait>(
  db: &C,
  data_root: &Path,
  node: &str,
  caller: Caller<'_>,
) -> Result<Access, Refusal> {
  let owners = owners_of(db, node).await?;
  let ActingFor {
    account,
    administrator,
  } = acting_for(db, data_root, caller).await?;
  let reaches = administrator || account.as_ref().is_some_and(|account| owners.contains(account));

  Ok(Access {
    node: node.to_owned(),
    owners,
    agent: caller.agent.to_owned(),
    account,
    administrator,
    reaches,
  })
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
