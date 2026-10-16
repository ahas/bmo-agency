# The store

Agency keeps its rows in one PostgreSQL database, the store. The `store` crate opens it, binds and runs
statements on it, reads the model registry out of it, makes its tables and gives a fresh store its
seeds. The other crates and the server reach the store only through it.

## Its tables

- **Made by the store's migrations,** each table in the shape the archive it was carried from holds it,
  columns, keys and indexes alike.
- **The record of what ran is `agency_migrations`,** a table of the store's own, so the store may stand
  in a database beside another service's tables without either's record in the other's way.
- **A migration is not undone:** a store that must go back is dropped and made again.
- **The tables, by what they keep:**
  - the agents and what they did: `agents`, `agent_log`, `agent_executions`, `execution_attempts`,
    `branch_agents`, `worker_sessions`, `session_resets`;
  - the base prompts: `base_prompts`, `agent_base_prompts`, `base_prompt_changes`;
  - the accounts: `claude_accounts`;
  - the co-worker pool: the thirteen `coworker_pool_` tables, `limits`, `meta`, `native`,
    `native_reservations`, `native_stops`, `pings`, `place_writes`, `requests`, `reservation_meta`,
    `reservations`, `roles`, `tier_limits` and `tier_meta`;
  - the models and their roads: `model_catalogue`, `model_uses`, `registered_models`,
    `registered_model_addresses`, `model_fallbacks`, `fallback_routes`, `fallback_sessions`, and the
    seven `model_tier_` tables, `assignments`, `backfilled`, `conflicts`, `legacy`, `mappings`, `meta`
    and `seeded`;
  - the work nodes: `node_workplaces`, `node_rotation`, `node_assignments`, `node_hubs`;
  - the live Claude guard: `live_claude_openings`, `live_claude_runs`;
  - the moves of the cache prefix: `prefix_moves`;
  - the run records: `run_records`;
  - the companions' sessions and settings: `room_sessions`, `room_session_turns`, `room_windows`,
    `companion_settings`, `companion_setting_changes`;
  - the settings, below.
- **An account's `user_id`** names the user it belongs to, or none for the owner's; agency reads it and
  never fills it in itself, since who the users are is the caller's.

## The settings table

- **One row per key, a text value each,** for the values agency keeps that need no table of their own.
- **The families it holds,** a family being a key or a key with a suffix after its colon:
  - a run's cancellation, `execution_cancel:*`, and the provider order, `provider_order`, which the
    agent host's supervisor reads again before it starts a child (`docs/agent-control.md`, 「The host」);
  - an agent's sampling, `agent_sampling:*`;
  - a session's record and its summary, `agent_session:*`, `agent_session_summary:*` and
    `agent_session_summary_thread:*`;
  - the machine watch's notes: `doctor_standing_failure`, `herdr_leftovers_told`,
    `dev_server_max_age_minutes`;
  - the logins watch's notes: `claude_node_token_gone:*` and its notice keys;
  - a job's road and its stop: `job_route_block:*`, `job_stop:*`;
  - the fallback and the rest: `fallback_since`, `fallback_provider`, `rest_until`, `rest_until:*`,
    `rest_kind`.

## Its seeds

- **The values are the deployment's,** handed to the settling by whoever settles the store; the store
  names none of them and supplies only when each is written.
- **The settling** runs the migrations and then writes every seed in one transaction.
- **The account:** a store holding no account is given the one the seed names, the login the machine
  already has; a list the owner has written to is left alone.
- **The tiers:** the plan's mappings are written once, with the tier store's versions; every work key
  the plan declares is given the tier it starts on once, the first time the store is settled with it,
  and recorded as given; every provider the plan declares after the first seed gains a mapping in every
  tier once, carrying the effort and the answer ceiling of the provider it names where it names one.
  A mapping or an assignment the owner removed stays removed, and one that stands is never touched. A
  plan naming a tier its ladder does not hold is refused before anything is written.
- **The model catalogue:** its entries are written once, as its table is made (`docs/model-catalogue.md`,
  「Where it is kept」).

## Connecting

- **The service opens a pool,** which never hands out a connection opened before the store's shape last
  moved under the process: a statement met with PostgreSQL's code for a plan whose table moved is run
  once more on a fresh connection.
- **A tool or a host handed the store's address opens one read-only connection:** every transaction on
  it is read-only, and an opening the server refuses while it restarts is tried again with a doubling
  pause, for a minute or what `AGENCY_STORE_RECONNECT_MS` names.
- **Every number is bound at its widest,** a whole number as a 64-bit integer and a fraction as a
  double, so a statement prepared once takes every later value.
