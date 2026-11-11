//! An account's node across the boundary (docs/api.md, 「person-nodes」): the record of the node an account's runs
//! stand on, read, set by the service that binds the account to its machine, and taken away.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::peers::PeerFault;
use crate::person_nodes;

fn account_of(call: &Call) -> Result<String, Refused> {
  call
    .param("ownerAccount")
    .map(str::trim)
    .filter(|account| !account.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| Refused::new(StatusCode::BAD_REQUEST, "an account is required"))
}

fn unanswered(fault: PeerFault) -> Refused {
  Refused::new(StatusCode::BAD_GATEWAY, fault.to_string()).with_code("upstream-failed")
}

/// `GET /person-nodes`: every account's node, by account.
pub struct ListPersonNodes;

impl Route for ListPersonNodes {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/person-nodes";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let items = person_nodes::every(call.store()).await?;

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(ListPersonNodes);

/// `PUT /person-nodes/{ownerAccount}`: an account's node set, a hub node the node service keeps.
pub struct SetPersonNode;

impl Route for SetPersonNode {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/person-nodes/{ownerAccount}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let account = account_of(&call)?;
    let body = call.body().await?;
    let node = body
      .get("node")
      .and_then(Value::as_str)
      .map(str::trim)
      .filter(|node| !node.is_empty())
      .ok_or_else(|| Refused::new(StatusCode::BAD_REQUEST, "node (string) is required"))?
      .to_owned();

    if let Some(why) = person_nodes::refusal(call.ground().peers.node.as_ref(), &node)
      .await
      .map_err(unanswered)?
    {
      return Err(Refused::new(StatusCode::UNPROCESSABLE_ENTITY, why).with_code("not-a-hub-node"));
    }

    let set = person_nodes::set(call.store(), &account, &node).await?;

    Ok(answer(StatusCode::OK, json!(set)))
  }
}

crate::moved!(SetPersonNode);

/// `DELETE /person-nodes/{ownerAccount}`: an account's node taken away.
pub struct ClearPersonNode;

impl Route for ClearPersonNode {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/person-nodes/{ownerAccount}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let account = account_of(&call)?;

    if !person_nodes::clear(call.store(), &account).await? {
      return Err(
        Refused::new(StatusCode::NOT_FOUND, format!("account {account} has no node"))
          .with_code("not-found"),
      );
    }

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(ClearPersonNode);
