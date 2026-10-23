//! The Claude accounts: a setting read as a whole number, a credential's stated end, a login closed by a
//! window of its own agency keeps read full while its reading stands, and each account's view carrying its
//! own two headline windows whichever road the chat stands on.

use std::sync::Arc;
use std::time::Duration;

use agency_server::control::claude_accounts::*;
use agency_server::now_ms;
use cache_siren::reader::Passing;
use cache_siren::{Keeping, Reader, Slot, Window};

use crate::scratch::Scratch;

/// A reader of Claude Code's windows whose reading stands for half an hour, and which passes over nothing.
struct HalfHour;

impl Reader for HalfHour {
  fn mode(&self) -> &str {
    "claude-code"
  }

  fn cadence(&self) -> Duration {
    Duration::from_secs(300)
  }

  fn fresh(&self) -> Duration {
    Duration::from_secs(1800)
  }

  fn pass<'a>(&'a self, _keeping: &'a Keeping, _now_ms: i64) -> Passing<'a> {
    Box::pin(async {})
  }
}

const SPARE: Slot<'static> = Slot {
  mode: "claude-code",
  account: "spare",
};

#[tokio::test]
async fn a_login_rests_while_a_window_of_its_own_is_kept_full_and_its_reading_stands() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let now = now_ms() / 1000;
  let resets_at = now + 600;
  let read = Keeping::new(db.clone(), vec![Arc::new(HalfHour)]);
  let unread = Keeping::new(db.clone(), vec![]);
  let five_hour = |used: f64| Window {
    name: "five_hour".into(),
    utilization: used,
    resets_at: Some(resets_at),
    duration_minutes: Some(300),
    limit_name: None,
  };
  let rest = |windows| async move { rest_until(Judging { db, windows }, "spare").await.unwrap() };

  read.replace(SPARE, &[five_hour(1.0)], now - 10).await.unwrap();

  assert_eq!(rest(&read).await, Some(resets_at));
  assert_eq!(rest(&unread).await, None, "no reading stands where no reader keeps the windows");

  read.mark_unread("spare", now - 10).await.unwrap();

  assert_eq!(rest(&read).await, None, "a reading seen at the account's unread mark closes nothing");

  read.lift_unread("spare").await.unwrap();
  read.replace(SPARE, &[five_hour(1.0)], now - 3600).await.unwrap();

  assert_eq!(rest(&read).await, None, "a reading past its span closes nothing");

  read.replace(SPARE, &[five_hour(0.5)], now - 10).await.unwrap();

  assert_eq!(rest(&read).await, None);

  scratch.run(r#"DROP TABLE "usage_windows""#, vec![]).await;

  assert_eq!(rest(&read).await, None, "a keeping that cannot be read closes nothing");
}

#[tokio::test]
async fn each_account_s_view_carries_its_own_headline_windows_and_an_endpoint_account_none() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let windows = Keeping::new(db.clone(), vec![Arc::new(HalfHour)]);
  let window = |name: &str, used: f64| Window {
    name: name.into(),
    utilization: used,
    resets_at: None,
    duration_minutes: None,
    limit_name: None,
  };

  scratch
    .run(
      r#"INSERT INTO "claude_accounts"
           ("id", "name", "place", "enabled", "credential_kind", "credential_path", "added_at",
            "endpoint_url", "key_file")
         VALUES ('main', 'one', 1, 1, 'directory', '/logins/one', 0, NULL, NULL),
                ('spare', 'two', 2, 0, 'token', '/logins/two.token', 0, NULL, NULL),
                ('paid', 'three', 3, 1, 'endpoint', '/logins/paid', 0, 'https://endpoint.invalid', '/keys/paid')"#,
      vec![],
    )
    .await;
  windows
    .replace(
      Slot {
        mode: "claude-code",
        account: "main",
      },
      &[window("five_hour", 0.25), window("seven_day", 0.5)],
      100,
    )
    .await
    .unwrap();
  windows.replace(SPARE, &[window("seven_day", 0.75)], 100).await.unwrap();
  windows
    .replace(
      Slot {
        mode: "claude-code",
        account: "paid",
      },
      &[window("five_hour", 1.0)],
      100,
    )
    .await
    .unwrap();

  let views = views(Judging {
    db,
    windows: &windows,
  })
  .await
  .unwrap();
  let of = |id: &str| views.iter().find(|view| view["id"] == id).unwrap();

  assert_eq!(of("main")["session"]["utilization"], 0.25);
  assert_eq!(of("main")["session"]["seenAt"], 100);
  assert_eq!(of("main")["week"]["utilization"], 0.5);
  assert_eq!(
    of("spare")["session"],
    serde_json::Value::Null,
    "an account off still answers what it holds"
  );
  assert_eq!(of("spare")["week"]["utilization"], 0.75);
  assert_eq!(of("paid")["session"], serde_json::Value::Null, "an endpoint account holds no windows");
  assert_eq!(of("paid")["week"], serde_json::Value::Null);
}

#[test]
fn a_setting_reads_as_parseint_would() {
  assert_eq!(leading_integer("1790000000"), Some(1_790_000_000));
  assert_eq!(leading_integer(" 12abc"), Some(12));
  assert_eq!(leading_integer("-5"), Some(-5));
  assert_eq!(leading_integer("x1"), None);
}

#[test]
fn the_earliest_moment_under_an_expiring_name_answers_in_milliseconds() {
  let dir = tempfile::tempdir().unwrap();
  let account = Account {
    id: "a".into(),
    name: "a".into(),
    place: 1,
    enabled: true,
    credential: Credential::Directory {
      home: dir.path().into(),
    },
    needs_login_at: Some(1),
    user_id: None,
    base_prompt: None,
  };

  std::fs::write(
    dir.path().join(CREDENTIAL_FILE),
    r#"{"claudeAiOauth":{"expiresAt":1790000000000,"refreshToken":"r"},"other":{"tokenExpiry":1780000000},"small":{"expires":5}}"#,
  )
  .unwrap();

  assert_eq!(credential_expiry(&account), Some(1_780_000_000_000.0));
}
