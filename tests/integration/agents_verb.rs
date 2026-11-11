//! A repository's support checkout count as `bmo-agency agent --set <repository> --support <n>` writes it
//! (docs/worker-slots.md, 「The base checkout and the support checkouts」), and the counts and names it
//! refuses.

use std::path::Path;

use agency_server::verbs::agents::{AgentsArgs, AgentsHands, Said, Stand, agents};
use futures_util::future::BoxFuture;
use harness_providers::Machine;
use sea_orm::DatabaseConnection;

use crate::integration::common::Scratch;
use crate::support::grounds::ground_on;
use crate::support::rows::repository;

// The verb changes rows alone, so its workbench, worker notice and standing input are never asked.
struct NoHands;

impl AgentsHands for NoHands {
  fn settle<'a>(&'a self, _worker: &'a str) -> BoxFuture<'a, ()> {
    Box::pin(async {})
  }

  fn running(&self) -> BoxFuture<'_, Option<Vec<String>>> {
    Box::pin(async { None })
  }

  fn standing_input(&self) -> String {
    String::new()
  }
}

// One call of the verb, `--set <name> --support <count>`.
async fn ask(db: &DatabaseConnection, home: &Path, name: &str, support: &str) -> Said {
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/nowhere".to_owned()),
    ],
    home.join("data"),
  );
  let ground = ground_on(db.clone(), home);
  let hands = NoHands;
  let stand = Stand {
    ground: &ground,
    machine: &machine,
    hands: &hands,
    home,
  };
  let args = AgentsArgs {
    set: Some(name.to_owned()),
    support: Some(support.to_owned()),
    ..AgentsArgs::default()
  };

  agents(&stand, &args).await.unwrap()
}

// The repository's support count as its row holds it now.
async fn support_of(scratch: &Scratch, name: &str) -> i64 {
  scratch
    .count(r#"SELECT "support_count" FROM "repositories" WHERE "name" = $1"#, vec![name.into()])
    .await
}

#[tokio::test]
async fn a_support_count_given_on_the_verb_is_written_to_the_repository() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  repository(scratch.db(), "video", "/checkouts/video", &[]).await;

  assert_eq!(support_of(&scratch, "video").await, 0, "a repository starts with none");

  let said = ask(scratch.db(), home.path(), "video", "3").await;

  assert_eq!(said.ended, None, "{said:?}");
  assert_eq!(support_of(&scratch, "video").await, 3);

  let said = ask(scratch.db(), home.path(), "video", "0").await;

  assert_eq!(said.ended, None, "{said:?}");
  assert_eq!(support_of(&scratch, "video").await, 0);
}

#[tokio::test]
async fn a_support_count_outside_the_range_is_refused_and_writes_nothing() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  repository(scratch.db(), "video", "/checkouts/video", &[]).await;

  for count in ["9", "-1", "1.5", "many"] {
    let said = ask(scratch.db(), home.path(), "video", count).await;

    assert_eq!(said.ended.as_ref().map(|(code, _)| *code), Some(1), "{count}: {said:?}");
    assert_eq!(support_of(&scratch, "video").await, 0, "{count}");
  }
}

#[tokio::test]
async fn a_name_that_is_no_repository_is_refused() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();

  let said = ask(scratch.db(), home.path(), "nowhere", "1").await;

  assert_eq!(said.ended.as_ref().map(|(code, _)| *code), Some(1), "{said:?}");
}
