//! The migration that leaves every supervisor on one road (docs/provider-order.md, 「What a road is」): each
//! supervisor keeps the first entry of the order it held as its road, one whose order held none names no road,
//! the order, its share and its forced provider go, and the marks of a stand-in answering and of a supervisor's
//! walk go with them.

use sea_orm_migration::MigratorTrait;
use store::migration::{Migrator, SUPERVISOR_ROAD};

use crate::support::scratch::Scratch;

#[tokio::test]
async fn every_supervisor_keeps_the_first_entry_of_its_order_as_its_one_road_and_the_stand_in_s_marks_go() {
  let scratch = Scratch::blank().await;
  let steps = Migrator::migrations()
    .iter()
    .take_while(|migration| migration.name() != SUPERVISOR_ROAD)
    .count();

  Migrator::up(scratch.db(), Some(steps as u32)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "supervisors" ("name", "session_scope", "turn_shape", "provider_order", "quota_share",
           "active_provider", "active_model", "created_at", "updated_at")
         VALUES ('walker', 'agent', 'window',
           '[{"provider":"deepseek","model":"deepseek-flash","effort":"high","thinking":null,"answerTokens":null},
             {"provider":"registered","model":"flash-next","effort":null,"thinking":"on","answerTokens":4096},
             {"provider":"claude-code","model":"claude-opus-5-5","effort":"medium","thinking":"on"}]',
           0.05, 'claude-code', 'claude-opus-5-5', 0, 0),
                ('registered-one', 'room', 'pieces',
           '[{"provider":"registered","model":"flash-next","thinking":"off","answerTokens":2048}]', 0.01, NULL, NULL,
           0, 0),
                ('empty', 'room', 'pieces', '[]', 0.01, NULL, NULL, 0, 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('fallback_since', '1793158662'), ('fallback_provider', 'deepseek'),
                ('road_kept:supervisor:walker', '{}'), ('road_kept:role:worker', '{}'), ('rest_kind', 'limit')"#,
      vec![],
    )
    .await;

  Migrator::up(scratch.db(), None).await.unwrap();

  let road = |name: &'static str| {
    let db = scratch.db().clone();

    async move {
      let row = store::one(
        &db,
        r#"SELECT "provider", "model", "effort", "thinking", "answer_tokens" FROM "supervisors" WHERE "name" = $1"#,
        vec![name.into()],
      )
      .await
      .unwrap()
      .unwrap();
      let text = |column: &str| row.try_get::<Option<String>>("", column).unwrap();

      (
        text("provider"),
        text("model"),
        text("effort"),
        text("thinking"),
        row.try_get::<Option<i64>>("", "answer_tokens").unwrap(),
      )
    }
  };
  let some = |text: &str| Some(text.to_owned());

  assert_eq!(
    road("walker").await,
    (some("deepseek"), some("deepseek-flash"), some("high"), None, None)
  );
  assert_eq!(
    road("registered-one").await,
    (some("registered"), some("flash-next"), None, some("off"), Some(2048))
  );
  assert_eq!(road("empty").await, (None, None, None, None, None), "an order of none names no road");

  for gone in ["provider_order", "quota_share", "active_provider", "active_model"] {
    assert_eq!(
      scratch
        .count(
          r#"SELECT count(*) FROM information_schema.columns
             WHERE table_name = 'supervisors' AND column_name = $1 AND table_schema = current_schema()"#,
          vec![gone.into()],
        )
        .await,
      0,
      "{gone}"
    );
  }

  let mut kept: Vec<String> =
    store::all(scratch.db(), r#"SELECT "key" FROM "settings" ORDER BY "key""#, vec![])
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get::<String>("", "key").unwrap())
      .filter(|key| key.starts_with("fallback") || key.starts_with("road_kept") || key == "rest_kind")
      .collect();

  kept.sort();
  assert_eq!(kept, ["rest_kind", "road_kept:role:worker"], "a role's kept road and the rest stay");
}
