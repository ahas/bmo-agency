//! The Claude accounts: a setting read as a whole number, a credential's stated end, and a login closed
//! by a window of its own the cache siren reads full while its reading stands.

use agency_server::control::claude_accounts::*;
use agency_server::now_ms;
use agency_server::peers::cache_siren::{CacheSiren, UsageWindows, Window};
use agency_server::peers::{Asking, PeerFault};

use crate::scratch::Scratch;

/// A cache siren holding one account's five-hour window at a share, read some seconds ago, standing for a
/// span; or one that cannot be asked.
enum Siren {
  Reads {
    used: f64,
    age: i64,
    fresh: u64,
    resets_at: i64,
  },
  Down,
}

impl CacheSiren for Siren {
  fn usage_windows<'a>(&'a self, _mode: &'a str, _account: &'a str) -> Asking<'a, UsageWindows> {
    Box::pin(async move {
      match self {
        Self::Reads {
          used,
          age,
          fresh,
          resets_at,
        } => Ok(UsageWindows {
          windows: vec![Window {
            name: "five_hour".into(),
            utilization: *used,
            resets_at: Some(*resets_at),
            seen_at: now_ms() / 1000 - age,
            duration_minutes: Some(300),
            limit_name: None,
          }],
          fresh_seconds: *fresh,
        }),
        Self::Down => Err(PeerFault::Unreachable {
          peer: "cache siren",
          detail: "connection refused".into(),
        }),
      }
    })
  }

  fn cache_lifetime<'a>(&'a self, _mode: &'a str, _model: &'a str) -> Asking<'a, Option<i64>> {
    Box::pin(async { Ok(None) })
  }
}

async fn rest_of(db: &sea_orm::DatabaseConnection, siren: &Siren) -> Option<i64> {
  rest_until(Judging { db, siren }, "spare").await.unwrap()
}

#[tokio::test]
async fn a_login_rests_while_the_siren_reads_its_window_full_and_its_reading_stands() {
  let scratch = Scratch::new().await;
  let resets_at = now_ms() / 1000 + 600;
  let rest = |siren| rest_of(scratch.db(), siren);

  let full = Siren::Reads {
    used: 1.0,
    age: 10,
    fresh: 1800,
    resets_at,
  };
  let stale = Siren::Reads {
    used: 1.0,
    age: 3600,
    fresh: 1800,
    resets_at,
  };
  let short = Siren::Reads {
    used: 0.5,
    age: 10,
    fresh: 1800,
    resets_at,
  };
  let unkept = Siren::Reads {
    used: 1.0,
    age: 10,
    fresh: 0,
    resets_at,
  };

  assert_eq!(rest(&full).await, Some(resets_at));
  assert_eq!(rest(&stale).await, None, "a reading past its span closes nothing");
  assert_eq!(rest(&short).await, None);
  assert_eq!(rest(&unkept).await, None, "no reading stands where no reader keeps the windows");
  assert_eq!(rest(&Siren::Down).await, None, "a siren that cannot be asked closes nothing");
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
    credential_kind: "directory".into(),
    credential_path: dir.path().display().to_string(),
    needs_login_at: Some(1),
    user_id: None,
  };

  std::fs::write(
    dir.path().join(CREDENTIAL_FILE),
    r#"{"claudeAiOauth":{"expiresAt":1790000000000,"refreshToken":"r"},"other":{"tokenExpiry":1780000000},"small":{"expires":5}}"#,
  )
  .unwrap();

  assert_eq!(credential_expiry(&account), Some(1_780_000_000_000.0));
}
