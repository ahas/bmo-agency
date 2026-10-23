// The store's `fallback_routes` and `fallback_sessions` tables, read and written here and nowhere else on
// the TypeScript side (docs/cache-fallback.md).

import { seededDefaultRoute } from "./archive-seeding";
import type { AgentProviderName as AgentMode } from "./provider-names";
import { isFallbackRouteMode } from "./route-modes";
import type { FallbackRouteMode } from "./route-modes";
import type { ArchiveSql } from "./archive-sql";

// One work key's route: the road it runs on and the model chosen on every provider. The key is
// the chat agent's, the shared job-worker line or a gate — never a Claude model — so changing a
// launch model cannot silently move this row. Every model stays on the row when the road changes,
// so switching away and back restores the earlier choice. A null model means the profile's code
// default; a null OpenRouter field means there is no OpenRouter model for this key.
export interface FallbackRouteRow {
  key: string;
  routeMode: FallbackRouteMode;
  // The Claude account this key is pinned to, or null where its runs walk the account order. A
  // key pinned to one login runs on that login alone and is not carried to another.
  account: string | null;
  claudeModel: string | null;
  codexModel: string | null;
  codexEffort: string | null;
  openrouterModel: string | null;
  setAt: number;
}

// A resumable session owned by a non-primary mode. Scope is the concrete chat, worker or gate
// instance; the route key remains the independently configured piece of work. Keeping both is
// what prevents two rooms from sharing a conversation while still sharing one model choice.
export interface FallbackSessionRow {
  scope: string;
  key: string;
  mode: AgentMode;
  model: string;
  effort: string;
  sessionId: string;
  lastReplyAt: number;
  runJson: string;
}

// The upstream that took a scope's last OpenRouter turn on a model, when it answered and what run
// sends that turn's prefix again. Written only where that upstream keeps a cache with a known life.
interface FallbackRouteDbRow {
  key: string;
  routeMode: string;
  account: string | null;
  claudeModel: string | null;
  codexModel: string | null;
  codexEffort: string | null;
  openrouterModel: string | null;
  setAt: number;
}

interface FallbackSessionDbRow {
  scope: string;
  key: string;
  mode: AgentMode;
  model: string;
  effort: string;
  sessionId: string;
  lastReplyAt: number;
  runJson: string;
}

function fallbackRouteOf(row: FallbackRouteDbRow): FallbackRouteRow {
  return {
    key: row.key,
    routeMode: isFallbackRouteMode(row.routeMode) ? row.routeMode : "auto",
    account: row.account,
    claudeModel: row.claudeModel,
    codexModel: row.codexModel,
    codexEffort: row.codexEffort,
    openrouterModel: row.openrouterModel,
    setAt: row.setAt,
  };
}

// The explicit rows only. Codex defaults live with the fallback profiles in code and are
// merged at the edge that needs them, so an untouched choice follows a benchmark-default
// update while an owner's choice stays put.
const fallbackRoutesSql = `
  SELECT key, route_mode AS "routeMode", claude_model AS "claudeModel",
         codex_model AS "codexModel", codex_effort AS "codexEffort",
         openrouter_model AS "openrouterModel", set_at AS "setAt"
  FROM fallback_routes ORDER BY key
`;

export class FallbackStore {
  constructor(private readonly sql: ArchiveSql) {}

  // The explicit rows only (`readFallbackRoutes`).
  async fallbackRoutes(): Promise<FallbackRouteRow[]> {
    return (await this.sql.all(fallbackRoutesSql) as FallbackRouteDbRow[]).map(fallbackRouteOf);
  }

  async fallbackRoute(key: string): Promise<FallbackRouteRow | null> {
    const row = await this.sql.get(`
        SELECT key, route_mode AS "routeMode", account, claude_model AS "claudeModel",
               codex_model AS "codexModel", codex_effort AS "codexEffort",
               openrouter_model AS "openrouterModel", set_at AS "setAt"
        FROM fallback_routes WHERE key = ?
      `, key) as FallbackRouteDbRow | null;

    return row === null ? null : fallbackRouteOf(row);
  }

