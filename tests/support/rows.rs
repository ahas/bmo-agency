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
