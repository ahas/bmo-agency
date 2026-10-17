// The archive's `claude_accounts` table, read and written here and nowhere else in the archive
// (docs/store.md).

import type { ArchiveSql } from "./archive-sql";

// One Claude login agency holds. The row says where its credential is reached and nothing about
// what is in there; `ts/agent/claude-account.ts` is what says which ids, names and paths may be
// written into one.
export interface ClaudeAccountRow {
  id: string;
  name: string;
  place: number;
  enabled: boolean;
  credentialKind: "directory" | "token";
  credentialPath: string;
  addedAt: number;
  // When this login was found to be gone — nothing runs on it until a person signs in again.
  // Null while it is standing, which is every account's ordinary state.
  needsLoginAt: number | null;
  // The user whose account it is once the multi-user model is in force; null on the file
  // (docs/store.md).
  userId: number | null;
}

export interface NewClaudeAccountRow {
  id: string;
  name: string;
  enabled: boolean;
  credentialKind: "directory" | "token";
  credentialPath: string;
}

interface ClaudeAccountDbRow {
  id: string;
  name: string;
  place: number;
  enabled: number;
  credential_kind: "directory" | "token";
  credential_path: string;
  added_at: number;
  needs_login_at: number | null;
  user_id?: number | null;
}

export class ClaudeAccountStore {
  constructor(private readonly sql: ArchiveSql) {}

  // The Claude accounts in the order the owner put them, the off ones keeping their places. Where the
  // places cannot say which comes first — every one of them the same, as a half-written order
  // would leave them — the order they were registered in decides, so a list never reorders itself
  // on its own. That order is the table's own insertion order rather than the moment written into
  // a row: two accounts registered in one second carry the same moment and would otherwise be
  // read out in whichever order their names happen to sort in.
  async claudeAccounts(): Promise<ClaudeAccountRow[]> {
    const rows = await this.sql.all("SELECT * FROM claude_accounts ORDER BY place, rowid") as ClaudeAccountDbRow[];

    return rows.map((row) => this.claudeAccountOf(row));
  }

  /** The accounts in the order they were registered, which is the order with nothing else said. */
  async claudeAccountsAsAdded(): Promise<ClaudeAccountRow[]> {
    const rows = await this.sql.all("SELECT * FROM claude_accounts ORDER BY rowid") as ClaudeAccountDbRow[];

    return rows.map((row) => this.claudeAccountOf(row));
  }

  async getClaudeAccount(id: string): Promise<ClaudeAccountRow | null> {
    const row = await this.sql.get("SELECT * FROM claude_accounts WHERE id = ?", id) as ClaudeAccountDbRow | null;

    return row === null ? null : this.claudeAccountOf(row);
  }

  // An account joins the end of the order. Where it goes afterwards is the owner's to drag.
  async addClaudeAccount(account: NewClaudeAccountRow, now: number = Date.now()): Promise<void> {
    return await this.sql.wrote(async () => {
      const last = await this.sql.get("SELECT MAX(place) AS place FROM claude_accounts") as { place: number | null };

      await this.sql.run(`
          INSERT INTO claude_accounts (id, name, place, enabled, credential_kind, credential_path, added_at)
          VALUES (?, ?, ?, ?, ?, ?, ?)
        `, account.id, account.name, (last.place ?? 0) + 1, account.enabled ? 1 : 0, account.credentialKind, account.credentialPath, Math.floor(now / 1000));
    });
  }

  // What the owner changed about an account. Only what was named moves; its id and the moment it was
  // added are the record and are not the owner's to write.
  async updateClaudeAccount(
    id: string,
    changes: {
      name?: string;
      place?: number;
      enabled?: boolean;
      credentialKind?: "directory" | "token";
      credentialPath?: string;
    },
  ): Promise<boolean> {
    const sets: string[] = [];
    const args: (string | number)[] = [];

    if (changes.name !== undefined) {
      sets.push("name = ?");
      args.push(changes.name);
    }

    if (changes.place !== undefined) {
      sets.push("place = ?");
      args.push(changes.place);
    }

    if (changes.enabled !== undefined) {
      sets.push("enabled = ?");
      args.push(changes.enabled ? 1 : 0);
    }

    if (changes.credentialKind !== undefined) {
      sets.push("credential_kind = ?");
      args.push(changes.credentialKind);
    }

    if (changes.credentialPath !== undefined) {
      sets.push("credential_path = ?");
      args.push(changes.credentialPath);
    }

    if (sets.length === 0) {
      return false;
    }

    return (await this.sql.run(`UPDATE claude_accounts SET ${sets.join(", ")} WHERE id = ?`, ...args, id)).changes > 0;
  }

  async deleteClaudeAccount(id: string): Promise<boolean> {
    return (await this.sql.run("DELETE FROM claude_accounts WHERE id = ?", id)).changes > 0;
  }

  // The whole order in one write: every account's place and its on/off together, so the sequence
  // and the switches are never half moved and no reader can see one without the other.
  async setClaudeAccountOrder(order: readonly { id: string; place: number; enabled: boolean }[]): Promise<void> {
    await this.sql.transaction(async () => {
      for (const one of order) await this.sql.run("UPDATE claude_accounts SET place = ?, enabled = ? WHERE id = ?", one.place, one.enabled ? 1 : 0, one.id);
    });
  }

  private claudeAccountOf(row: ClaudeAccountDbRow): ClaudeAccountRow {
    return {
      id: row.id,
      name: row.name,
      place: row.place,
      enabled: row.enabled === 1,
      credentialKind: row.credential_kind,
      credentialPath: row.credential_path,
      addedAt: row.added_at,
      needsLoginAt: row.needs_login_at ?? null,
      userId: row.user_id === undefined || row.user_id === null ? null : Number(row.user_id),
    };
  }

  // A login found gone, or found standing again. Nothing else about the account moves: it keeps
  // its place, its name and its switch, and it is the walk that passes it over while this stands.
  async setClaudeAccountLogin(id: string, needsLoginAt: number | null): Promise<boolean> {
    return (await this.sql.run("UPDATE claude_accounts SET needs_login_at = ? WHERE id = ?", needsLoginAt, id)).changes > 0;
  }
}
