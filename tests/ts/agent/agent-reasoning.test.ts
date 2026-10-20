import "@agency/test-support/setup";

import { expect, test } from "bun:test";

import { agentEffort, agentReasoning, mappedRun } from "@agency/agent/model-settings";
import { scratchArchive } from "@agency/test-support/store";

import type { HarnessRun } from "@agency/agent/harness";

// An agent's own thinking and effort over its tier (docs/model-tiers.md, 「An agent's own thinking
// and effort」).

test("each provider makes its own word of an agent's choice, and the tier's word stands where none reaches", async () => {
  const level = (effort: "high" | "max") => ({ thinking: null, effort });

  // Claude Code takes every level of a model its catalogue names no efforts for.
  expect(await agentEffort({ provider: "claude-code", model: "claude-unlisted", effort: "high" }, level("max"))).toBe("max");
  // OpenRouter's highest is xhigh; a mapping of it names no effort.
  expect(await agentEffort({ provider: "openrouter", model: "some/model", effort: null }, level("high"))).toBe("high");
  expect(await agentEffort({ provider: "openrouter", model: "some/model", effort: null }, level("max"))).toBeNull();
  // A registered model takes thinking alone, as its effort word.
  expect(await agentEffort({ provider: "registered", model: "local-0-1", effort: null }, { thinking: true, effort: "max" })).toBe("thinking");
  expect(await agentEffort({ provider: "registered", model: "local-0-1", effort: "thinking" }, { thinking: false, effort: null })).toBe("no-thinking");
  expect(await agentEffort({ provider: "registered", model: "local-0-1", effort: null }, level("max"))).toBeNull();
  // A run no agent's choice reaches goes out on its mapping.
  expect(await agentEffort({ provider: "claude-code", model: "claude-unlisted", effort: "high" }, undefined)).toBe("high");
  expect(await mappedRun({ effort: "high" } as HarnessRun, { provider: "codex", model: "terra", effort: "low", tier: "Fine" })).toMatchObject({ model: "terra", effort: "low", tier: "Fine" });
  expect((await mappedRun({ effort: "high", reasoning: level("max") } as HarnessRun, { provider: "codex", model: "terra", effort: "low", tier: "Fine" })).effort).toBe("max");
});

test("a support worker runs on its base's choice", async () => {
  const archive = await scratchArchive();

  await archive.agents().addWorker({ name: "base", label: "Base", cwd: "/srv/base" });
  await archive.sql.run("INSERT INTO agents (name, kind, support_of, support_slot, builtin) VALUES ('base-w2', 'worker', 'base', 2, 0)");
  await archive.agents().setReasoning("base", { thinking: false, effort: "low" });

  expect(await agentReasoning("base-w2", archive)).toEqual({ thinking: false, effort: "low" });
  expect(await agentReasoning("nobody", archive)).toEqual({ thinking: null, effort: null });
});
