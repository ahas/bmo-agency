//! The cache-siren crate's tests: the keeping of the accounts' usage windows on a scratch store of the
//! test's own, and the cache-lifetime judgement.

#[path = "../support/scratch.rs"]
mod scratch;

mod lifetime;
mod windows;
