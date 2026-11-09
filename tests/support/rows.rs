//! The rows a test lays down on the two axes' store: a supervisor, which every conversational agent is, and
//! a repository, the place a job's checkout is read off. Each is written with the columns its table requires
//! and whatever else the test names beside them.

use sea_orm::{DatabaseConnection, Value};

/// A row of a table, written with the columns named, in their order.
async fn insert(db: &DatabaseConnection, table: &str, columns: Vec<(&str, Value)>) {
  let names: Vec<String> = columns.iter().map(|(name, _)| format!("\"{name}\"")).collect();
  let places: Vec<String> = (1..=columns.len()).map(|at| format!("${at}")).collect();

  store::run(
    db,
    &format!(r#"INSERT INTO "{table}" ({}) VALUES ({})"#, names.join(", "), places.join(", ")),
    columns.into_iter().map(|(_, value)| value).collect(),
  )
  .await
  .unwrap_or_else(|why| panic!("the {table} row could not be written: {}", why.detail));
}

/// A supervisor: one session across every room, the window's turns and an order of Claude Code alone unless
/// the test names otherwise. A supervisor's turn waits until a provider of its order can answer, so none is
/// laid down with an empty order.
pub async fn supervisor(db: &DatabaseConnection, name: &str, with: &[(&str, Value)]) {
  let mut columns: Vec<(&str, Value)> = vec![("name", name.into())];

  for (column, default) in [
    ("session_scope", "agent"),
    ("turn_shape", "window"),
    ("provider_order", r#"[{"provider":"claude-code","model":"claude-opus-5"}]"#),
  ] {
    if !with.iter().any(|(named, _)| *named == column) {
      columns.push((column, default.into()));
    }
  }

  columns.extend(with.iter().cloned());
  columns.extend([("created_at", 0i64.into()), ("updated_at", 0i64.into())]);
  insert(db, "supervisors", columns).await;
}

/// A repository whose base checkout stands at a path.
pub async fn repository(db: &DatabaseConnection, name: &str, path: &str, with: &[(&str, Value)]) {
  let mut columns: Vec<(&str, Value)> = vec![("name", name.into()), ("path", path.into())];

  columns.extend(with.iter().cloned());
  columns.extend([("created_at", 0i64.into()), ("updated_at", 0i64.into())]);
  insert(db, "repositories", columns).await;
}

/// A role's agents laid down afresh, one a road named, in the order named: its seats, each on one provider and
/// model (docs/role-agents.md). A role given none carries no run.
pub async fn seats(db: &DatabaseConnection, role: &str, roads: &[(&str, &str)]) {
  store::run(db, r#"DELETE FROM "role_agents" WHERE "role" = $1"#, vec![role.into()])
    .await
    .unwrap();

  for (place, (provider, model)) in roads.iter().enumerate() {
    store::run(
      db,
      r#"INSERT INTO "role_agents" ("name", "role", "provider", "model", "position") VALUES ($1, $2, $3, $4, $5)"#,
      vec![
        format!("{role}-{:02}", place + 1).into(),
        role.into(),
        (*provider).into(),
        (*model).into(),
        (place as i64 + 1).into(),
      ],
    )
    .await
    .unwrap();
  }
}

/// A role's agents laid down afresh, `count` of them, each naming no road, so its runs follow the supervisor
/// that asks them.
pub async fn following_seats(db: &DatabaseConnection, role: &str, count: usize) {
  store::run(db, r#"DELETE FROM "role_agents" WHERE "role" = $1"#, vec![role.into()])
    .await
    .unwrap();

  for place in 1..=count {
    store::run(
      db,
      r#"INSERT INTO "role_agents" ("name", "role", "position") VALUES ($1, $2, $3)"#,
      vec![format!("{role}-{place:02}").into(), role.into(), (place as i64).into()],
    )
    .await
    .unwrap();
  }
}

/// Every role's agents laid down afresh from what its row names, as the store gives a role its agents: as many
/// as its row's cap, each on its row's forced provider or its order's first entry. For a test that writes a
/// role's row itself.
pub async fn reseated(db: &DatabaseConnection) {
  store::run(db, r#"DELETE FROM "role_agents""#, vec![]).await.unwrap();
  store::run(db, store::migration::m20261203_000001_role_agents::AGENTS_OF_ROLES, vec![])
    .await
    .unwrap();
}
