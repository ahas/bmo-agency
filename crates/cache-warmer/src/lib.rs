//! The keepalive schedule's general rule (docs/keepalive-schedule.md, 「The general rule」): when a kept
//! prompt-cache prefix is due a ping, in what order a pass pings its targets, and the owner's switches as
//! values. Everything is judged from what it is handed, `now` included; the crate reads no clock, no store
//! and no policy, and depends on no other crate. Its public names are agency's boundary (docs/api.md,
//! 「The keepalive schedule library」).

mod clocks;
mod plan;
mod reason;
mod switches;

pub use clocks::{Clocks, Record, usable};
pub use plan::{Candidate, Group, Pass, plan};
pub use reason::first_units;
pub use switches::{is_on, use_clears_pause};
