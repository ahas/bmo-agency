import { appendFileSync, lstatSync, mkdirSync, readFileSync, readlinkSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "bun:test";

import { agentPlace, readRun } from "@agency/agent/agent-place";
import { Archive } from "@agency/archive/archive";
import { codexResident } from "@agency/agent/codex";
import { prepareCodexHome } from "@agency/agent/codex-local";
import { residentCodexTurn } from "@agency/agent/codex-resident";
import { standInHerdr } from "@agency/test-support/herdr-standin";
import { installProviderSettings } from "@agency/agent/provider-order";
import { readProgram, readRunning } from "@agency/agent/resident-pane";
import { codeRoot } from "@agency/support/roots";
import { scratchDir } from "@agency/test-support/scratch";
import { toolHostStandIn } from "@agency/test-support/tool-host-stand-in";
import { headCommand } from "@agency/agent/units";

import type { HarnessRun } from "@agency/agent/harness";
import type { AgentHome } from "@agency/agent/herdr-run";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

// Codex's interactive program standing in its agent's pane (docs/agent-control.md, 「The interactive
// surface a provider declares」): under a home of agency's own that shares the user's login and threads,
// fed a turn as a person would type it, and read back out of the thread's rollout. The stand-in herdr
// plays the pane's loop and the program, and writes the rollout lines a recorded turn wrote.

// A program written into a place is handed its session token first, by the tool host stood in here.
const toolHost = toolHostStandIn(scratchDir("bmo-tool-host-"));

afterAll(() => toolHost.stop());

const root = scratchDir("bmo-codex-resident-");
const realDataDir = process.env.BMO_DATA_DIR;
const userHome = join(root, "user-codex");
const codexHome = join(root, "codex-home");
const cwd = join(root, "work");
const thread = "019a1b2c-0000-7000-8000-00000000c0de";
const rollout = join(userHome, "sessions", "2026", "09", "21", `rollout-2026-09-21T12-03-04-${thread}.jsonl`);

process.env.BMO_DATA_DIR = join(root, "data");
mkdirSync(cwd, { recursive: true });

afterAll(() => {
  if (realDataDir === undefined) delete process.env.BMO_DATA_DIR;
  else process.env.BMO_DATA_DIR = realDataDir;

  rmSync(root, { recursive: true, force: true });
});

const home = (agent: string): AgentHome => ({ agent, label: agent, head: { command: headCommand(agent, codeRoot), cwd: root, env: {} } });

function run(overrides: Partial<HarnessRun> = {}): HarnessRun {
  return {
    line: "chat", model: "gpt-5.6-luna", effort: "high", systemPromptPath: null, toolsShown: "", toolsAllowed: null, toolsDenied: null,
    mcpConfigPath: null, prompt: "hello", promptDelivery: "input", resumeSessionId: null, keepSession: true, hostCommands: false,
    thinking: true, answerShape: "text", cwd, env: { PATH: "/usr/bin:/bin" }, ...overrides,
  };
}

// The lines a turn of the program writes to its thread's rollout, in the shape a recorded rollout
// carries them: the task it started, the agent's words, a count per request with the account's
// windows, and the task completed.
function turnLines(text: string, windows: { primary: number; secondary: number } = { primary: 12, secondary: 3 }): unknown[] {
  const at = new Date().toISOString();

  return [
    { timestamp: at, type: "turn_context", payload: { model: "gpt-5.6-luna" } },
    { timestamp: at, type: "event_msg", payload: { type: "task_started", turn_id: "t", model_context_window: 272000 } },
    { timestamp: at, type: "event_msg", payload: { type: "agent_message", message: text } },
    {
      timestamp: at,
      type: "event_msg",
      payload: {
        type: "token_count",
        info: { last_token_usage: { input_tokens: 1000, cached_input_tokens: 900, cache_write_input_tokens: 0, output_tokens: 12, total_tokens: 1012 } },
        rate_limits: {
          limit_id: "codex",
          primary: { used_percent: windows.primary, window_minutes: 300, resets_at: 1790000000 },
          secondary: { used_percent: windows.secondary, window_minutes: 10080, resets_at: 1790500000 },
        },
      },
    },
    { timestamp: at, type: "event_msg", payload: { type: "task_complete", turn_id: "t", last_agent_message: text, duration_ms: 900 } },
  ];
}

describe("the resident Codex home", () => {
  test("shares the user's login and threads, and trusts the folder in its own configuration once", () => {
    const user = join(root, "home-user");
    const own = join(root, "home-own");

    mkdirSync(user, { recursive: true });
    writeFileSync(join(user, "auth.json"), "{}");
    writeFileSync(join(user, "config.toml"), "model = \"gpt-5.6-terra\"\n");

    prepareCodexHome("/work/one", own, user);
    prepareCodexHome("/work/one", own, user);
    prepareCodexHome("/work/two", own, user);

    expect(readlinkSync(join(own, "auth.json"))).toBe(join(user, "auth.json"));
    expect(readlinkSync(join(own, "sessions"))).toBe(join(user, "sessions"));
    expect(lstatSync(join(own, "config.toml")).isSymbolicLink()).toBe(false);
    expect(readFileSync(join(own, "config.toml"), "utf8")).toBe(
      '[projects."/work/one"]\ntrust_level = "trusted"\n[projects."/work/two"]\ntrust_level = "trusted"\n',
    );
    // The user's own configuration is neither read into it nor touched.
    expect(readFileSync(join(user, "config.toml"), "utf8")).toBe("model = \"gpt-5.6-terra\"\n");
  });
});

describe("a turn of Codex's resident program", () => {
  test("opens its thread, is relabelled onto it, and the next turn resumes it in the same program", async () => {
    const archive = await scratchArchive();
    const place = agentPlace("codex-bmo");
    let told = 0;
    const herdr = standInHerdr("codex-resident", {
      record: () => rollout,
      ending: [],
      answer: async ({ text }) => {
        told += 1;

        const opening = told === 1 ? [{ timestamp: new Date().toISOString(), type: "session_meta", payload: { id: thread, cwd } }] : [];

        return [...opening, ...turnLines(text.endsWith("next") ? "the second answer" : "the first answer")];
      },
    });

    installProviderSettings(archive);

    try {
      const first = await residentCodexTurn({ run: run({ env: { PATH: "/usr/bin:/bin", BMO_TURN_CHAT: "-1" } }), home: home("codex-bmo"), name: "bmo:codex:1", title: "a talk", timeoutMs: 10_000, socket: herdr.socket, pollMs: 5, readyMs: 5_000, codexHome, userHome, surface: codexResident });

      expect("reply" in first ? first.reply.text : first.error).toBe("the first answer");
      expect("reply" in first ? first.reply.sessionId : null).toBe(thread);
      expect("reply" in first ? first.reply.windows.map((window) => window.name) : []).toEqual(["codex:codex:primary", "codex:codex:secondary"]);
      expect("reply" in first ? first.reply.lastRequestUsage : null).toEqual({ inputTokens: 100, cacheReadTokens: 900, cacheWriteTokens: 0, outputTokens: 12 });

      // The program was started as `codex` with no thread, under agency's own home, and is now the
      // program on the thread it opened: a fall-over would resume it.
      const program = readProgram(place);
      const pid = readRunning(place)?.pid;

      expect(program?.restart?.args.slice(0, 3)).toEqual(["codex", "resume", thread]);
      expect(program?.command.env.CODEX_HOME).toBe(codexHome);
      expect(readRunning(place)?.identity).toBe(program?.identity);
      // The program stands without the room its first run was about; the run put that in the place,
      // under its own name, for whatever the program runs to read.
      expect(program?.command.env.BMO_TURN_CHAT).toBeUndefined();
      expect(program?.command.env.BMO_AGENT_PLACE).toBe(place);
      expect(readRun(place)?.env).toEqual({ BMO_RUN_NAME: "bmo:codex:1", BMO_TURN_CHAT: "-1" });

      const second = await residentCodexTurn({
        run: run({ prompt: "next", resumeSessionId: thread, env: { PATH: "/usr/bin:/bin", BMO_TURN_CHAT: "42" } }), home: home("codex-bmo"), name: "bmo:codex:2", title: "a talk",
        timeoutMs: 10_000, socket: herdr.socket, pollMs: 5, readyMs: 5_000, codexHome, userHome, surface: codexResident,
      });

      // Resumed on the stand-in: the same process, and only what this turn added is its answer.
      expect("reply" in second ? second.reply.text : second.error).toBe("the second answer");
      // Another room's turn is a turn of the same program, and reads its own room.
      expect(readRunning(place)?.pid).toBe(pid);
      expect(herdr.asked.some((one) => one.method === "pane.close")).toBe(false);
      expect(readRun(place)?.env).toEqual({ BMO_RUN_NAME: "bmo:codex:2", BMO_TURN_CHAT: "42" });

      const rows = (await archive.executions().executions()).filter((row) => row.provider === "codex");

      expect(rows).toHaveLength(2);
      expect(rows.every((row) => row.state === "exited" && row.endedAt !== null && row.resident?.record === join(codexHome, "sessions", "2026", "09", "21", `rollout-2026-09-21T12-03-04-${thread}.jsonl`))).toBe(true);
    } finally {
      installProviderSettings(null);
      herdr.stop();
    }
  }, 30_000);

  test("a program showing the folder trust question, which herdr reads as idle, is left for a person", async () => {
    const archive = await scratchArchive();
    const herdr = standInHerdr("codex-resident-trust", {
      record: () => null,
      ending: [],
      answer: async () => null,
      screen: "> You are in /work\n\n  Do you trust the contents of this\n  directory? Working with untrusted contents\n\n› 1. Yes, continue\n  2. No, quit",
    });

    installProviderSettings(archive);

    try {
      const turn = await residentCodexTurn({
        run: run(), home: home("codex-trust"), name: "bmo:codex:trust", title: "a talk",
        timeoutMs: 10_000, socket: herdr.socket, pollMs: 5, readyMs: 5_000, codexHome, userHome, surface: codexResident,
      });

      expect("error" in turn ? turn.error : "").toStartWith("codex's program is waiting on a person");
      expect(herdr.asked.some((one) => one.method === "agent.prompt")).toBe(false);
      expect((await archive.executions().executions()).find((row) => row.provider === "codex")?.state).toBe("cancelled");
    } finally {
      installProviderSettings(null);
      herdr.stop();
    }
  }, 30_000);

  test("a turn the program ended on its own error is no answer, and says why", async () => {
    const archive = await scratchArchive();
    const failed = join(userHome, "sessions", "2026", "09", "21", "rollout-2026-09-21T13-00-00-019a-failed.jsonl");

    mkdirSync(join(userHome, "sessions", "2026", "09", "21"), { recursive: true });
    appendFileSync(failed, `${JSON.stringify({ type: "session_meta", payload: { id: "019a-failed", cwd } })}\n`);

    const herdr = standInHerdr("codex-resident-failed", {
      record: () => failed,
      ending: [],
      answer: async () => [
        { type: "event_msg", payload: { type: "task_started" } },
        { type: "event_msg", payload: { type: "token_count", info: null, rate_limits: { primary: { used_percent: 100, window_minutes: 10080, resets_at: 1790000000 } } } },
        { type: "event_msg", payload: { type: "task_complete", error: { codex_error_info: "usage_limit_exceeded", message: "You've hit your usage limit" } } },
      ],
    });

    installProviderSettings(archive);

    try {
      const turn = await residentCodexTurn({
        run: run({ resumeSessionId: "019a-failed" }), home: home("codex-failed"), name: "bmo:codex:3", title: "a talk",
        timeoutMs: 10_000, socket: herdr.socket, pollMs: 5, readyMs: 5_000, codexHome, userHome, surface: codexResident,
      });

      expect(turn).toEqual({ error: "usage_limit_exceeded: You've hit your usage limit" });
    } finally {
      installProviderSettings(null);
      herdr.stop();
    }
  }, 30_000);
});
