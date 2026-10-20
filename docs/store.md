# The store

Agency keeps its rows in one PostgreSQL database, the store. The `store` crate opens it, binds and runs
statements on it, reads the model registry out of it, makes its tables and gives a fresh store its
seeds. The other crates and the server reach the store only through it.

## Its tables

- **Made by the store's migrations,** each table carried from the archive in the shape the archive holds
  it, columns, keys and indexes alike, and the clocks' two tables and the run ledger in agency's own,
  each made by a migration of its own after the first. A column of agency's own beside an archive shape
  is added by a migration of its own too: a worker's session record keeps the tokens its last answered
  turn carried and the share of the window they filled. The words said to a worker stand in the
  archive's shape, made by a migration of their own after the first.
- **The record of what ran is `agency_migrations`,** a table of the store's own, so the store may stand
  in a database beside another service's tables without either's record in the other's way.
- **A migration is not undone:** a store that must go back is dropped and made again.
- **The store names its own tables,** the ones its migration makes, so what reads every table of the
  store — a room forgotten among them — reads them and no other service's beside them in one database.
- **The tables, by what they keep:**
  - the agents and what they did: `agents`, `agent_log`, `agent_executions`, `execution_attempts`,
    `branch_agents`, `worker_sessions`, `session_resets`, and `worker_words`, the words said to a
    worker, each kept until a turn has taken it;
  - the base prompts: `base_prompts`, `agent_base_prompts`;
  - the accounts: `claude_accounts`;
  - the accounts' usage windows: `usage_windows`, one row a window of a slot — its `mode`, its
    `account` (empty on a provider that holds no logins of agency's) and its `name` together naming it,
    the fraction used, when it resets in epoch seconds or none, when it was seen, its length in minutes
    or none and its limit's name or none (`docs/usage-windows.md`);
  - the co-worker pool: the thirteen `coworker_pool_` tables, `limits`, `meta`, `native`,
    `native_reservations`, `native_stops`, `pings`, `place_writes`, `requests`, `reservation_meta`,
    `reservations`, `roles`, `tier_limits` and `tier_meta`;
  - the models and their roads: `model_catalogue`, `model_uses`, `registered_models`,
    `registered_model_addresses`, `model_fallbacks`, `fallback_routes`, `fallback_sessions`, and the
    seven `model_tier_` tables, `assignments`, `backfilled`, `conflicts`, `legacy`, `mappings`, `meta`
    and `seeded`;
  - the work nodes: `node_workplaces`, `node_rotation`, `node_assignments`; the nodes themselves, how
    each is reached among it, are the node service's and read from it (`docs/api.md`, 「The node
    service」);
  - the live Claude guard: `live_claude_openings`, `live_claude_runs`; an opening is kept under the run
    it was opened for, `run`, and carries its job and item only where a caller names them, since a
    job's rows are the job unit's (`docs/live-claude-guard.md`);
  - the moves of the cache prefix: `prefix_moves`;
  - the run records: `run_records`;
  - the work a harness hands over: `assignments`, one row a worker held for an item — the item, the
    holder, the worker, its state and when the hold runs out — and `jobs`, one row the item's run on that
    worker — the item and lease the harness named, the assignment, the worker, the session and run it
    opened, its state, outcome, report and tokens, and the prompt it was launched with; and what a
    session carries: `session_work`, its item and results; `session_loads`, the documents loaded into
    it; and `session_summaries`, an agent's kept summary (`docs/api.md`, 「Assignments and jobs」);
  - the run ledger: `runs`, one row a run of an agent's unit — its agent and room, its unit, how deep
    it runs, its bound, when it started and finished in epoch seconds, how it ended, who started it, the
    tokens its context held and the search it carried — opened and closed by the job unit
    (`docs/api.md`, 「Run log and run records」); a room forgotten takes its runs with it, and a
    companion's reset of its conversation takes the companion's and those of the rooms it answers alone
    (`docs/companions.md`, 「Starting it afresh」);
  - the clocks runs are held to and the asks past them: `run_clocks`, one row a clocked run — its name,
    the name it runs under, its work key and agent, what it is doing, when it started, its bound and when
    it ended — and `clock_asks`, one row an ask — the run, the facts it carried, when it was asked and
    when its answer is due, the verdict, until when a keep holds, who gave it, why and when it closed
    (`docs/agent-control.md`, 「A clock asks before it cuts」); a row ended a week ago goes as the next
    clock is opened;
  - the companions' sessions and settings: `room_sessions`, `room_session_turns`, `companion_settings`;
    the window a room holds open for an agent is the room's owner's, and agency keeps none;
  - the settings, below.
