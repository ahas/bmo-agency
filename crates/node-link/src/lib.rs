//! What the link's two ends say to each other and the spec it is held to, shared with the process that
//! writes the spec (docs/work-nodes.md, 「The link」), so the writer and the reader are held to one shape;
//! and the two ends themselves, which the binary runs.

pub mod bridge;
pub mod connect;
pub mod frame;
pub mod hub;
pub mod rewrite;
pub mod serve;
pub mod spec;
