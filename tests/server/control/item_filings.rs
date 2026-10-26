//! An item a chat agent files (docs/api.md, 「assignments and jobs」): its room the turn's, `asked-by=owner` stamped
//! only where the turn is the owner's, any such label the call names let go, and no item for an agent carrying no
//! turn.

use agency_server::control::item_filings::item_of;
use agency_server::turn_marks::{self, Mark};
use bmo_agency_client::ItemFilingAsk;
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

fn ask(labels: &[(&str, &str)]) -> ItemFilingAsk {
  ItemFilingAsk {
    agent_name: "chat-agent".to_owned(),
    title: "Mend the guard".to_owned(),
    brief: "The guard refuses a good command.".to_owned(),
    queue_name: "todo".to_owned(),
    priority: None,
    assignee_name: Some("helper".to_owned()),
    requester_name: Some("a member".to_owned()),
    labels: labels.iter().map(|(key, value)| ((*key).to_owned(), (*value).to_owned())).collect(),
  }
}

#[test]
fn the_owner_s_turn_stamps_his_ask_and_the_room_is_the_turn_s() {
  let owner = item_of(
    &ask(&[("lane", "repair")]),
    Mark {
      chat_id: -9_000_000_000_003,
      privileged: true,
    },
  );

  assert_eq!(owner["scope"], "-9000000000003");
  assert_eq!(owner["labels"], json!({ "asked-by": "owner", "lane": "repair" }));
  assert_eq!((&owner["assigneeName"], &owner["queueName"]), (&json!("helper"), &json!("todo")));
  assert!(owner.get("priority").is_none(), "a field the call left out is left out");

  // Anyone else's turn files a draft: no stamp, and an `asked-by` the call names is let go.
  let other = item_of(
    &ask(&[("asked-by", "owner"), ("lane", "repair")]),
    Mark {
      chat_id: -5,
      privileged: false,
    },
  );

  assert_eq!(other["labels"], json!({ "lane": "repair" }));
  assert_eq!(other["scope"], "-5");
}

#[test]
fn a_turn_s_mark_stands_until_that_turn_ends_and_a_newer_one_stands_past_an_older_end() {
  let mark = Mark {
    chat_id: 7,
    privileged: true,
  };

  turn_marks::carrying("marked-agent", "run-1", mark);
  assert_eq!(turn_marks::of("marked-agent"), Some(mark));

  turn_marks::carrying(
    "marked-agent",
    "run-2",
    Mark {
      privileged: false,
      ..mark
    },
  );
  turn_marks::ended("marked-agent", "run-1");
  assert_eq!(
    turn_marks::of("marked-agent"),
    Some(Mark {
      privileged: false,
      ..mark
    })
  );

  turn_marks::ended("marked-agent", "run-2");
  assert_eq!(turn_marks::of("marked-agent"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_carrying_no_turn_files_no_item() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let answer = reqwest::Client::new()
    .post(format!("http://127.0.0.1:{}/item-filings", server.port))
    .bearer_auth(token)
    .json(&ask(&[]))
    .send()
    .await
    .unwrap();
  let status = answer.status().as_u16();
  let body: Value = answer.json().await.unwrap_or(Value::Null);

  assert_eq!((status, body["code"].as_str()), (409, Some("no-turn")), "{body}");
}