- **An account's `user_id`** names the user it belongs to, or none for the owner's; agency reads it and
  never fills it in itself, since who the users are is the caller's.
- **An agent's `prompts`** names the folder under `prompts/agents/` that holds what that agent alone is
  given, one plain name, or none for an agent given no folder of its own, as a companion added on the
  console starts. A companion's system prompt and its persona are read from this folder, never from the
  agent's name (`docs/harness-machine.md`, 「A companion's system prompt」). Whoever makes the row writes
  it, a companion's seed among them; a row written without it names none.

## The settings table

- **One row per key, a text value each,** for the values agency keeps that need no table of their own.
- **The families it holds,** a family being a key or a key with a suffix after its colon:
  - a run's cancellation, `execution_cancel:*`, and the provider order, `provider_order`, which the
    agent host's supervisor reads again before it starts a child (`docs/agent-control.md`, 「The host」);
  - an agent's sampling, `agent_sampling:*`, and its own gate switches, `agent_gate:*`, which agency keeps
    and a turn's consumer reads;
  - the Claude account chosen to run on and the one in use, `claude_account_chosen` and
    `claude_account_in_use`;
  - a session's record and its summary, `agent_session:*`, `agent_session_summary:*` and
    `agent_session_summary_thread:*`, and the sessions a worker set aside on other models,
    `agent_session_shelf:*`;
  - a companion's reset point, `reset_point:*` (`docs/companions.md`, 「What the reset cuts off」);
  - the machine watch's notes: `doctor_standing_failure`, `herdr_leftovers_told`,
    `dev_server_max_age_minutes`;
  - the logins watch's notes: `claude_node_token_gone:*` and its notice keys;
  - a job's road and its stop: `job_route_block:*`, `job_stop:*`;
  - the machine's room: its six limits, `machine_limit:<name>`, the launcher's notice of launches held,
    `machine_launch_held`, its `at` and `held` as JSON; and the job and item a worker's turn named while
    it goes, `worker_job:*` by the worker, as JSON (`docs/agent-control.md`, 「Room on the machine」);
  - the fallback and the rest: `fallback_since`, `fallback_provider`, `rest_until`, `rest_until:*`,
    `rest_kind`;
  - an account's unread mark, `usage_windows_unread:*` by the account, the moment its reader first found
    its token could not be read with (`docs/usage-windows.md`, 「The Claude account reader」);
  - the owner's warming switch, `keepalive_stopped`, standing while it holds `1`
    (`docs/keepalive-schedule.md`);
  - the data move's mark, `users_moved_at`, and the user who administers the platform,
    `users_administrator`, carried with the rows of a store moved across; agency reads them and never
    writes them, and a store without them reads as not moved and as naming no administrator.

## Its seeds

- **The values are the deployment's,** handed to the settling by whoever settles the store; the store
  names none of them and supplies only when each is written.
- **Agency's server settles the store as it starts** with the values agency's policy declares
  (`docs/policy.md`): the account `policy/seeds.toml` names, its credential the Claude Code login
  directory of the machine the server runs on — `CLAUDE_CONFIG_DIR`, else `.claude` under the home
  folder; the ladder `policy/tiers.toml` declares; the first seed's mapping of every tier on Claude Code,
  Codex and OpenRouter, and the work keys beyond the profiles with the tier each starts on, both in
  `policy/seeds.toml`; every profile's work key at the tier `policy/work-keys.toml` gives it; and the
  catalogue entries `policy/seeds.toml` declares, of which the repository's own declares none, since a
  model file is a deployment's. No provider is declared after the first seed: the registry's provider is
  given its mappings by the owner.
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

- **The store's address** is the one `BMO_ARCHIVE_URL` names, else the one the data root's
  `env/archive-url` holds; a process with neither names no store.
- **The service opens a pool,** which never hands out a connection opened before the store's shape last
  moved under the process: a statement met with PostgreSQL's code for a plan whose table moved is run
  once more on a fresh connection.
- **A tool or a host handed the store's address opens one read-only connection:** every transaction on
  it is read-only, and an opening the server refuses while it restarts is tried again with a doubling
  pause, for a minute or what `AGENCY_STORE_RECONNECT_MS` names.
- **Every number is bound at its widest,** a whole number as a 64-bit integer and a fraction as a
  double, so a statement prepared once takes every later value.
- **The TypeScript side's transactions write one at a time:** each takes one transaction-scoped
  advisory lock as it begins and holds it until it ends, whichever process of agency's opened it, so a
  transaction reads what the one before it wrote.
