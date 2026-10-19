//! The nodes as the job unit reads them, of a running server deployed with no node service: no node, no
//! reach held, and the copy of a checkout on a node where the node's link keeps its shadow here.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_the_nodes_their_reach_and_a_checkout_s_copy() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  assert_eq!(get(port, token, "/nodes").await, (200, json!([])));

  let (status, reach) = get(port, token, "/nodes/near/reach").await;

  assert_eq!((status, reach["code"].as_str()), (404, Some("nothing-held")), "{reach}");

  let shadow = data.join("nodes").join("near").join("shadow").join("-srv-checkout");

  assert_eq!(
    get(port, token, "/nodes/near/copy?checkout=%2Fsrv%2Fcheckout").await,
    (200, json!({ "path": shadow.to_string_lossy() }))
  );
  assert_eq!(get(port, token, "/nodes/near/copy").await.0, 400);
}
