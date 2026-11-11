//! `bmo-agency node person`: the node each account's runs stand on, listed, set or taken away
//! (docs/person-nodes.md, 「Which node is a person's」), as the binding service sets it through the boundary.

use std::path::Path;

use super::{Answer, VerbError, service};
use crate::person_nodes;

/// Every account's node with no account named; the node named set for the account, or its node taken away.
pub async fn person_node(
  data_root: &Path,
  account: Option<String>,
  node: Option<String>,
  clear: bool,
) -> Result<Answer, VerbError> {
  let service = service(data_root).await?;
  let db = &service.db;
  let refused = |refusal: crate::Refusal| VerbError::Refused(refusal.detail);

  match (account, node, clear) {
    (None, None, false) => {
      let every = person_nodes::every(db).await.map_err(refused)?;

      if every.is_empty() {
        return Ok(Answer::from(vec!["no account has a node".to_owned()]));
      }

      Ok(Answer::from(
        every.iter().map(|one| format!("{:<12} {}", one.owner_account, one.node)).collect::<Vec<_>>(),
      ))
    }
    (Some(account), None, true) => {
      if !person_nodes::clear(db, &account).await.map_err(refused)? {
        return Err(VerbError::Refused(format!("account {account} has no node")));
      }

      Ok(Answer::from(vec![format!("account {account}: its runs stand on this machine again")]))
    }
    (Some(account), Some(node), false) => {
      let unanswered = |fault: crate::peers::PeerFault| VerbError::Refused(fault.to_string());

      if let Some(why) = person_nodes::refusal(service.peers.node.as_ref(), &node).await.map_err(unanswered)? {
        return Err(VerbError::Refused(why));
      }

      person_nodes::set(db, &account, &node).await.map_err(refused)?;

      Ok(Answer::from(vec![format!("account {account}: its runs stand on {node}")]))
    }
    _ => Err(VerbError::Refused(
      "name an account and a node to set, an account with --clear to take its node away, or nothing to list".into(),
    )),
  }
}
