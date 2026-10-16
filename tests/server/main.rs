//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

#[path = "../support/scratch.rs"]
mod scratch;

mod clock;
mod command_reading {
  mod reading;
  mod records;
  mod shell;
}

// The companion members built so far; the others stand beside these and are declared with their modules.
mod companion {
  mod going;
  mod reset;
}
mod config;
mod control {
  mod agents;
  mod claude_account_changes;
  mod claude_accounts;
  mod companion_reset;
  mod companions;
  mod fallback;
  mod front;
  mod harness;
  mod model_tiers;
  mod models;
  mod own_llm;
  mod pool_board;
  mod registry;
  mod roads;
  mod sessions;
  mod tiers;
}
mod conversational;
mod data_move;
mod doctor;

mod door {
  mod apart;
  mod confirm;
  mod endpoint;
  mod execution;
  mod inbox;
  mod looks;
  mod place_run;
  mod record;
  mod rollout;
}
mod events;
mod ground;
mod herdr_backend;
mod hooks;
mod host_run;

// The job run members built so far; the reset stands beside this and is declared with its module.
mod job_run {
  mod claude {
    mod failure;
  }
  mod openrouter;
  mod openrouter_tap;
}
mod keepalive {
  mod logins;
  mod pool {
    mod branch;
    mod margin;
  }
}
mod order_run {
  mod dispatch;
  mod places;
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
mod queue {
  mod cancels;
  mod leftovers;
  mod paths;
  mod reconcile;
  mod routes;
  mod settings;
  mod worker_rules;
  mod workers;
}
mod readiness;
mod rest;
mod rooms;

mod run_log {
  mod codex;
  mod json;
  mod lines;
  mod secrets;
  mod spans;
  mod writer;
}

mod verbs {
  mod agents;
  mod claude_batch;
}

mod watches {
  mod links;
}
mod wording;

// The worker session members built so far; the others stand beside these and are declared with their
// modules.
mod worker_session {
  mod summary;
  mod told;
  mod warmth;
}
