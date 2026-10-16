//! The agent host's tests: its modules' own rules, and its binary started on a scratch data root as a
//! launcher, a collector and a work node's setup meet it.

#[path = "../support/binary.rs"]
mod binary;
#[path = "../support/scratch.rs"]
mod scratch;

mod disk;
mod host;
mod layout;
mod place;
mod policy;
mod trust;
mod view;
