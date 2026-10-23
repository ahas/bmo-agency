import "@agency/test-support/setup";

import { expect, test } from "bun:test";

import { dshOverlay } from "@agency/agent/dsh";
import { deepSeekProvider, deepSeekRoute } from "@agency/agent/deepseek";
import { scratchArchive } from "@agency/test-support/store";

import type { HarnessRun } from "@agency/agent/harness";

// DeepSeek's road (docs/cache-fallback.md, 「The DeepSeek road」): its two models, its route through dsh,
// and the words a run's thinking and effort go out with.

const run = (model: string, thinking = true): HarnessRun => ({
  line: "chat",
  model,
  effort: "",
  systemPromptPath: null,
  toolsShown: "",
  toolsAllowed: "",
  toolsDenied: null,
  mcpConfigPath: null,
  prompt: "hello",
  promptDelivery: "argument",
  resumeSessionId: null,
  keepSession: false,
  hostCommands: false,
  thinking,
  answerShape: "text",
  cwd: "/work",
  env: {},
});

test("its runs go through dsh on DeepSeek's own route, the key named by variable and read from the road's file", () => {
  expect(deepSeekProvider.adapter.name).toBe("dsh:deepseek");
  expect(deepSeekProvider.resident).toBeNull();
  expect(deepSeekRoute.apiBase()).toBe("https://api.deepseek.com/v1");
  expect(deepSeekRoute.keyEnv).toBe("DEEPSEEK_API_KEY");
  expect(deepSeekRoute.keyFile?.().endsWith("/env/deepseek.key")).toBe(true);

  const rows = new Map(dshOverlay(run("deepseek-flash", false), deepSeekRoute).map((row) => [row.id, row]));

  expect(rows.get("llm-pi-ai")?.config).toMatchObject({
    providers: { deepseek: { apiKeyEnv: "DEEPSEEK_API_KEY", baseURL: "https://api.deepseek.com/v1", reasoning: "off" } },
  });
  expect(rows.get("agent-default-model")?.config).toEqual({ provider: "deepseek", model: "deepseek-flash" });
});

test("an agent's thinking off is DeepSeek's own none, and a level the run takes stands", async () => {
  expect(await deepSeekProvider.reasoning.effort("high", { thinking: false, effort: null }, "deepseek-flash")).toBe("none");
  expect(await deepSeekProvider.reasoning.effort("high", { thinking: true, effort: "max" }, "deepseek-flash")).toBe("max");
  expect(await deepSeekProvider.reasoning.effort("low", { thinking: null, effort: null }, "deepseek-flash")).toBe("low");
  expect(deepSeekProvider.efforts).toEqual([null, "none", "low", "medium", "high", "max"]);
  expect(deepSeekProvider.lines).toEqual(["chat", "gate", "worker"]);
  expect(deepSeekProvider.lendsCoworkers).toBe(true);
});

test("a pin naming one of its models is taken and another is refused", async () => {
  const archive = await scratchArchive();

  expect(await deepSeekProvider.namedModelFault?.("deepseek-v4-pro", archive)).toBeNull();
  expect(await deepSeekProvider.namedModelFault?.("someone-else", archive)).toContain("Not a DeepSeek model");
});
