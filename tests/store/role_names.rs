//! The roles under the names the owner gave them (docs/agent-roles.md, 「The roles the policy seeds」): a
//! store's rows keyed on `delegator`, `searcher`, `browser` and `video` are keyed on `doppelganger`,
//! `voyager`, `seeker` and `producer`, a label the policy gave takes the new name's while one the owner set
//! stays, and `digest` and `helper` go with what they held.

use sea_orm_migration::MigratorTrait;
use store::migration::{Migrator, ROLE_NAMES};

use crate::support::scratch::Scratch;

// A blank store brought to the shape before the roles took their names.
async fn before() -> Scratch {
  let scratch = Scratch::blank().await;
  let steps = Migrator::migrations()
    .iter()
    .take_while(|migration| migration.name() != ROLE_NAMES)
    .count();

  Migrator::up(scratch.db(), Some(u32::try_from(steps).unwrap())).await.unwrap();

  scratch
}

async fn role(scratch: &Scratch, name: &str, label: &str, prompt: Option<&str>) {
  scratch
    .run(
      r#"INSERT INTO "roles" ("name", "label", "max_concurrent", "prompt", "position", "created_at", "updated_at")
         VALUES ($1, $2, 2, $3, 0, 0, 0)"#,
      vec![name.into(), label.into(), prompt.map(str::to_owned).into()],
    )
    .await;
  scratch
    .run(r#"INSERT INTO "role_seeded" ("name") VALUES ($1)"#, vec![name.into()])
    .await;
}

async fn slot(scratch: &Scratch, token: &str, role: &str, state: &str, child: Option<&str>) {
  scratch
    .run(
      r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "provider", "model",
           "effort", "capacity", "state", "enqueued_at", "child_id")
         VALUES ($1, $1, $1, 'search', $2, 'claude-code', 'claude-opus-5', 'high', 1, $3, 0, $4)"#,
      vec![token.into(), role.into(), state.into(), child.map(str::to_owned).into()],
    )
    .await;
}

async fn names(scratch: &Scratch, sql: &str) -> Vec<String> {
  store::all(scratch.db(), sql, vec![])
    .await
    .unwrap()
    .iter()
    .map(|row| row.try_get::<String>("", "name").unwrap())
    .collect()
}

#[tokio::test]
async fn the_roles_and_everything_keyed_on_them_take_their_new_names_and_the_retired_roles_go() {
  let scratch = before().await;
  let prompt =
    "You are the delegator: the double of the supervisor that handed you this task. You carry its tools.";

  role(&scratch, "worker", "Worker", None).await;
  role(&scratch, "delegator", "Delegator", Some(prompt)).await;
  role(&scratch, "searcher", "Searcher", None).await;
  role(&scratch, "browser", "The owner's browser", None).await;
  role(&scratch, "video", "Video producer", Some("VIDEO PRODUCER — what you are here.")).await;
  role(&scratch, "digest", "Digest", None).await;
  role(&scratch, "helper", "Helper", None).await;
  slot(&scratch, "s1", "searcher", "active", Some("searcher@run:14")).await;
  slot(&scratch, "s2", "searcher", "waiting", None).await;
  slot(&scratch, "d1", "digest", "released", None).await;
  scratch
    .run(
      r#"INSERT INTO "role_slot_reservations" ("token", "reservation_key", "owner", "source", "role", "provider",
           "model", "effort", "wanted", "state", "created_at")
         VALUES ('r1', 'r1', 'r1', 'cowork', 'digest', 'claude-code', 'claude-sonnet-5', 'low', 1, 'released', 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "role_sessions" ("role", "path", "provider", "model", "command_key", "session", "opened_at")
         VALUES ('delegator', '/scratch', 'deepseek', 'deepseek-flash', 'k', 'one', 0),
                ('helper', '/scratch', 'claude-code', 'claude-sonnet-5', 'k', 'two', 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("agent", "run", "at", "kind", "text") VALUES ('searcher', 'searcher@run:14', 1, 'text', 'x')"#,
      vec![],
    )
    .await;

  // The renaming alone: a later migration takes the double's role away again.
  Migrator::up(scratch.db(), Some(1)).await.unwrap();

  assert_eq!(
    names(&scratch, r#"SELECT "name" FROM "roles" ORDER BY "name""#).await,
    ["doppelganger", "producer", "seeker", "voyager", "worker"]
  );
  assert_eq!(
    names(&scratch, r#"SELECT "name" FROM "role_seeded" ORDER BY "name""#).await,
    ["doppelganger", "producer", "seeker", "voyager", "worker"],
    "a role given under its old name is given under its new one, so no seed gives it again"
  );
  assert_eq!(
    names(&scratch, r#"SELECT "label" AS "name" FROM "roles" ORDER BY "name""#).await,
    ["Doppelganger", "Producer", "The owner's browser", "Voyager", "Worker"],
    "a label the policy gave takes the new name's; one the owner set stays"
  );

  let double =
    names(&scratch, r#"SELECT "prompt" AS "name" FROM "roles" WHERE "name" = 'doppelganger'"#).await;
  assert_eq!(
    double,
    ["You are the double of the supervisor that handed you this task. You carry its tools."]
  );

  assert_eq!(
    names(&scratch, r#"SELECT "role" || ' ' || COALESCE("child_id", '-') AS "name" FROM "role_slot_requests" ORDER BY "token""#)
      .await,
    ["voyager searcher@run:14", "voyager -"],
    "a standing slot takes the new name and keeps the run's name it is bound to; the retired role's slot goes"
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "role_slot_reservations""#, vec![]).await, 0);
  assert_eq!(
    names(&scratch, r#"SELECT "role" AS "name" FROM "role_sessions""#).await,
    ["doppelganger"]
  );
  assert_eq!(names(&scratch, r#"SELECT "agent" AS "name" FROM "agent_log""#).await, ["voyager"]);
}

#[tokio::test]
async fn a_retired_role_a_slot_still_stands_under_stays_for_the_owner() {
  let scratch = before().await;

  role(&scratch, "helper", "Helper", None).await;
  slot(&scratch, "h1", "helper", "active", None).await;

  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(names(&scratch, r#"SELECT "name" FROM "roles""#).await, ["helper"]);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "role_slot_requests""#, vec![]).await, 1);
}
