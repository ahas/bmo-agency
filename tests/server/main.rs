//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

#[path = "../support/scratch.rs"]
mod scratch;

mod clock;
mod config;
mod conversational;
mod data_move;

// The door's members built so far; the others stand beside these and are declared with their modules.
mod door {
  mod looks;
  mod record;
}
mod events;
mod ground;
mod hooks;
mod peers;

// The launch lines built so far; the others stand beside these and are declared with their modules.
mod lines {
  mod ordered;
  mod prefix;
}
