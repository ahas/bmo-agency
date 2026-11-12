//! Who reaches a node, read over the control routes of a running server (docs/api.md, 「person-nodes」): the
//! answer a node service's tool reads as agency's client types it, and a caller left unnamed refused.

use agency_server::person_nodes;
use bmo_agency_client::NodeAccess;

use crate::support::rows::supervisor;
use crate::support::scratch::Scratch;
use crate::support::standing;

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_who_reaches_a_node() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  supervisor(scratch.db(), "u7", &[("owner_account", 7i64.into())]).await;
  person_nodes::set(scratch.db(), "7", "pc-7").await.unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let client = reqwest::Client::new();
  let ask = |query: &'static str| {
    client
      .get(format!("http://127.0.0.1:{}/nodes/pc-7/access{query}", server.port))
      .bearer_auth(token)
      .send()
  };

  let answer = ask("?agent=u7&checkout=%2Fwork").await.unwrap();

  assert_eq!(answer.status().as_u16(), 200);
  assert_eq!(
    answer.json::<NodeAccess>().await.unwrap(),
    NodeAccess {
      node: "pc-7".into(),
      owners: vec!["7".into()],
      agent: "u7".into(),
      account: Some("7".into()),
      administrator: false,
      reaches: true,
    }
  );
  assert_eq!(ask("?agent=%20").await.unwrap().status().as_u16(), 400);
}
