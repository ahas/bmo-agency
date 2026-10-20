import "@agency/test-support/setup";

import { appendFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, setDefaultTimeout, test } from "bun:test";

import { identityAlive } from "@agency/agent/agent-host";
import { agentPlace, readRun } from "@agency/agent/agent-place";
import { endpointPlaceSettings } from "@agency/agent/claude-code";
import { endpointTurn } from "@agency/agent/claude-endpoint-turn";
import { cliProjectsRoot } from "@agency/agent/cli-transcripts";
import { codexResident } from "@agency/agent/codex";
import { residentCodexTurn } from "@agency/agent/codex-resident";
import { openRouterProvider } from "@agency/agent/dsh";
import { executionCancelKey } from "@agency/agent/execution-identity";
import { residentLoopTurn } from "@agency/agent/loop-turn";
import { ExecutionStopped } from "@agency/agent/pane-process";
import { installProviderSettings } from "@agency/agent/provider-order";
import { takeLoopTurn } from "@agency/agent/loop-program";
import { readProgram, readRunning, residentCommand } from "@agency/agent/resident-pane";
import { headCommand } from "@agency/agent/units";
import { Archive } from "@agency/archive/archive";
import { crateBinary } from "@agency/test-support/cargo";
import { codeRoot } from "@agency/support/roots";
import { scratchDir } from "@agency/test-support/scratch";
import { toolHostStandIn } from "@agency/test-support/tool-host-stand-in";
import { answerRecord, standInHerdr } from "@agency/test-support/herdr-standin";

import type { ClaudeEndpoint } from "@agency/agent/claude-code";
import type { HarnessCommand, HarnessRun, ResidentSurface, ResidentTurnRequest } from "@agency/agent/harness";
import type { AgentHome } from "@agency/agent/herdr-run";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

// One turn of a surface that carries its own turns — Codex's program read out of its rollout, agency's
// loop over its inbox and its record, and the registry's Claude Code on the gateway — by the TypeScript
// surface turn kept as the parity reference and by agency's Rust door (`bmo-agency resident-turn`) for the same provider (docs/agent-control.md, 「The interactive
// surface a provider declares」). Each scenario stages the
// same program on a stand-in herdr of its own for each side, in a place and a checkout of that side's
// own, and what each side came to — the ending, the run the place names, the program it stood, the
// execution row, what it put into the pane or the inbox and how often the program was started — must be
// the same.

// A program written into a place is handed its session token first, by the tool host stood in here.
const toolHost = toolHostStandIn(scratchDir("bmo-tool-host-"));

afterAll(() => toolHost.stop());

const binary = crateBinary(codeRoot, "bmo-agency", "agency-server");
const root = scratchDir("bmo-surface-twin-");
const realDataDir = process.env.BMO_DATA_DIR;
// The user's own Codex home, whose login and threads the program's home shares: the bun side is handed
// it, and the Rust side finds it under the home its environment names.
const userHome = join(root, "home");
const sessions = join(userHome, ".codex", "sessions", "2026", "09", "21");
// The registration's own configuration home, where its program's records go.
const registered = join(root, "registered-home");

process.env.BMO_DATA_DIR = root;
mkdirSync(sessions, { recursive: true });

afterAll(() => {
  if (realDataDir === undefined) delete process.env.BMO_DATA_DIR;
  else process.env.BMO_DATA_DIR = realDataDir;

  rmSync(root, { recursive: true, force: true });
});

type Side = "bun" | "rust";
type HerdrOptions = Parameters<typeof standInHerdr>[1];

// What a side came to, in one shape on both: an answer or the reason there is none, a turn stopped from
// outside, or what threw the turn out.
type Came = { reply: unknown } | { error: string } | { stopped: string } | { thrown: string };

interface Ask {
  name: string;
  run: HarnessRun;
  timeoutMs?: number | null;
}

interface Surface {
  provider: string;
  // The TypeScript side's turn of the surface.
  bun: (request: ResidentTurnRequest) => Promise<Came>;
  // The command its program stands on for this run, as the provider composes it.
  command: (run: HarnessRun, place: string) => Promise<HarnessCommand>;
  clocks: Record<string, number>;
}

