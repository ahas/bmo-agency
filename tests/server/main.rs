//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

#[path = "../support/scratch.rs"]
mod scratch;

mod clock;
mod config;
mod data_move;
mod events;
mod ground;
mod hooks;
mod peers;
