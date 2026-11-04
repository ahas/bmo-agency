//! Agency's tools (docs/tools.md): their side of the tool host's call protocol, the procedures, the
//! worker's ask, the delegate and the relay, each held to its manifest, and each executable answering a
//! request as the tool host hands it one.

mod call;
mod delegate;
mod manifests;
mod procedures;
mod relay;
mod worker_ask;
