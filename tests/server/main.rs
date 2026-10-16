//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

#[path = "../support/scratch.rs"]
mod scratch;

mod clock;

// The companion members built so far; the others stand beside these and are declared with their modules.
mod companion {
  mod going;
}
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

// The job run members built so far; the reset stands beside this and is declared with its module.
mod job_run {
  mod claude {
    mod failure;
  }
}
mod peers;

// The launch lines built so far; the others stand beside these and are declared with their modules.
mod lines {
  mod ordered;
  mod prefix;
}

// The places built so far; the others stand beside these and are declared with their modules.
mod places {
  mod branch;
  mod claims;
  mod follow;
  mod ground;
  mod home;
  mod locks;
  mod nodes;
  mod servers;
}
mod readiness;

mod watches {
  mod links;
}

// The worker session members built so far; the others stand beside these and are declared with their
// modules.
mod worker_session {
  mod summary;
  mod told;
  mod warmth;
}
