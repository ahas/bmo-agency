// A store of a test's own (docs/store.md): a database on the suite's PostgreSQL server, copied from a
// template this process makes once out of the same `store.sql` the store crate's migration runs and the
// account seed a deployment's settling writes, and dropped when the file or the test that asked for it is
// done. A template or copy a process that is gone left behind is dropped by the next process to make a
// template.
//
// A store is asked for at a file's top level, inside a test or in a `beforeEach`, never in a
// `beforeAll`: Bun runs an `afterAll` registered inside a `beforeAll` as soon as that hook ends, so the
// store would be dropped before the first test reads it.

import { afterAll } from "bun:test";
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

// The store's tables, as the store crate's one migration makes them.
const STORE_SQL = join(import.meta.dir, "..", "..", "..", "crates", "store", "src", "migration", "store.sql");

let made: Promise<string> | null = null;
let copies = 0;

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
    const name = `${TEMPLATE}${process.pid}`;

    await onServer(async (sql) => {
      const left = await sql.unsafe("SELECT datname::text AS name FROM pg_database WHERE datname LIKE $1 OR datname LIKE $2", [
        `${TEMPLATE.replaceAll("_", "\\_")}%`,
        `${COPY.replaceAll("_", "\\_")}%`,
      ]) as { name: string }[];

      for (const { name: old } of left) {
        const pid = old.replace(TEMPLATE, "").replace(COPY, "").split("_")[0] ?? "";

        if (!alive(pid)) await sql.unsafe(`DROP DATABASE IF EXISTS "${old}" WITH (FORCE)`);
      }

      await sql.unsafe(`CREATE DATABASE "${name}" TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'`);
    });

    const db = postgresClient(addressOf(name), { connections: 1 });

    try {
      await db.unsafe(readFileSync(STORE_SQL, "utf8")).simple();
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

// A database of the test's own made from `from`, dropped when the file that asked is done; its address.
async function copyOf(from: string): Promise<string> {
  copies += 1;

  const name = `${COPY}${process.pid}_${copies}`;

  await onServer((sql) => sql.unsafe(`CREATE DATABASE "${name}" TEMPLATE "${from}" ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'`));
  afterAll(() => onServer(async (sql) => {
    try {
      await sql.unsafe(`DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`);
    } catch (error) {
      const held = await sql.unsafe("SELECT usename::text AS role, application_name AS program, client_addr::text AS address, backend_start::text AS since FROM pg_stat_activity WHERE datname = $1", [name]);

      throw new Error(`${name} could not be dropped while ${JSON.stringify(held)} held it: ${String(error)}`);
    }
  }));

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