const home = (agent: string): AgentHome => ({ agent, label: agent, head: { command: headCommand(agent, codeRoot), cwd: root, env: {} } });

async function bunTurn(surface: Surface, agent: string, socket: string, ask: Ask): Promise<Came> {
  try {
    return await surface.bun({ run: ask.run, home: home(agent), name: ask.name, title: "a talk", timeoutMs: ask.timeoutMs ?? 10_000, socket });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);

    return error instanceof ExecutionStopped ? { stopped: message } : { thrown: message };
  }
}

async function rustTurn(surface: Surface, agent: string, configHome: string, ask: Ask, address: string): Promise<Came> {
  const request = join(root, `${agent}-${ask.name.replaceAll(":", "-")}.json`);
  const command = residentCommand(await surface.command(ask.run, agentPlace(agent)));

  writeFileSync(request, JSON.stringify({
    agent, provider: surface.provider, name: ask.name, title: "a talk", timeoutMs: ask.timeoutMs ?? 10_000,
    run: ask.run, command, ...surface.clocks,
  }));

  const run = Bun.spawn([binary, "resident-turn", request, "--data-root", root], {
    cwd: codeRoot,
    env: { ...process.env, XDG_CONFIG_HOME: configHome, HOME: userHome, BMO_ARCHIVE_URL: address },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, err, code] = await Promise.all([new Response(run.stdout).text(), new Response(run.stderr).text(), run.exited]);

  expect({ code, err: code === 0 ? "" : err }).toEqual({ code: 0, err: "" });

  const said = JSON.parse(out) as { ending?: Came; thrown?: string };

  return said.ending ?? { thrown: said.thrown ?? "" };
}

// A program a turn ended on its way out, stood again by the stand-in's loop, so what the place holds is
// read once it stands.
async function standing(place: string): Promise<void> {
  for (let look = 0; look < 200; look += 1) {
    const running = readRunning(place);

    if (running === null || identityAlive({ pid: running.pid, birth: running.birth ?? "" })) return;

    await Bun.sleep(10);
  }
}

// What a side came to, with what is that side's own — its agent, its checkout, the clock, the ids a row,
// a thread or a session is given — named the same way on both.
function normalised(value: unknown, agent: string, cwd: string, own: Record<string, string>): unknown {
  let text = JSON.stringify(value)
    .replaceAll(cwd, "<cwd>")
    .replaceAll(agentPlace(agent), "<place>")
    .replaceAll(cwd.replace(/[^a-zA-Z0-9]/g, "-"), "<project>")
    .replace(/ · \d\d:\d\d\]/g, " · <clock>]")
    .replace(/bmo-root-[0-9a-f-]{36}/g, "<root>")
    // The token a program is handed is the tool host's to choose, issued afresh for each side.
    .replace(/"token-\d+"/g, "\"<token>\"")
    // A session a turn opens for itself is named afresh on each side.
    .replace(/[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/g, "<session>");

  for (const [mine, named] of Object.entries(own)) text = text.replaceAll(mine, named);

  return JSON.parse(text) as unknown;
}

interface Staged {
  archive: Archive;
  agent: string;
  cwd: string;
  side: Side;
}

async function side(
  side: Side,
  base: string,
  surface: Surface,
  herdrOptions: (staged: Staged) => HerdrOptions,
  asks: (staged: Staged) => Ask[],
  extra: { own?: (staged: Staged) => Record<string, string>; beside?: (staged: Staged) => () => unknown } = {},
): Promise<unknown> {
  const archive = await scratchArchive();
  const agent = `${base}-${side}`;
  const cwd = join(root, side, base);
  const staged = { archive, agent, cwd, side };
  const herdr = standInHerdr(`${base}-${side}`, herdrOptions(staged));
  const place = agentPlace(agent);

  mkdirSync(cwd, { recursive: true });
  installProviderSettings(archive);

  // Whatever runs beside the program for the scenario — a loop taking its inbox — and what it saw.
  const beside = extra.beside?.(staged) ?? (() => null);

  try {
    const turns: unknown[] = [];

    for (const ask of asks(staged)) {
      const came = side === "bun" ? await bunTurn(surface, agent, herdr.socket, ask) : await rustTurn(surface, agent, herdr.configHome, ask, archive.path);
      await standing(place);

      const row = (await archive.executions().executions()).find((one) => one.parent === ask.name) ?? null;
      const program = readProgram(place);
      const placeRun = readRun(place);

      turns.push({
        came,
        run: placeRun === null ? null : { name: placeRun.name, title: placeRun.title, person: placeRun.person, env: placeRun.env, ended: placeRun.endedAt !== null },
        program: program === null ? null : { command: program.command, restart: program.restart, opened: program.opened },
        row: row === null ? null : {
          root: row.root, parent: row.parent, workKey: row.workKey, model: row.model, effort: row.effort, tier: row.tier, sessionId: row.sessionId,
          provider: row.provider, line: row.line, state: row.state, panePath: row.panePath, attempt: row.attempt, exitCode: row.exitCode,
          ended: row.endedAt !== null, resident: row.resident === undefined ? null : { place: row.resident.place, record: row.resident.record, from: row.resident.from },
        },
        starts: readRunning(place)?.starts ?? null,
      });
    }

    const put = herdr.asked
      .filter((one) => ["pane.send_text", "agent.prompt", "pane.send_keys", "pane.close"].includes(one.method))
      .map((one) => ({ method: one.method, params: one.params }));

    return normalised({ turns, put, beside: beside() }, agent, cwd, extra.own?.(staged) ?? {});
  } finally {
    installProviderSettings(null);
    herdr.stop();
  }
}

async function twin(base: string, surface: Surface, herdrOptions: (staged: Staged) => HerdrOptions, asks: (staged: Staged) => Ask[], extra: Parameters<typeof side>[5] = {}) {
  const bun = await side("bun", base, surface, herdrOptions, asks, extra);
  const rust = await side("rust", base, surface, herdrOptions, asks, extra);

  expect(rust).toEqual(bun);

  return bun as { turns: { came: Came; starts: number | null; row: { state: string } | null }[]; put: { method: string; params: Record<string, unknown> }[]; beside: unknown };
}

function harnessRun(overrides: Partial<HarnessRun>): HarnessRun {
  return {
    line: "chat", model: "", effort: "", systemPromptPath: null, toolsShown: "", toolsAllowed: null, toolsDenied: null,
    mcpConfigPath: null, prompt: "hello", promptDelivery: "input", resumeSessionId: null, keepSession: true, hostCommands: false,
    thinking: true, answerShape: "text", cwd: root, env: { PATH: "/usr/bin:/bin" }, ...overrides,
  };
}

// A twin starts `bmo-agency` and stands a store of its own on each side, several times in one
// scenario: a second or two alone, many times that on a machine carrying other runs.
setDefaultTimeout(60_000);

describe("a turn of Codex's program, by either side", () => {
  const codex: Surface = {
    provider: "codex",
    bun: (request) => residentCodexTurn({ ...request, surface: codexResident, pollMs: 5, readyMs: 5_000, userHome: join(userHome, ".codex") }),
    command: (run) => codexResident.command(run),
    clocks: { pollMs: 5, readyMs: 5_000 },
  };
  // Each side's own thread, in one directory of threads both read.
  const thread = (staged: Staged) => `019a1b2c-${staged.agent}`;
  const rollout = (staged: Staged) => join(sessions, `rollout-2026-09-21T12-03-04-${thread(staged)}.jsonl`);
  const own = (staged: Staged) => ({ [thread(staged)]: "<thread>" });
  const run = (staged: Staged, overrides: Partial<HarnessRun> = {}) =>
    harnessRun({ model: "gpt-5.6-luna", effort: "high", cwd: staged.cwd, env: { PATH: "/usr/bin:/bin", BMO_TURN_CHAT: "-1" }, ...overrides });
  // The lines a turn writes to its thread's rollout, in the shape a recorded rollout carries them.
  const turnLines = (text: string): unknown[] => [
    { type: "turn_context", payload: { model: "gpt-5.6-luna" } },
    { type: "event_msg", payload: { type: "task_started" } },
    { type: "event_msg", payload: { type: "agent_message", message: text } },
    {
      type: "event_msg",
      payload: {
        type: "token_count",
        info: { last_token_usage: { input_tokens: 1000, cached_input_tokens: 900, cache_write_input_tokens: 0, output_tokens: 12 } },
        rate_limits: { limit_id: "codex", primary: { used_percent: 12, window_minutes: 300, resets_at: 1790000000 } },
      },
    },
    { type: "event_msg", payload: { type: "task_complete", last_agent_message: text } },
  ];

  test("opens its thread, is relabelled onto it, and the next turn resumes it in the same program", async () => {
    const seen = await twin(
      "codex-two",
      codex,
      (staged) => {
        let told = 0;

        return {
          record: () => rollout(staged),
          ending: [],
          answer: async ({ text }) => {
            told += 1;

            const opening = told === 1 ? [{ type: "session_meta", payload: { id: thread(staged), cwd: staged.cwd } }] : [];

            return [...opening, ...turnLines(text.endsWith("next") ? "the second answer" : "the first answer")];
          },
        };
      },
      (staged) => [
        { name: "bmo:codex:1", run: run(staged) },
        { name: "bmo:codex:2", run: run(staged, { prompt: "next", resumeSessionId: thread(staged), env: { PATH: "/usr/bin:/bin", BMO_TURN_CHAT: "42" } }) },
      ],
      { own },
    );

    expect(seen.turns.map((one) => ("reply" in one.came ? (one.came.reply as { text: string }).text : one.came))).toEqual(["the first answer", "the second answer"]);
    expect(seen.turns.map((one) => one.starts)).toEqual([1, 1]);
  }, 60_000);

  test("a program showing the folder trust question is left for a person, and a turn ended on the program's own error says why", async () => {
    const trust = await twin(
      "codex-trust",
      codex,
      () => ({ record: () => null, ending: [], answer: async () => null, screen: "  Do you trust the contents of this\n  directory? Working with untrusted contents\n\n› 1. Yes, continue" }),
      (staged) => [{ name: "bmo:codex:trust", run: run(staged) }],
    );
    const failed = await twin(
      "codex-failed",
      codex,
      (staged) => {
        writeFileSync(rollout(staged), `${JSON.stringify({ type: "session_meta", payload: { id: thread(staged), cwd: staged.cwd } })}\n`);

        return {
          record: () => rollout(staged),
          ending: [],
          answer: async () => [
            { type: "event_msg", payload: { type: "task_started" } },
            { type: "event_msg", payload: { type: "task_complete", error: { codex_error_info: "usage_limit_exceeded", message: "You've hit your usage limit" } } },
          ],
        };
      },
      (staged) => [{ name: "bmo:codex:failed", run: run(staged, { resumeSessionId: thread(staged) }) }],
      { own },
    );

    expect((trust.turns[0]?.came as { error: string }).error).toStartWith("codex's program is waiting on a person");
    expect(trust.turns[0]?.row?.state).toBe("cancelled");
    expect(failed.turns[0]?.came).toEqual({ error: "usage_limit_exceeded: You've hit your usage limit" });
  }, 60_000);

  test("told to stop or past its bound, is cut short with the surface's interrupt", async () => {
    const working = (staged: Staged, stop: boolean): HerdrOptions => ({
      record: () => rollout(staged),
      ending: [],
      after: ["working"],
      answer: async () => {
        if (stop) await staged.archive.settings().setSetting(executionCancelKey("bmo:codex:stop"), "1");

        return [{ type: "session_meta", payload: { id: thread(staged), cwd: staged.cwd } }, { type: "event_msg", payload: { type: "task_started" } }];
      },
    });
    const stopped = await twin("codex-stop", codex, (staged) => working(staged, true), (staged) => [{ name: "bmo:codex:stop", run: run(staged) }], { own });
    const overran = await twin("codex-bound", codex, (staged) => working(staged, false), (staged) => [{ name: "bmo:codex:bound", run: run(staged), timeoutMs: 300 }], { own });

    expect(stopped.turns[0]?.came).toEqual({ error: "the turn was stopped" });
    expect(stopped.turns[0]?.row?.state).toBe("cancelled");
    expect(overran.turns[0]?.came).toEqual({ error: "the turn ran past its time" });
    expect(overran.turns[0]?.row?.state).toBe("timeout");
    expect(overran.put.some((one) => one.method === "pane.send_keys")).toBe(true);
  }, 60_000);
});

describe("a turn of agency's loop, by either side", () => {
  const loop: Surface = {
    provider: "openrouter",
    bun: (request) => residentLoopTurn({ ...request, surface: openRouterProvider.resident as ResidentSurface, provider: "openrouter", pollMs: 5, readyMs: 5_000, settleMs: 50 }),
    command: (run, place) => (openRouterProvider.resident as ResidentSurface).command({ ...run, resident: { place, settingsPath: null } }),
    clocks: { pollMs: 5, readyMs: 5_000, settleMs: 50 },
  };
  const run = (staged: Staged) =>
    harnessRun({
      model: "qwen/qwen3.8-27b:free",
      cwd: staged.cwd,
      env: { PATH: "/usr/bin:/bin", BMO_OPENROUTER_KEY: "sk-or-bot", BMO_DSH_LOCAL_URL: "http://127.0.0.1:1/tap", BMO_TURN_CHAT: "-1" },
    });

  // The loop the pane's program would be: it takes each turn out of the inbox and writes what the
  // scenario says the turn came to into its record, and keeps what it was handed.
  function loopBeside(answer: (staged: Staged, turn: { id: string; name: string }) => unknown[] | Promise<unknown[]>) {
    return (staged: Staged) => {
      const place = agentPlace(staged.agent);
      const taken: unknown[] = [];
      const tending = setInterval(async () => {
        const turn = takeLoopTurn(place);

        if (turn === null) return;

        taken.push({ ...turn, id: "<id>" });

        const lines = [{ kind: "start", id: turn.id, name: turn.name, title: turn.title, at: Date.now(), typed: false }, ...(await answer(staged, turn))];

        appendFileSync(join(place, "loop.jsonl"), `${lines.map((line) => JSON.stringify(line)).join("\n")}\n`);
      }, 5);

      return () => {
        clearInterval(tending);
        return taken;
      };
    };
  }

  test("goes in through the inbox with only its own variables and is read back out of the loop's record", async () => {
    const seen = await twin(
      "loop-answer",
      loop,
      () => ({ answer: async () => null }),
      (staged) => [{ name: "chat:9", run: run(staged) }],
      { beside: loopBeside(() => [{ kind: "answer", text: " the loop's answer ", model: "qwen/qwen3.8-27b:free", usage: { inputTokens: 30, outputTokens: 4 } }, { kind: "end" }]) },
    );

    expect((seen.turns[0]?.came as { reply: { text: string } }).reply.text).toBe("the loop's answer");
    expect(JSON.stringify(seen.beside)).not.toContain("sk-or-bot");
  }, 60_000);

  test("a failed turn, and one with no answer, carry no reply", async () => {
    const failed = await twin("loop-failed", loop, () => ({ answer: async () => null }), (staged) => [{ name: "chat:10", run: run(staged) }], {
      beside: loopBeside(() => [{ kind: "failed", text: "429 Too Many Requests" }]),
    });
    const empty = await twin("loop-empty", loop, () => ({ answer: async () => null }), (staged) => [{ name: "chat:11", run: run(staged) }], {
      beside: loopBeside(() => [{ kind: "end" }]),
    });

    expect(failed.turns[0]?.came).toEqual({ error: "429 Too Many Requests" });
    expect(empty.turns[0]?.came).toEqual({ error: "the loop wrote no answer" });
  }, 60_000);

  test("told to stop or past its bound, is cut short with the surface's interrupt", async () => {
    const stopped = await twin("loop-stop", loop, () => ({ answer: async () => null }), (staged) => [{ name: "chat:12", run: run(staged) }], {
      beside: loopBeside(async (staged) => {
        await staged.archive.settings().setSetting(executionCancelKey("chat:12"), "1");
        return [];
      }),
    });
    const overran = await twin("loop-bound", loop, () => ({ answer: async () => null }), (staged) => [{ name: "chat:13", run: run(staged), timeoutMs: 300 }], {
      beside: loopBeside(() => []),
    });

    expect(stopped.turns[0]?.came).toEqual({ error: "the turn was stopped" });
    expect(stopped.turns[0]?.row?.state).toBe("cancelled");
    expect(overran.turns[0]?.came).toEqual({ error: "the turn ran past its time" });
    expect(overran.turns[0]?.row?.state).toBe("timeout");
  }, 60_000);
});

describe("a turn of the registry's Claude Code on the gateway, by either side", () => {
  const model = { home: registered, name: "qwen-test" };
  const endpoint = { provider: "registered", model: async () => model } as unknown as ClaudeEndpoint;
  // The road's program as its provider composes it, on the session the run goes on in or opens.
  const command = async (run: HarnessRun): Promise<HarnessCommand> => ({
    args: ["claude", "--model", "qwen-test", run.resumeSessionId === null ? "--session-id" : "--resume", run.resumeSessionId ?? run.openSessionId ?? "", "--settings", run.resident?.settingsPath ?? ""],
    cwd: run.cwd,
    env: { PATH: "/usr/bin:/bin", CLAUDE_CONFIG_DIR: registered, ANTHROPIC_BASE_URL: "http://127.0.0.1:9/gateway" },
    stdin: null,
  });
  const surface = { command } as unknown as ResidentSurface;
  const road: Surface = {
    provider: "registered",
    bun: (request) => endpointTurn({ ...request, surface, endpoint }),
    command: async (run, place) => {
      const resident = { place, settingsPath: endpointPlaceSettings(run, place, model as never) };

      return await command({ ...run, openSessionId: run.resumeSessionId === null ? crypto.randomUUID() : null, resident });
    },
    clocks: {},
  };
  const run = (staged: Staged, overrides: Partial<HarnessRun> = {}) => harnessRun({ model: "qwen-test", effort: "high", cwd: staged.cwd, ...overrides });
  const herdr = (): HerdrOptions => ({ projects: cliProjectsRoot(registered), answer: async () => [answerRecord({ text: "done", input: 5 })] });

  test("a round opens a session of its own behind the round's lead, and a word goes on in the session the program stands on", async () => {
    const continued = "5b1d5a1c-0000-4000-8000-0000000000e1";
    const seen = await twin(
      "road",
      road,
      herdr,
      (staged) => [
        { name: "chat:20", run: run(staged) },
        { name: "worker-word:1", run: run(staged, { line: "worker", resumeSessionId: continued }) },
      ],
    );

    expect(seen.turns.map((one) => ("reply" in one.came ? (one.came.reply as { mode: string; text: string }) : one.came))).toMatchObject([
      { mode: "registered", text: "done" },
      { mode: "registered", text: "done" },
    ]);
    expect(seen.put.some((one) => one.method === "pane.send_text" && String(one.params.text).includes("The text pasted after this line is this round"))).toBe(true);
  }, 90_000);

  test("a turn that wrote the program's own error is that error", async () => {
    const failed = { type: "assistant", isApiErrorMessage: true, message: { id: "e", model: "<synthetic>", content: [{ type: "text", text: "API Error: 529 Overloaded" }] } };
    const seen = await twin(
      "road-errored",
      road,
      () => ({ projects: cliProjectsRoot(registered), answer: async () => [failed] }),
      (staged) => [{ name: "worker-word:2", run: run(staged, { line: "worker", resumeSessionId: "5b1d5a1c-0000-4000-8000-0000000000e2" }) }],
    );

    expect(seen.turns[0]?.came).toEqual({ error: "API Error: 529 Overloaded" });
  }, 60_000);
});
