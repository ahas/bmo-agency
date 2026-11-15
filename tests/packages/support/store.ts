// A store of a test's own (docs/store.md): a database on the suite's PostgreSQL server, copied from a
// template this process makes once out of the same `store.sql` and the same migrations after it the store
// crate runs, and the account seed a deployment's settling writes, and dropped when the file or the test
// that asked for it is done. A template or copy a process that is gone left behind is dropped by the next
// process to make a template.
//
// A store is asked for at a file's top level, inside a test or in a `beforeEach`, never in a
// `beforeAll`: Bun runs an `afterAll` registered inside a `beforeAll` as soon as that hook ends, so the
// store would be dropped before the first test reads it.

import { afterAll } from "bun:test";
import { randomUUID } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { seedClaudeAccount } from "@agency/archive/account-seed";
import { Archive } from "@agency/archive/archive";
import { databaseOn } from "@agency/archive/archive-place";
import { postgresClient } from "@agency/archive/postgres-client";

import type { PostgresClient } from "@agency/archive/postgres-client";

// What a template's and a copy's names begin with, ahead of the process that made them.
const TEMPLATE = "agency_ts_tmpl_";
const COPY = "agency_ts_copy_";

// The store's tables, as the store crate's first migration makes them.
const MIGRATION_DIR = join(import.meta.dir, "..", "..", "..", "crates", "store", "src", "migration");
const STORE_SQL = join(MIGRATION_DIR, "store.sql");

// The migrations after it, every file each runs, in the order `crates/store/src/migration.rs` runs them:
// what a store this process makes has to be the store the server's own migration leaves, or a column the
// TypeScript side reads would be missing only in its tests. What a migration runs between its files carries
// rows, which a new store holds none of.
const LATER_MIGRATIONS: readonly string[] = [
  "clocks.sql",
  "runs.sql",
  "worker_context.sql",
  "worker_words.sql",
  "usage_windows.sql",
  "work.sql",
  "job_model.sql",
  "base_prompt_seeds.sql",
  "endpoint_accounts.sql",
  "pool_session.sql",
  "deepseek_provider.sql",
  "mapping_thinking.sql",
  "session_summaries_retired.sql",
  "fold_ratios.sql",
  "agent_owner.sql",
  "recollectors_retired.sql",
  "usage_balances.sql",
  "run_rounds.sql",
  "route_registration.sql",
  "run_usage.sql",
  "job_usage.sql",
  "prefix_documents.sql",
  "job_instruction.sql",
  "run_tools.sql",
  "gate_kinds.sql",
  "job_roads.sql",
  "worker_skill_names.sql",
  "translator_runner.sql",
  "pickers_retired.sql",
  "moments_in_milliseconds.sql",
  "picker_keys_retired.sql",
  "role_files.sql",
  "agent_log_prune.sql",
  "chat_documents.sql",
  "registration_last_answered.sql",
  "agent_roles.sql",
  "agent_log_run_id.sql",
  "card_services.sql",
  "pool_asks.sql",
  "registration_wait_retired.sql",
  "clock_asks_unbounded.sql",
  "assignments_unbounded.sql",
  "role_sessions.sql",
  "delegation_titles.sql",
  "job_close_lease.sql",
  "turn_liveness.sql",
  "stream_events.sql",
  "renewal_tables.sql",
  "renewal_retired.sql",
  "delegator_follows_supervisor.sql",
  "role_names.sql",
  "supervisor_settings.sql",
  "supervisors_alike.sql",
  "chosen_roads.sql",
  "supervisor_card_rank.sql",
  "relays.sql",
  "supervisor_system_hash.sql",
  "role_agents.sql",
  "role_agent_settings.sql",
  "role_skills.sql",
  "agent_wanted.sql",
  "arbiter_model.sql",
  "arbiter_model_gone.sql",
  "account_seats.sql",
  "job_review.sql",
  "person_nodes.sql",
  "coworkers_gone.sql",
  "agent_order_gone.sql",
  "job_lines_by_agent.sql",
  "job_seats_unbound.sql",
  "job_machine_turn.sql",
  "notices.sql",
  "job_turn_words.sql",
  "registration_languages.sql",
];

let made: Promise<string> | null = null;
let copies = 0;

// The run this file's stores belong to. A test file run beside another in one process has the same
// process id, so the pid alone would name one template twice and the second creation would fail.
const run = randomUUID().slice(0, 8);

/** The suite's PostgreSQL server, as the run names it, and the machine's own socket where nothing does. */
export function testServer(): string {
  return process.env.BMO_TEST_POSTGRES ?? "postgres:///bmo_test?host=/var/run/postgresql";
}

/** The test server's address with another database in place of its own. */
export function addressOf(database: string): string {
  return databaseOn(testServer(), database);
}

