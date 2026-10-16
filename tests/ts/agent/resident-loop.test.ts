import { chmodSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "bun:test";

import { agentPlace, readRun } from "@agency/agent/agent-place";
import { Archive } from "@agency/archive/archive";
import { loopRecordPath } from "@agency/agent/loop-record";
import { standInHerdr } from "@agency/test-support/herdr-standin";
import { residentLoopTurn } from "@agency/agent/loop-turn";
import { installProviderSettings } from "@agency/agent/provider-order";
import { dshHome, openRouterProvider } from "@agency/agent/dsh";
import { dshTurnUsage, putLoopTurn, readLoopRecord, ResidentLoop, StepRelay } from "@agency/agent/loop-program";
import { codeRoot } from "@agency/support/roots";
import { scratchDir } from "@agency/test-support/scratch";
import { toolHostStandIn } from "@agency/test-support/tool-host-stand-in";
import { headCommand } from "@agency/agent/units";

import type { HarnessRun } from "@agency/agent/harness";
import type { AgentHome } from "@agency/agent/herdr-run";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

// agency's own loop standing in the pane of an agent on OpenRouter (docs/agent-control.md, 「The
// interactive surface a provider declares」): a turn from its inbox or a line typed at its screen,
// one dsh for each, the reasoning and the answer on its screen, and its record read back as the
// reply. The dsh here is a stand-in `npx` on the loop's PATH that answers the way headless does.

// A program written into a place is handed its session token first, by the tool host stood in here.
const toolHost = toolHostStandIn(scratchDir("bmo-tool-host-"));

afterAll(() => toolHost.stop());

const root = scratchDir("bmo-resident-loop-");
const real = { deploy: process.env.BMO_DEPLOY_DIR, data: process.env.BMO_DATA_DIR };
const bin = join(root, "bin");
const seenKey = join(root, "seen-key");
const seenTurn = join(root, "seen-turn");
const slowPid = join(root, "slow-pid");

process.env.BMO_DEPLOY_DIR = join(root, "deploy");
process.env.BMO_DATA_DIR = join(root, "data");
mkdirSync(bin, { recursive: true });
// Answers at once, reasoning on stderr and the answer on stdout, and notes the key and the turn's
// room and run it was handed;
// told to be slow, it works until it is ended, and says which process it is; handed a session log,
// it files it where dsh files the log of its run.
writeFileSync(join(bin, "npx"), `#!/bin/sh
printf '%s' "$BMO_OPENROUTER_KEY" > "${seenKey}"
printf '%s %s' "$BMO_TURN_CHAT" "$BMO_RUN_NAME" > "${seenTurn}"
if [ -n "$LOOP_STEPS_TO" ]; then
  mkdir -p "$(dirname "$LOOP_STEPS_TO")"
  cp "$LOOP_STEPS_FROM" "$LOOP_STEPS_TO"
fi
if [ "$LOOP_SLOW" = "1" ]; then
  printf '%s' "$$" > "${slowPid}"
  sleep 30
fi
printf 'thinking\\n' >&2
printf 'the loop answer\\n'
`);
chmodSync(join(bin, "npx"), 0o755);

afterAll(() => {
  for (const [name, value] of [["BMO_DEPLOY_DIR", real.deploy], ["BMO_DATA_DIR", real.data]] as const) {
    if (value === undefined) delete process.env[name];
    else process.env[name] = value;
  }

  rmSync(root, { recursive: true, force: true });
});

function run(overrides: Partial<HarnessRun> = {}): HarnessRun {
  return {
    line: "chat", model: "qwen/qwen3.8-27b:free", effort: "", systemPromptPath: null, toolsShown: "", toolsAllowed: null, toolsDenied: null,
    mcpConfigPath: null, prompt: "hello", promptDelivery: "argument", resumeSessionId: null, keepSession: false, hostCommands: false,
    thinking: true, answerShape: "text", cwd: root, env: {}, ...overrides,
  };
}

function loopAt(place: string, env: Record<string, string> = {}): { loop: ResidentLoop; screen: () => string } {
  let drawn = "";
  const loop = new ResidentLoop({
    provider: "openrouter", place, line: "chat", model: "qwen/qwen3.8-27b:free", pollMs: 10,
    env: { PATH: `${bin}:/usr/bin:/bin`, BMO_OPENROUTER_KEY: "sk-or-loop", ...env },
    write: (text) => { drawn += text; },
  });

  return { loop, screen: () => drawn };
}

async function until(done: () => boolean, ms = 5_000): Promise<void> {
  for (const deadline = Date.now() + ms; !done() && Date.now() < deadline;) await Bun.sleep(10);
}

describe("agency's resident loop", () => {
  test("a line typed at its screen is a person's turn, on the run the bot last put in", async () => {
    const place = join(root, "typed-place");
    const { loop, screen } = loopAt(place);

    loop.start();

    try {
      // Nothing has come from the bot yet, so there is no run for a typed line to go out on.
      loop.input("first\r");
      await Bun.sleep(50);
      expect(readLoopRecord(place)).toEqual([]);
      expect(screen()).toContain("so a typed line cannot be run");

      putLoopTurn(place, { id: "t1", name: "chat:1", title: "a talk", run: run() });
      await until(() => readLoopRecord(place).some((record) => record.kind === "end"));

      loop.input("hello loop");
      loop.input("\x7fp\r");
      await until(() => readLoopRecord(place).filter((record) => record.kind === "end").length === 2);
    } finally {
      loop.stop();
    }

    const typed = readLoopRecord(place).slice(3);

    expect(typed.map((record) => record.kind)).toEqual(["start", "answer", "end"]);
    expect(typed[0]).toMatchObject({ name: "typed line", title: "hello loop", typed: true });
    expect(screen()).toContain("── typed line · hello loop ·");
  });

  // A line of dsh's log is `{ type, seq, time, data }`: a streamed chunk of type `usage` carries a
  // request's figures, a refused request writes a chunk of zeros, and a whole response carries the
  // same figures on its message. The sessions are filed by the directory the run worked in.
  test("the usage of a turn is what dsh's own session log wrote for it, each request once", () => {
    const home = join(root, "dsh-home");
    const write = (cwd: string, session: string, lines: unknown[]) => {
      const log = join(home, "sessions", `-${cwd.replaceAll("/", "-")}--`, session, "session.jsonl.zstd");

      mkdirSync(join(log, ".."), { recursive: true });
      writeFileSync(log, Bun.zstdCompressSync(new TextEncoder().encode(lines.map((line) => `${JSON.stringify(line)}\n`).join(""))));
    };
    const chunk = (inputTokens: number, outputTokens: number) =>
      ({ type: "assistant/chunk", seq: 1, time: 1, data: { turn: 1, step: 1, chunk: { type: "usage", usage: { inputTokens, outputTokens, totalTokens: inputTokens + outputTokens } } } });
    const message = (inputTokens: number, outputTokens: number, cached: { cacheReadTokens?: number } = {}) =>
      ({ type: "assistant/message", seq: 2, time: 2, data: { message: { role: "assistant", usage: { inputTokens, outputTokens, totalTokens: inputTokens + outputTokens, ...cached } } } });

    // Streamed and whole: the responses are what is counted, so no request counts twice.
    write("/work/one", "session-a", [{ type: "turn/start", seq: 0, time: 0, data: {} }, chunk(0, 0), chunk(1200, 30), message(1200, 30), chunk(1300, 12), message(1300, 12)]);
    // Another directory's run in the same home is not this turn's.
    write("/work/other", "session-b", [chunk(9000, 900), message(9000, 900)]);
    // Streamed alone: the chunks are counted.
    write("/work/two", "session-c", [chunk(0, 0), chunk(800, 20)]);
    // The share the cache gave stands beside the input where the upstream reports one; a response naming none adds none.
    write("/work/four", "session-e", [message(300, 10, { cacheReadTokens: 900 }), message(200, 5)]);
    // Refused on every try: zeros are no usage.
    write("/work/three", "session-d", [chunk(0, 0), chunk(0, 0)]);

    expect(dshTurnUsage(home, 0, "/work/one")).toEqual({ inputTokens: 2500, outputTokens: 42, cacheReadTokens: 0 });
    expect(dshTurnUsage(home, 0, "/work/two")).toEqual({ inputTokens: 800, outputTokens: 20, cacheReadTokens: 0 });
    expect(dshTurnUsage(home, 0, "/work/four")).toEqual({ inputTokens: 500, outputTokens: 15, cacheReadTokens: 900 });
    expect(dshTurnUsage(home, 0, "/work/three")).toBeNull();
    expect(dshTurnUsage(home, Date.now() + 60_000, "/work/one")).toBeNull();
  });

  // dsh writes its session log as whole zstd frames, one for each batch of lines.
  const frame = (lines: unknown[]) => Bun.zstdCompressSync(new TextEncoder().encode(lines.map((line) => `${JSON.stringify(line)}\n`).join("")));
  const called = { type: "assistant/message", seq: 3, time: 3, data: { message: { id: "message-1", role: "assistant", content: [
    { type: "reasoning", text: "looking" },
    { type: "tool-call", id: "call-1", name: "bash", arguments: "{\"command\":\"ls\"}" },
  ] }, usage: { inputTokens: 30, outputTokens: 5, cacheReadTokens: 70 } } };
  const answered = { type: "tool/result", seq: 4, time: 4, data: { message: { role: "user", content: [
    { type: "tool-result", toolCallId: "call-1", content: [{ type: "text", text: "x".repeat(5_000) }], isError: false },
  ] } } };
  const said = { type: "assistant/message", seq: 5, time: 5, data: { message: { id: "message-2", role: "assistant", content: [{ type: "text", text: "done" }] }, usage: { inputTokens: 40, outputTokens: 2 } } };
  const calledStep = {
    kind: "step", id: "turn-1", type: "assistant",
    message: { id: "message-1", content: [{ type: "thinking", thinking: "looking" }, { type: "tool_use", id: "call-1", name: "bash", input: { command: "ls" } }], usage: { input_tokens: 30, cache_read_input_tokens: 70, output_tokens: 5 } },
  };

  test("the steps dsh takes go into the record as its session log is written, each once", () => {
    const home = join(root, "relay-home");
    const log = join(home, "sessions", "--work-relay--", "session-a", "session.jsonl.zstd");
    const steps: unknown[] = [];
    const relay = new StepRelay({ home, since: 0, cwd: "/work/relay", id: "turn-1", cut: 100, write: (step) => steps.push(step) });
    const first = frame([{ type: "turn/start", seq: 0, time: 0, data: {} }, { type: "reasoning-chunks", seq: 1, time: 1, data: {} }, called]);
    const second = frame([answered, said]);

    mkdirSync(join(log, ".."), { recursive: true });
    writeFileSync(log, first);
    relay.relay();
    expect(steps).toEqual([calledStep]);

    // A frame still being written is left for the next reading, which finds it whole.
    writeFileSync(log, Buffer.concat([first, second.subarray(0, second.length - 4)]));
    relay.relay();
    expect(steps).toHaveLength(1);

    writeFileSync(log, Buffer.concat([first, second]));
    relay.relay();
    relay.relay();
    expect(steps.slice(1)).toEqual([
      { kind: "step", id: "turn-1", type: "user", message: { content: [{ type: "tool_result", tool_use_id: "call-1", content: "x".repeat(100), is_error: false }] } },
      { kind: "step", id: "turn-1", type: "assistant", message: { id: "message-2", content: [{ type: "text", text: "done" }], usage: { input_tokens: 40, cache_read_input_tokens: 0, output_tokens: 2 } } },
    ]);
  });

  test("a turn's record holds the steps its dsh took before its answer", async () => {
    const place = join(root, "steps-place");
    const cwd = join(root, "steps-work");
    const from = join(root, "steps.jsonl.zstd");
    const { loop } = loopAt(place, {
      LOOP_STEPS_FROM: from,
      LOOP_STEPS_TO: join(dshHome(), "sessions", `-${cwd.replaceAll("/", "-")}--`, "session-steps", "session.jsonl.zstd"),
    });

    mkdirSync(cwd, { recursive: true });
    writeFileSync(from, frame([called, answered, said]));
    loop.start();

    try {
      putLoopTurn(place, { id: "turn-1", name: "chat:1", title: "sample", run: run({ cwd }) });
      await until(() => readLoopRecord(place).some((record) => record.kind === "end"));
    } finally {
      loop.stop();
    }

    const record = readLoopRecord(place);

    expect(record.map((line) => line.kind)).toEqual(["start", "step", "step", "step", "answer", "end"]);
    expect(record[1]).toEqual(calledStep as (typeof record)[number]);
    expect(record[4]).toMatchObject({ kind: "answer", usage: { inputTokens: 70, outputTokens: 7, cacheReadTokens: 70 } });
  });
});

describe("a turn of the resident loop, from the bot's side", () => {
  test("goes in through the inbox and is read back out of the loop's record as the reply", async () => {
    const archive = await scratchArchive();
    const herdr = standInHerdr("resident-loop", { answer: async () => null });
    const agent = "loop-bmo";
    const place = agentPlace(agent);
    const home: AgentHome = { agent, label: agent, head: { command: headCommand(agent, codeRoot), cwd: root, env: {} } };
    // The loop the pane's program would be, running here beside the stand-in's.
    const { loop } = loopAt(place);

    installProviderSettings(archive);
    loop.start();

    try {
      const turn = await residentLoopTurn({
        run: run({ env: { PATH: "/usr/bin", BMO_OPENROUTER_KEY: "sk-or-bot", BMO_DSH_LOCAL_URL: "http://127.0.0.1:1/tap", BMO_TURN_CHAT: "-1" } }),
        home, name: "chat:9", title: "a talk", timeoutMs: 10_000, socket: herdr.socket, pollMs: 10, readyMs: 5_000,
        surface: openRouterProvider.resident!, provider: "openrouter",
      });

      expect("reply" in turn ? turn.reply.text : turn.error).toBe("the loop answer");
      expect(readLoopRecord(place)[0]).toMatchObject({ kind: "start", name: "chat:9", typed: false });
      // The loop's own command carries no variable of one turn's, so its identity stays put.
      expect(readFileSync(join(place, "program.json"), "utf8")).not.toContain("BMO_DSH_LOCAL_URL");
      expect(readFileSync(join(place, "program.json"), "utf8")).not.toContain("BMO_TURN_CHAT");
      // The turn's room and its name are the run's, in the place as they went with the turn.
      expect(readRun(place)?.env).toEqual({ BMO_RUN_NAME: "chat:9", BMO_TURN_CHAT: "-1" });
      // And the dsh the loop ran for the turn was handed both in its own environment.
      expect(readFileSync(seenTurn, "utf8")).toBe("-1 chat:9");
      expect((await archive.executions().executions()).find((row) => row.provider === "openrouter")).toMatchObject({ state: "exited", resident: { record: loopRecordPath(place) } });
      expect(herdr.asked.some((one) => one.method === "pane.close")).toBe(false);
    } finally {
      loop.stop();
      installProviderSettings(null);
      herdr.stop();
    }
  }, 30_000);
});
