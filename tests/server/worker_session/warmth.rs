//! Whether a worker's session is still warm: the cache siren's lifetime first, the provider's declared one
//! where the siren answers none or cannot be asked, and a record retired once its cache has gone.

use agency_server::peers::cache_siren::{CacheSiren, UsageWindows};
use agency_server::peers::{Asking, PeerFault};
use agency_server::worker_session::warmth::{lifetime_ms, record_ttl_ms, set_aside_lifetime_ms, warm};
use policy::providers::claude_code::CLAUDE_CODE;

/// A cache siren answering one lifetime, none, or no answer at all.
enum Siren {
  Judges(i64),
  JudgesNone,
  Down,
}

impl CacheSiren for Siren {
  fn usage_windows<'a>(&'a self, _mode: &'a str, _account: &'a str) -> Asking<'a, UsageWindows> {
    Box::pin(async { Ok(UsageWindows::default()) })
  }

  fn cache_lifetime<'a>(&'a self, _mode: &'a str, _model: &'a str) -> Asking<'a, Option<i64>> {
    Box::pin(async move {
      match self {
        Self::Judges(minutes) => Ok(Some(*minutes)),
        Self::JudgesNone => Ok(None),
        Self::Down => Err(PeerFault::Unreachable {
          peer: "cache siren",
          detail: "connection refused".into(),
        }),
      }
    })
  }
}

#[tokio::test]
async fn the_lifetime_is_the_sirens_and_the_providers_declared_one_where_the_siren_names_none() {
  let declared = CLAUDE_CODE.cache.ttl_minutes * 60_000;

  assert_eq!(lifetime_ms(&Siren::Judges(5), "claude-code", "model-a").await, Some(5 * 60_000));
  assert_eq!(lifetime_ms(&Siren::JudgesNone, "claude-code", "model-a").await, Some(declared));
  assert_eq!(lifetime_ms(&Siren::Down, "claude-code", "model-a").await, Some(declared));
  assert_eq!(lifetime_ms(&Siren::JudgesNone, "no-such-provider", "model-a").await, None);
}

#[tokio::test]
async fn a_session_set_aside_is_warm_inside_its_lifetime_and_an_hour_where_none_is_named() {
  let hour = 60 * 60_000;

  assert_eq!(set_aside_lifetime_ms(&Siren::JudgesNone, "no-such-provider", "model-a").await, hour);
  assert_eq!(
    set_aside_lifetime_ms(&Siren::Judges(5), "no-such-provider", "model-a").await,
    5 * 60_000
  );
  assert!(warm(0, hour - 60_000, hour));
  assert!(!warm(0, hour, hour));
}

#[test]
fn a_record_is_retired_at_its_cache_lifetime_or_the_deployments_life_where_that_is_sooner() {
  assert_eq!(record_ttl_ms(None, Some(3_600_000)), Some(3_600_000));
  assert_eq!(record_ttl_ms(Some(600_000), Some(3_600_000)), Some(600_000));
  assert_eq!(record_ttl_ms(Some(7_200_000), Some(3_600_000)), Some(3_600_000));
  assert_eq!(record_ttl_ms(Some(600_000), None), Some(600_000));
  assert_eq!(record_ttl_ms(None, None), None);
}
