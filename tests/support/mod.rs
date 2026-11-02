//! What the modules of the test harness share: a workspace binary built for a test to start, a scratch
//! store of a test's own, a ground seeded on one, and agency's own server standing on one.

pub mod binary;
pub mod grounds;
pub mod scratch;
pub mod standing;
