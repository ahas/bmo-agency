// The archive's `worker_sessions` and `branch_agents` tables, read and written here and nowhere
// else in the archive (docs/archive-schema.md, 「The query stores」).

import { SEED_ACCOUNT_ID } from "./account-seed";
import type { AgentProviderName as AgentMode } from "./provider-names";
import type { ArchiveSql } from "./archive-sql";

// The one Claude session the job worker keeps for a model on one account, and when it last
// answered. The account is part of what identifies it: a session opened under one login is not
// resumable under another, so two accounts' sessions for one worker and model stand side by side
// and neither is ever offered in the other's place.
export interface WorkerSession {
  worker: string;
  model: string;
  account: string;
  claudeSessionId: string;
  lastReplyAt: number;
  // What the prefix this session was opened on reduced to when it last answered. A record
  // written before this was kept says nothing, and reads as unknown rather than as unchanged.
  prefixDigest: string | null;
  // The model that actually answered on this session where the safeguard refused the one it was
  // launched on and the worker keeps it anyway; null, or absent, where it answers on its own model.
  // The record stays under the model it was launched on, which is the key every later run asks for.
  answeredModel?: string | null;
}

// One standing checkout a co-work's side work borrows, as one provider and model saw it last.
// The row names the provider, the role that answered there and the moment it answered, which is
// what the owner asked the list to carry; the digest beside them is what the prompt cache is
// really keyed on, since the place's own path is inside the prefix a run opens on.
export interface BranchAgent {
  slot: number;
  mode: AgentMode;
  model: string;
  role: string;
  // The upstream that took the place's last turn, on a provider whose cache belongs to the upstream
  // rather than to the model — OpenRouter. Empty everywhere else.
  upstream: string;
  // Empty where the provider's prefix is not the one this bot can measure, which is every
  // provider but Claude Code: a digest of the wrong assembly would fire on changes that never
  // reached the run and stay silent on the ones that did.
  prefixDigest: string;
  lastReplyAt: number;
}

interface WorkerSessionRow {
  worker: string;
  model: string;
  account: string | null;
  claude_session_id: string;
  last_reply_at: number;
  prefix_digest: string | null;
  answered_model: string | null;
}

interface BranchAgentRow {
  slot: number;
  mode: AgentMode;
  model: string;
  role: string;
  upstream: string;
  prefix_digest: string;
  last_reply_at: number;
}

export class WorkerSessionStore {
  constructor(private readonly sql: ArchiveSql) {}

  // A session belongs to one worker on one model: two workers run in different checkouts, so
  // the CLI's own prompt differs and neither could resume the other's without paying for its
  // whole context again.
  async getWorkerSession(worker: string, model: string, account: string): Promise<WorkerSession | null> {
    const row = await this.sql.get("SELECT * FROM worker_sessions WHERE worker = ? AND model = ? AND account = ?", worker, model, account) as WorkerSessionRow | null;

    return row === null ? null : this.workerSessionOf(row);
  }

  // Every session a worker holds, oldest reply first, which is the order the keepalive wants
  // them in. A worker named is that worker's alone; none named is every worker's.
  async workerSessions(worker?: string): Promise<WorkerSession[]> {
    const rows = (
      worker === undefined
        ? await this.sql.all("SELECT * FROM worker_sessions ORDER BY last_reply_at")
        : await this.sql.all("SELECT * FROM worker_sessions WHERE worker = ? ORDER BY last_reply_at", worker)
    ) as WorkerSessionRow[];

    return rows.map((row) => this.workerSessionOf(row));
  }

  async setWorkerSession(session: WorkerSession): Promise<void> {
    await this.sql.run(`
        INSERT INTO worker_sessions (worker, model, account, claude_session_id, last_reply_at, prefix_digest, answered_model)
        VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (worker, model, account) DO UPDATE SET claude_session_id = excluded.claude_session_id, last_reply_at = excluded.last_reply_at, prefix_digest = excluded.prefix_digest, answered_model = excluded.answered_model
      `, session.worker, session.model, session.account, session.claudeSessionId, session.lastReplyAt, session.prefixDigest, session.answeredModel ?? null);
  }

  // One session goes, or — with no account named — every account's session for that worker and
  // model, which is what a prefix change or a handover means: the record is worthless whichever
  // login it was opened under.
  async deleteWorkerSession(worker: string, model: string, account?: string): Promise<void> {
    return await this.sql.wrote(async () => {
      if (account === undefined) {
        await this.sql.run("DELETE FROM worker_sessions WHERE worker = ? AND model = ?", worker, model);

        return;
      }

      await this.sql.run("DELETE FROM worker_sessions WHERE worker = ? AND model = ? AND account = ?", worker, model, account);
    });
  }

  // Every session of a worker that is going, which is what removing one leaves behind.
  async deleteWorkerSessions(worker: string): Promise<void> {
    await this.sql.run("DELETE FROM worker_sessions WHERE worker = ?", worker);
  }

  private workerSessionOf(row: WorkerSessionRow): WorkerSession {
    return {
      worker: row.worker,
      model: row.model,
      account: row.account ?? SEED_ACCOUNT_ID,
      claudeSessionId: row.claude_session_id,
      lastReplyAt: row.last_reply_at,
      prefixDigest: row.prefix_digest ?? null,
      answeredModel: row.answered_model ?? null,
    };
  }

  // One pool's list, oldest answer first, which is the order the side work reads it in: the place
  // that answered longest ago is the one whose warmth is closest to running out. A place's number
  // is only a place inside its own pool.
  async branchAgents(pool: string): Promise<BranchAgent[]> {
    const rows = await this.sql.all("SELECT slot, mode, model, role, upstream, prefix_digest, last_reply_at FROM branch_agents WHERE pool = ? ORDER BY last_reply_at", pool) as BranchAgentRow[];

    return rows.map((row) => ({
      slot: row.slot,
      mode: row.mode,
      model: row.model,
      role: row.role,
      upstream: row.upstream,
      prefixDigest: row.prefix_digest,
      lastReplyAt: row.last_reply_at,
    }));
  }

  async setBranchAgent(pool: string, agent: BranchAgent): Promise<void> {
    await this.sql.run(`
        INSERT INTO branch_agents (pool, slot, mode, model, role, upstream, prefix_digest, last_reply_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (pool, slot, mode, model, upstream) DO UPDATE SET role = excluded.role, prefix_digest = excluded.prefix_digest, last_reply_at = excluded.last_reply_at
      `, pool, agent.slot, agent.mode, agent.model, agent.role, agent.upstream, agent.prefixDigest, agent.lastReplyAt);
  }

  async deleteBranchAgent(pool: string, slot: number, mode: AgentMode, model: string, upstream = ""): Promise<void> {
    await this.sql.run("DELETE FROM branch_agents WHERE pool = ? AND slot = ? AND mode = ? AND model = ? AND upstream = ?", pool, slot, mode, model, upstream);
  }

  // Everything a place stood for, which is what removing the place itself leaves behind.
  async deleteBranchAgents(pool: string, slot: number): Promise<void> {
    await this.sql.run("DELETE FROM branch_agents WHERE pool = ? AND slot = ?", pool, slot);
  }

  // Every pool the list names. A pool whose places are already gone still has its lines here, so a
  // sweep that read only the directories standing on disk would leave them behind for ever.
  async branchAgentPools(): Promise<string[]> {
    const rows = await this.sql.all("SELECT DISTINCT pool FROM branch_agents ORDER BY pool") as { pool: string }[];

    return rows.map((row) => row.pool);
  }
}