// One step on the server's own database, its connection held for as long as the step takes.
async function onServer<T>(work: (sql: PostgresClient) => Promise<T>): Promise<T> {
  const sql = postgresClient(addressOf("postgres"), { connections: 1 });

  try {
    return await work(sql);
  } finally {
    await sql.close();
  }
}

function alive(pid: string): boolean {
  return pid !== "" && existsSync(join("/proc", pid));
}

// The template this process's copies are made from, migrated the first time a test asks.
function template(): Promise<string> {
  made ??= (async () => {
    const name = `${TEMPLATE}${process.pid}_${run}`;

    await onServer(async (sql) => {
      const left = await sql.unsafe("SELECT datname::text AS name FROM pg_database WHERE datname LIKE $1 OR datname LIKE $2", [
        `${TEMPLATE.replaceAll("_", "\\_")}%`,
        `${COPY.replaceAll("_", "\\_")}%`,
      ]) as { name: string }[];

      for (const { name: old } of left) {
        const pid = old.replace(TEMPLATE, "").replace(COPY, "").split("_")[0] ?? "";

        if (!alive(pid)) await dropped(sql, old);
      }

      await sql.unsafe(`CREATE DATABASE "${name}" TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'`);
    });

    const db = postgresClient(addressOf(name), { connections: 1 });

    try {
      await db.unsafe(readFileSync(STORE_SQL, "utf8")).simple();

      for (const migration of LATER_MIGRATIONS) {
        await db.unsafe(readFileSync(join(MIGRATION_DIR, migration), "utf8")).simple();
      }
    } finally {
      await db.close();
    }

    // The account a deployment's settling gives a store holding none: the login the machine already
    // has, which is the test home's (docs/store.md, 「Its seeds」).
    const seeded = new Archive(addressOf(name));
    const account = seedClaudeAccount();

    try {
      await seeded.claudeAccounts().addClaudeAccount({ id: account.id, name: account.name, enabled: true, credentialKind: account.credential.kind, credentialPath: account.credential.path });
    } finally {
      await seeded.close();
    }

    return name;
  })();

  return made;
}

// How long a database's drop waits for its holders to leave, and how often it tries again.
const dropWithinMs = 15_000;
const dropEveryMs = 250;

// The server's SQLSTATE for a database other sessions still stand on.
const IN_USE = "55006";

function inUse(error: unknown): boolean {
  return (error as { errno?: unknown } | null)?.errno === IN_USE;
}

// A test database dropped. The test role ends only its own sessions on it. A forced drop would try to end
// every holder, and the server refuses the whole drop, before ending any, where one holder is a worker of
// its own — an autovacuum worker visiting the database, say — since a role without the right to signal
// every backend may not end it. A plain drop asks the server's own workers there to stop and waits a few
// seconds for every holder to leave; where one stays past that, or a session of the role came back in
// between, it fails as in use, and the drop tries again within its bound and then says who held it.
async function dropped(sql: PostgresClient, name: string): Promise<void> {
  const until = Date.now() + dropWithinMs;

  for (;;) {
    try {
      await sql.unsafe("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND usename = current_user", [name]);
      await sql.unsafe(`DROP DATABASE IF EXISTS "${name}"`);
      return;
    } catch (error) {
      if (!inUse(error) || Date.now() >= until) {
        const held = await sql.unsafe("SELECT usename::text AS role, backend_type AS kind FROM pg_stat_activity WHERE datname = $1", [name]) as { role: string | null; kind: string }[];

        throw new Error(`${name} could not be dropped while ${JSON.stringify(held)} held it: ${String(error)}`);
      }

      await Bun.sleep(dropEveryMs);
    }
  }
}

// A database of the test's own made from `from`, dropped when the file that asked is done; its address.
async function copyOf(from: string): Promise<string> {
  copies += 1;

  const name = `${COPY}${process.pid}_${run}_${copies}`;

  await onServer((sql) => sql.unsafe(`CREATE DATABASE "${name}" TEMPLATE "${from}" ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'`));
  afterAll(() => onServer((sql) => dropped(sql, name)));

  return addressOf(name);
}

/** A new copy of the migrated, empty store; its address. It is dropped when the file that asked is done. */
export async function scratchDatabase(): Promise<string> {
  return await copyOf(await template());
}

/**
 * A database with no table in it, for agency's own server to migrate as it stands on it, the way it
 * stands on a deployment's new store; its address. It is dropped when the file that asked is done.
 */
export async function emptyDatabase(): Promise<string> {
  return await copyOf("template0");
}

/** A migrated, empty store of the test's own, opened; closed and dropped when the file is done. */
export async function scratchArchive(): Promise<Archive> {
  const archive = new Archive(await scratchDatabase());

  afterAll(() => archive.close());

  return archive;
}
