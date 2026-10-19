//! Agency's keeping of the accounts' usage windows and their readers (docs/usage-windows.md), and its
//! cache-lifetime judgement (docs/sessions.md, 「A worker's session per model」).
//!
//! Derived from cache-siren by the-original-author (https://github.com/the-original-author/cache-siren), carried
//! forward through bmo-cache-siren; the repository's NOTICE gives the credit in full.
//!
//! The keeping and the judgement know no provider: each reader is one provider's member and declares its
//! own provider, cadence and freshness, and a provider member hands in the lifetime it declares.

pub mod claude;
pub mod codex;
pub mod lifetime;
pub mod reader;
pub mod windows;

pub use reader::Reader;
pub use windows::{Keeping, KeptWindow, Reading, Slot, Window};
