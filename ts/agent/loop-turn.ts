import { randomUUID } from "node:crypto";

import declaredTurn from "@agency/policy/resident-turn.toml";

import { processBirth } from "./agent-host";
import { agentPlace, beginPlaceRun, endPlaceRun, runVariables } from "./agent-place";
import { isLoopTurnVariable, loopRecordPath, type LoopRecord } from "./loop-record";
import { beginExecution, executionCancelled, executionRoot } from "./execution-identity";
import { RUN_NAME_VARIABLE } from "./herdr";
import { PaneTransportError } from "./pane-process";
import { providerSettingsArchive } from "./provider-order";
import { putLoopTurn, readLoopRecord } from "./loop-program";
import { interruptProgram, programAlive, readRunning, residentCommand, standResident, type ResidentProgram } from "./resident-pane";
import { publishEnvelope, type PaneTurnEnd } from "./worker-pane";

import type { HarnessRun, ResidentSurface, ResidentTurnRequest, ResidentTurnResult } from "./harness";

// One turn of agency's own loop standing in its agent's pane, from the bot's side (docs/agent-control.md,
// 「The interactive surface a provider declares」). The loop is stood up on its command, the turn is
// put into its inbox, and the loop's record is read until that turn has ended. The turn carries only
// its own variables; the key is the loop's, read from its environment.

// The loop comes up in a second and writes its record line by line, so it is given less time to stand
// and looked at more often than other programs; a turn cut short is given as long to write that it was
// as any resident turn is given to close.
const readyMs = declaredTurn.loop_ready_seconds * 1_000;
const pollMs: number = declaredTurn.loop_poll_ms;
const settleMs: number = declaredTurn.settle_ms;

// The lines of the record that belong to one turn: from its start to the next turn's start.
function linesOf(records: readonly LoopRecord[], id: string): LoopRecord[] | null {
  const start = records.findIndex((record) => record.kind === "start" && record.id === id);

  if (start === -1) return null;

  const next = records.findIndex((record, at) => at > start && record.kind === "start");

  return records.slice(start, next === -1 ? undefined : next);
}

export async function residentLoopTurn(options: ResidentTurnRequest & {
  // The surface this turn runs on, handed in by that surface's own `turn`, and the provider it
  // belongs to, which the turn's execution row names.
  surface: ResidentSurface;
  provider: string;
  pollMs?: number;
  readyMs?: number;
  settleMs?: number;
}): Promise<ResidentTurnResult> {
  const archive = providerSettingsArchive();

  if (!archive) throw new Error("execution archive unavailable");

  const surface = options.surface;
  const place = agentPlace(options.home.agent);
  const run: HarnessRun = { ...options.run, resident: { place, settingsPath: null } };
  const standing = residentCommand(await surface.command(run));
  const program: ResidentProgram = { identity: surface.identity({ ...standing, stdin: null }), command: standing, restart: null };
  const every = options.pollMs ?? pollMs;
  const execution = await beginExecution(archive, options.provider, run.line, {
    root: options.root ?? executionRoot(options.name), parent: options.parent ?? options.name,
    model: run.model, effort: null, tier: run.tier ?? null, panePath: true,
  });
  const owner = { pid: process.pid, birth: processBirth(process.pid) };
  const cancelled = () => executionCancelled(archive.settings(), [options.name, execution.id, execution.root, execution.parent]);
  const id = randomUUID();
  let ending: PaneTurnEnd | "launch-error" = "launch-error";

  publishEnvelope(execution.id, { owner });
  await archive.executions().saveExecution(execution);

  try {
    const stood = await standResident({ place, home: options.home, program, line: run.line, socket: options.socket, waitMs: options.readyMs ?? readyMs });

    if (!stood.ok) return { error: `the loop did not stand: ${stood.error.code}: ${stood.error.message}` };

    const pane = stood.result.paneId;
    const started = readRunning(place);
    const from = readLoopRecord(place).length;
    // Only the variables that are this turn's travel with it, so nothing of the bot's environment —
    // the key least of all — is ever written into the inbox.
    const env: Record<string, string> = { [RUN_NAME_VARIABLE]: options.name };

    for (const [name, value] of Object.entries(run.env)) {
      if (value !== undefined && isLoopTurnVariable(name) && name !== RUN_NAME_VARIABLE) env[name] = value;
    }

    execution.paneId = pane;
    execution.resident = { place, record: loopRecordPath(place), from, pid: started?.pid ?? null, birth: started?.birth ?? null };

    if (await cancelled()) {
      ending = "cancelled";

      return { error: "the turn was stopped before it went in" };
    }

    const promptedAt = Date.now();

    beginPlaceRun(place, options.name, options.title, promptedAt, runVariables(env));
    putLoopTurn(place, { id, name: options.name, title: options.title, timeoutMs: options.timeoutMs, run: { ...run, env, resident: null } });
    execution.state = "running";
    await archive.executions().saveExecution(execution);

    const deadline = options.timeoutMs === null ? null : promptedAt + options.timeoutMs;
    const cutShort = async (why: string, as: PaneTurnEnd): Promise<ResidentTurnResult> => {
      await interruptProgram(place, pane, surface.interrupt, { socket: options.socket });

      const settleBy = Date.now() + (options.settleMs ?? settleMs);

      while (Date.now() < settleBy) {
        const lines = linesOf(readLoopRecord(place).slice(from), id);

        if (lines === null || surface.readTurn({ lines }, run).state !== "working") break;

        await Bun.sleep(every);
      }

      ending = as;
      return { error: why };
    };

    for (;;) {
      if (await cancelled()) return await cutShort("the turn was stopped", "cancelled");
      if (deadline !== null && Date.now() >= deadline) return await cutShort("the turn ran past its time", "timeout");

      const alive = programAlive(place);

      if (alive === null || (started !== null && alive.pid !== started.pid)) throw new PaneTransportError("the loop went away during the turn");

      const lines = linesOf(readLoopRecord(place).slice(from), id);

      if (lines !== null) {
        const reading = surface.readTurn({ lines }, run);

        if (reading.state !== "working") {
          ending = reading.state === "interrupted" ? "cancelled" : "exited";

          if (reading.state === "interrupted") return { error: "the turn was interrupted" };
          if (reading.error !== null) return { error: reading.error };
          if (reading.reply.text === "") return { error: "the loop wrote no answer" };

          return { reply: reading.reply };
        }
      }

      await Bun.sleep(every);
    }
  } catch (error) {
    ending = ending === "launch-error" && execution.paneId !== null ? "cancelled" : ending;
    throw error;
  } finally {
    endPlaceRun(place);
    execution.state = ending;
    execution.endedAt = Date.now();
    await archive.executions().saveExecution(execution);
    publishEnvelope(execution.id, { owner, cause: ending });
  }
}
