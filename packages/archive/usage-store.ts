// The store's `prefix_moves` table, read and written here and nowhere else on the TypeScript side
// (docs/sessions.md). Usage itself is the usage reader's: agency publishes `usage-spent` and keeps no
// usage rows (docs/api.md, 「Events」).

import type { ArchiveSql } from "./archive-sql";

// Why a cached prefix moved. A document a session reads was written and the session was told to
// read it again; the code the bot runs went live; or the line a session is opened on — the tools
// it is shown, the model it runs as, the documents its own prompt is built out of — moved with a
// deployment. Nothing else moves a prefix from this side, and what the watch sees that none of
// these explains is the CLI's own doing rather than the bot's.
export type PrefixMoveWhy = "reload" | "deploy" | "launch";

export interface NewPrefixMove {
  agent: string;
  at: number;
  why: PrefixMoveWhy;
  detail?: string | null;
}

export interface PrefixMove extends NewPrefixMove {
  id: number;
  detail: string | null;
}

export class UsageStore {
  constructor(private readonly sql: ArchiveSql) {}

  // Puts one prefix move on the record. The agent is the bot's own name for whose prefix moved, or
  // the mark that stands for all of them — a deployment moves what every session stands on, and
  // naming them one by one would be a list that goes stale the moment a worker is added.
  async notePrefixMove(move: NewPrefixMove): Promise<void> {
    await this.sql.run("INSERT INTO prefix_moves (agent, at, why, detail) VALUES (?, ?, ?, ?)", move.agent, move.at, move.why, move.detail ?? null);
  }

  // What moved between two moments, oldest first. It is asked for a span rather than row by row
  // because a screen full of requests would otherwise be a query apiece.
  async prefixMovesBetween(from: number, to: number): Promise<PrefixMove[]> {
    return await this.sql.all("SELECT id, agent, at, why, detail FROM prefix_moves WHERE at >= ? AND at <= ? ORDER BY at ASC", from, to) as PrefixMove[];
  }
}
