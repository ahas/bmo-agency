//! Whether a worker's session is still warm (docs/sessions.md, 「A worker's session per model」): the time
//! since its last answer against the cache lifetime the cache siren judges its provider and model by, or the
//! lifetime the provider declares where the siren answers none. Agency keeps no cache measurement of its own.

use harness_providers::provider;

use crate::peers::cache_siren::CacheSiren;

// The lifetime a session set aside is judged warm by where neither the siren nor its provider names one:
// the subscription's hour.
const SET_ASIDE_DEFAULT_MS: i64 = 60 * 60_000;

/// The cache lifetime of a session on a provider and model, in milliseconds: the cache siren's, else the
/// one the provider declares for the model; none where neither names one. A siren that cannot be asked
/// leaves the declared lifetime to stand.
pub async fn lifetime_ms(siren: &dyn CacheSiren, mode: &str, model: &str) -> Option<i64> {
  let asked = match siren.cache_lifetime(mode, model).await {
    Ok(minutes) => minutes,
    Err(fault) => {
      eprintln!(
        "cache lifetime {mode} {model}: the cache siren was not read, the declared one stands: {fault}"
      );
      None
    }
  };

  asked
    .or_else(|| provider(mode).and_then(|provider| provider.cache_minutes(model)))
    .map(|minutes| minutes * 60_000)
}

/// The lifetime a session set aside is judged warm by: its cache lifetime, or an hour where none is named.
pub async fn set_aside_lifetime_ms(siren: &dyn CacheSiren, mode: &str, model: &str) -> i64 {
  lifetime_ms(siren, mode, model).await.unwrap_or(SET_ASIDE_DEFAULT_MS)
}

/// Whether a session last answered at `last_at` is warm at `now`, both in epoch milliseconds: inside the
/// lifetime, and cold from the lifetime on.
pub fn warm(last_at: i64, now: i64, lifetime: i64) -> bool {
  now - last_at < lifetime
}

/// How long a kept session's record stands unused before it is retired: the cache lifetime, or the life
/// the deployment gives a session where that is sooner; none where neither names one.
pub fn record_ttl_ms(configured: Option<i64>, lifetime: Option<i64>) -> Option<i64> {
  match (configured, lifetime) {
    (Some(configured), Some(lifetime)) => Some(configured.min(lifetime)),
    (configured, lifetime) => configured.or(lifetime),
  }
}
