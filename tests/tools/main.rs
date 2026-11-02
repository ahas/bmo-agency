//! Agency's tools (docs/tools.md): their side of the tool host's call protocol, the procedures, the
//! worker's ask and the delegate, each held to its manifest, and each executable answering a request as the
//! tool host hands it one.

#[path = "../support/binary.rs"]
mod binary;
#[path = "../support/scratch.rs"]
mod scratch;
#[path = "../support/standing.rs"]
mod standing;

mod call;
mod delegate;
mod manifests;
mod procedures;
mod worker_ask;
