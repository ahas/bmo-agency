//! What the modules of the test harness share: a workspace binary built for a test to start, a scratch
//! store of a test's own, a ground seeded on one, the supervisor and repository rows a test lays down,
//! agency's own server standing on one on a seat of the test server's connections, and a front that cuts
//! or holds the calls to a stand-in.

pub mod binary;
pub mod connections;
pub mod cut;
pub mod grounds;
pub mod peers;
pub mod rows;
pub mod scratch;
pub mod standing;
pub mod wording;
