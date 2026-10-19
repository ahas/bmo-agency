//! The cache-warmer crate's tests: the owner's warming switch on a scratch store of the test's own and over
//! the control route that reads and throws it, and when a kept prefix is worth a ping.

#[path = "../support/scratch.rs"]
mod scratch;

mod due;
mod switch;
