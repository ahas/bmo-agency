import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { isTurnVariable } from "./agent-place";

import type { HarnessRun, ResidentSurface } from "./harness";

// What agency's own loop and the bot say to each other through the agent's place (docs/agent-control.md,
// 「The interactive surface a provider declares」): the record the loop keeps of the turns it ran, the
// inbox the bot puts turns into, the run a line typed at the loop goes out on, and which of a run's
// variables travel with one turn.

// One line of the record agency's resident loop keeps of the turns it ran: a turn starting, each step
// dsh took, its answer with the tokens dsh's own session log gave it, and how it ended — or that a
// person typed it. A step is also a line of Claude Code's record, by its `type` and `message`, so
// whoever follows the turn reads it as it reads that record.
export interface LoopRecord {
  kind: "start" | "step" | "answer" | "end" | "interrupted" | "failed";
  // The turn the line belongs to, by the id it was put in under, and on its start line what it is.
  id?: string;
  name?: string;
  title?: string;
  at?: number;
  text?: string;
  model?: string;
  usage?: Partial<DshTurnUsage>;
  typed?: boolean;
  type?: "assistant" | "user";
  message?: { id?: string; model?: string; content: Record<string, unknown>[]; usage?: Record<string, number> };
}

// The tokens dsh's session log gave a turn: the fresh input, the output, and the share of the input
// read from the cache, which stands apart from the input and is none where the upstream reports none.
export interface DshTurnUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
}

// The road one turn of the loop goes out on, opened before its dsh child starts and closed once the
// child has ended: the variables the child is handed on top of the loop's own, or why the turn cannot
// go out at all.
export type LoopRoad = { env: Record<string, string>; close: () => void } | { error: string };

// A surface whose program is agency's loop: what it declares as a resident surface, and the road each of
// its turns goes out on, which is the provider's own — a key the loop holds, an adapter it opens. The
// loop is handed its own environment and the turn's wait, where the turn names one.
export interface LoopSurface extends ResidentSurface {
  road(run: HarnessRun, loopEnv: Record<string, string | undefined>, timeoutMs: number | null): LoopRoad;
}

export function isLoopSurface(surface: ResidentSurface | null): surface is LoopSurface {
  return surface !== null && typeof (surface as Partial<LoopSurface>).road === "function";
}

// The variables that belong to one turn rather than to the loop: the gateway address a turn's tap
// stands at, and the run's own variables (`isTurnVariable`). They travel with the turn through the
// inbox, so the loop's own command — and so its identity — stays the same from one turn to the next.
export function isLoopTurnVariable(name: string): boolean {
  return name === "BMO_DSH_LOCAL_URL" || isTurnVariable(name);
}

// The record the loop keeps in its agent's place, and the inbox it takes its turns from.
export function loopRecordPath(place: string): string {
  return join(place, "loop.jsonl");
}

export function loopInboxDir(place: string): string {
  return join(place, "inbox");
}

// The run a line typed at the screen goes out on: the last one the bot put in, which is the agent's
// own, or the agent's own line laid down before any turn came. It is kept beside the inbox, without a
// secret in it: no prompt and no variable.
const loopRunPath = (place: string) => join(place, "loop-run.json");

// Written whole or not at all, since the loop reads what the bot writes while it is written.
export function writeWhole(path: string, value: unknown): void {
  const temporary = `${path}.${process.pid}.new`;

  writeFileSync(temporary, `${JSON.stringify(value)}\n`, { mode: 0o600 });
  renameSync(temporary, path);
}

export function writeLoopRun(place: string, run: HarnessRun): void {
  mkdirSync(place, { recursive: true, mode: 0o700 });
  writeWhole(loopRunPath(place), { ...run, prompt: "", env: {}, resident: null });
}

export function readLoopRun(place: string): HarnessRun | null {
  try {
    return JSON.parse(readFileSync(loopRunPath(place), "utf8")) as HarnessRun;
  } catch {
    return null;
  }
}
