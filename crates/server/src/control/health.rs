//! Agency's health across the boundary (docs/api.md, 「Health」): readiness — the server's process, the code
//! tree it reads and whether its places have stood once — with the nodes as the node service keeps them,
//! the pool's counts by state and whether each account is usable now. What cannot be read says so.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::claude_accounts::{Judging, account_open, accounts};
use super::{Call, Refused, Route, answer};
use crate::now_ms;

/// Agency's health.
pub struct Health;

impl Route for Health {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/health";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let ground = call.ground();
    let ready = ground.readiness.answer(&ground.code_root());
    let nodes = match ground.peers.node.node_rows().await {
      Ok(rows) => json!(
        rows
          .iter()
          .map(
            |row| json!({ "node": row.node, "label": row.label, "state": row.state, "detail": row.detail })
          )
          .collect::<Vec<_>>()
      ),
      Err(fault) => json!({ "error": fault.to_string() }),
    };
    let mut pool = serde_json::Map::new();

    for state in ["active", "reserved", "waiting"] {
      let count = super::pool::requests(&ground.db, &[state]).await?.len();

      pool.insert(state.to_owned(), json!(count));
    }

    let now = now_ms();
    let mut usable: Vec<Value> = vec![];

    for account in accounts(&ground.db).await? {
      let open = account.enabled && account_open(Judging::of(ground), &account.id, now).await?;

      usable.push(
        json!({ "id": account.id, "name": account.name, "enabled": account.enabled, "usable": open }),
      );
    }

    Ok(answer(
      StatusCode::OK,
      json!({
        "pid": ready.pid,
        "code": ready.code,
        "ready": ready.ready,
        "nodes": nodes,
        "pool": pool,
        "accounts": usable,
      }),
    ))
  }
}

crate::moved!(Health);
