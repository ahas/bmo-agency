//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

#[path = "../support/binary.rs"]
mod binary;
#[path = "../support/scratch.rs"]
mod scratch;
#[path = "../support/standing.rs"]
mod standing;

mod apart_session;
mod chat_runs;
mod chat_stand_in;
mod clock;
mod conversation_session;
mod command_reading {
  mod reading;
  mod records;
  mod shell;
}

mod companion {
  mod going;
  mod guard;
  mod reset;
  mod run;
}
mod config;
mod control {
  mod account_routes;
  mod agent_roster;
  mod agents;
  mod base_prompts;
  mod brain_reads;
  mod claude_account_changes;
  mod claude_accounts;
  mod companion_reset;
  mod companions;
  mod consumer_calls;
  mod conversation_routes;
  mod endpoint_accounts;
  mod events_publish;
  mod executions;
  mod fallback;
  mod fold_ratios;
  mod front;
  mod harness;
  mod job_log;
  mod mcp;
  mod model_tiers;
  mod models;
  mod own_llm;
  mod pool_board;
  mod pool_reads;
  mod pool_sessions;
  mod pool_sources;
  mod registry;
  mod resources;
  mod roads;
  mod room_session_reads;
  mod route_registration;
  mod run_launch;
  mod session_ops;
  mod sessions;
  mod stand_in_route;
  mod tiers;
  mod walk_reads;
  mod worker_runtime;
}
mod conversational;
mod data_move;
mod doctor;

mod door {
  mod apart;
  mod aside;
  mod confirm;
  mod endpoint;
  mod execution;
  mod guard;
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
    mod turn;
  }
  mod openrouter;
  mod openrouter_tap;
}
mod keepalive {
  mod logins;
  mod pool {
    mod branch;
    mod margin;
    mod schedule;
  }
  mod switch;
}
mod order_run {
  mod dispatch;
  mod places;
}
mod peers;

mod lines {
  mod chat;
  mod claude;
  mod codex;
  mod dsh;
  mod ordered;
  mod persona;
  mod place;
  mod prefix;
  mod runner;
  mod standing;
  mod token;
  mod trees;
  mod worker;
  mod worker_settings;
}

// The places built so far; the others stand beside these and are declared with their modules.
mod places {
  mod branch;
  mod claims;
  mod follow;
  mod ground;
  mod home;
  mod item_sweep;
  mod locks;
  mod nodes;
  mod servers;
  mod workbench;
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
mod relay;
mod rest;
mod rooms;
mod run_clock;
mod seeds;
mod session_output;
mod session_questions;
mod stand_in;
mod usage_balances;

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
  mod reload;
}

mod watches {
  mod links;
  mod machine;
  mod pressure;
}
mod wording;

mod worker_session {
  mod fold;
  mod held;
  mod records;
  mod told;
  mod warmth;
}
