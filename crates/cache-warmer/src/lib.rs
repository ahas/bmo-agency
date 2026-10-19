//! Agency's keepalive schedule (docs/keepalive-schedule.md): whether its keepalive pings go, by the owner's
//! warming switch, and when a kept prefix is worth a ping, by its usable lifetime and its idle stretch.
//! Which places stand, the ping itself and its record are the pool's.

pub mod due;
pub mod switch;

pub use due::{Prefix, usable};
pub use switch::{pings_go, set_stopped, stopped};
