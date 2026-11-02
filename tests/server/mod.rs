//! The server's tests: its ground, each piece asked as a handler or a later part of the server asks it.

mod pin_model;

mod agent_roles;
mod apart_session;
mod cards;
mod chat_runs;
mod chat_stand_in;
mod clock;
mod conversation_session;
mod role_sessions;
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
  mod sessions;
}
mod config;
mod control {
  mod account_routes;
  mod agent_roles;
  mod base_prompts;
  mod brain_reads;
  mod chat_documents;
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
  mod front;
  mod gate_kinds;
  mod harness;
  mod item_filings;
  mod job_end;
  mod job_log;
  mod job_notes;
  mod job_reaper;
  mod job_roads;
  mod job_stops;
  mod jobs;
  mod mcp;
  mod models;
  mod own_llm;
  mod pane_death;
  mod registry;
  mod resources;
  mod room_session_reads;
  mod run_launch;
  mod session_ops;
  mod sessions;
  mod stand_in_route;
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
  mod liveness;
  mod looks;
  mod place_run;
  mod record;
  mod rollout;
  mod stand;
}
mod events;
mod ground;
mod handoff;
mod herdr_backend;
mod hooks;
mod host_run;

// The job run members built so far; the reset stands beside this and is declared with its module.
mod job_leases;
mod job_list;
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
}
mod order_run {
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
  mod gate_places;
  mod ground;
  mod handed_tokens;
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
  mod workers;
}
mod readiness;
mod rest;
mod road;
mod role_slots;
mod rooms;
mod run_clock;
mod run_list;
mod run_sweep;
mod seeds;
mod session_output;
mod session_questions;
mod session_tools;
mod translate;
mod turn_pickup;
mod usage_balances;

mod run_log {
  mod codex;
  mod json;
  mod lines;
  mod pane_calls;
  mod secrets;
  mod spans;
  mod writer;
}

mod verbs {
  mod claude_batch;
  mod reload;
}

mod watches {
  mod links;
  mod machine;
  mod pressure;
  mod pull_requests;
  mod queue_idle;
}
mod wording;

mod worker_session {
  mod fold;
  mod held;
  mod records;
  mod told;
  mod warmth;
}
