//! The owner's switches as values (docs/keepalive-schedule.md, 「The general rule」): whether a target
//! warms, and whether a use clears a pause. Where the switches are kept is the caller's.

/// Whether a target warms: its subject's switch where thrown, else its registration's default, else the
/// policy's.
pub fn is_on(switch: Option<bool>, warm_by_default: Option<bool>, policy_default: bool) -> bool {
  switch.or(warm_by_default).unwrap_or(policy_default)
}

/// Whether a use report clears a pause: only work that began after the pause was set does.
pub fn use_clears_pause(paused_at: i64, started_at: i64) -> bool {
  started_at > paused_at
}
