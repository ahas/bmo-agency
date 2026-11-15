//! Agency's tools (docs/tools.md): their side of the tool host's call protocol, the procedures, the
//! worker's ask, the delegate, the relay and the DeepSeek top-up, each held to its manifest, and each executable answering a
//! request as the tool host hands it one.

mod call;
mod deepseek_top_up;
mod delegate;
mod manifests;
mod procedures;
mod relay;
mod worker_ask;
