//! The store's tests: its migrations, its seeds, its readers and its statements, each on a scratch
//! database of the test's own on the suite's PostgreSQL server.

#[path = "../support/scratch.rs"]
mod scratch;

mod migration;
mod read;
mod registry;
mod seed;