  // The model a key runs on one provider, where that provider takes its model from the row alone.
  async setClaudeRouteModel(key: string, model: string, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run(`
          INSERT INTO fallback_routes (key, route_mode, claude_model, set_at)
          VALUES (?, ?, ?, ?)
          ON CONFLICT(key) DO UPDATE SET claude_model = excluded.claude_model, set_at = excluded.set_at
        `, key, this.defaultRouteModeOf(key), model, at);
    });
  }

  async setCodexFallback(key: string, codexModel: string, codexEffort: string, at: number): Promise<void> {
    await this.sql.run(`
        INSERT INTO fallback_routes (key, codex_model, codex_effort, set_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
          codex_model = excluded.codex_model,
          codex_effort = excluded.codex_effort,
          set_at = excluded.set_at
      `, key, codexModel, codexEffort, at);
  }

  async resetCodexFallback(key: string, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE fallback_routes SET codex_model = NULL, codex_effort = NULL, set_at = ? WHERE key = ?", at, key);
      await this.deleteEmptyFallbackRoute(key);
    });
  }

  async setOpenRouterFallback(key: string, openrouterModel: string, at: number): Promise<void> {
    await this.sql.run(`
        INSERT INTO fallback_routes (key, openrouter_model, set_at)
        VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
          openrouter_model = excluded.openrouter_model,
          set_at = excluded.set_at
      `, key, openrouterModel, at);
  }

  async clearOpenRouterFallback(key: string, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE fallback_routes SET openrouter_model = NULL, set_at = ? WHERE key = ?", at, key);
      await this.deleteEmptyFallbackRoute(key);
    });
  }

  // The login a key's runs go out on: one account, or null to walk the order again.
  async setFallbackRouteAccount(key: string, account: string | null, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run(`
          INSERT INTO fallback_routes (key, set_at, route_mode, account)
          VALUES (?, ?, ?, ?)
          ON CONFLICT(key) DO UPDATE SET
            account = excluded.account,
            set_at = excluded.set_at
        `, key, at, this.defaultRouteModeOf(key), account);
      await this.deleteEmptyFallbackRoute(key);
    });
  }

  async setFallbackRouteMode(key: string, routeMode: FallbackRouteMode, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run(`
          INSERT INTO fallback_routes (key, set_at, route_mode)
          VALUES (?, ?, ?)
          ON CONFLICT(key) DO UPDATE SET
            route_mode = excluded.route_mode,
            set_at = excluded.set_at
        `, key, at, routeMode);
      await this.deleteEmptyFallbackRoute(key);
    });
  }

  // A row that says nothing the key's defaults do not is removed, so the key follows its profile
  // again. The mode compared is the key's own default: a gate that starts on the registry's provider and
  // was put on auto keeps a row saying so.
  private async deleteEmptyFallbackRoute(key: string): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run(`
          DELETE FROM fallback_routes
          WHERE key = ? AND codex_model IS NULL AND codex_effort IS NULL
            AND openrouter_model IS NULL AND claude_model IS NULL
            AND account IS NULL
            AND route_mode = ?
        `, key, this.defaultRouteModeOf(key));
    });
  }

  private defaultRouteModeOf(key: string): FallbackRouteMode {
    const mode = seededDefaultRoute(key);

    return isFallbackRouteMode(mode) ? mode : "auto";
  }

  async fallbackSession(scope: string, mode: AgentMode, model: string, effort: string): Promise<FallbackSessionRow | null> {
    return await this.sql.get(`
        SELECT scope, key, mode, model, effort, session_id AS "sessionId",
               last_reply_at AS "lastReplyAt", run_json AS "runJson"
        FROM fallback_sessions
        WHERE scope = ? AND mode = ? AND model = ? AND effort = ?
      `, scope, mode, model, effort) as FallbackSessionDbRow | null;
  }

  async fallbackSessions(mode?: AgentMode): Promise<FallbackSessionRow[]> {
    return (mode === undefined
      ? await this.sql.all(`
          SELECT scope, key, mode, model, effort, session_id AS "sessionId",
                 last_reply_at AS "lastReplyAt", run_json AS "runJson"
          FROM fallback_sessions ORDER BY last_reply_at
        `)
      : await this.sql.all(`
          SELECT scope, key, mode, model, effort, session_id AS "sessionId",
                 last_reply_at AS "lastReplyAt", run_json AS "runJson"
          FROM fallback_sessions WHERE mode = ? ORDER BY last_reply_at
        `, mode)) as FallbackSessionDbRow[];
  }

  async setFallbackSession(session: FallbackSessionRow): Promise<void> {
    await this.sql.run(`
        INSERT INTO fallback_sessions
          (scope, key, mode, model, effort, session_id, last_reply_at, run_json)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (scope, mode, model, effort) DO UPDATE SET key = excluded.key, session_id = excluded.session_id, last_reply_at = excluded.last_reply_at, run_json = excluded.run_json
      `, session.scope, session.key, session.mode, session.model, session.effort, session.sessionId, session.lastReplyAt, session.runJson);
  }

  async deleteFallbackSession(scope: string, mode: AgentMode, model: string, effort: string): Promise<void> {
    await this.sql.run("DELETE FROM fallback_sessions WHERE scope = ? AND mode = ? AND model = ? AND effort = ?", scope, mode, model, effort);
  }

}
