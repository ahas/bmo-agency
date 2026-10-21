import "@agency/test-support/setup";

import { expect, test } from "bun:test";

import { deepSeekEndpoint, deepSeekProvider } from "@agency/agent/deepseek";
import { scratchArchive } from "@agency/test-support/store";

// DeepSeek's road (docs/cache-fallback.md, 「The DeepSeek road」): its two models, its endpoint, and the
// words a run's thinking and effort go out with.

const run = (model: string, effort = "", thinking = true) => ({ model, effort, thinking });

test("the endpoint holds the two models its policy declares and refuses another", async () => {
  const flash = await deepSeekEndpoint.model(run("deepseek-flash"));

  expect(flash).toMatchObject({
    name: "deepseek-flash",
    label: "DeepSeek V4.1 Flash",
    contextTokens: 1_048_576,
    answerTokens: 32_768,
    thinking: true,
  });
  expect(flash.home.endsWith("/claude-roads/deepseek/deepseek-flash")).toBe(true);
  expect((await deepSeekEndpoint.model(run("deepseek-v4-pro"))).label).toBe("DeepSeek V4 Pro");
  await expect(deepSeekEndpoint.model(run("deepseek-unknown"))).rejects.toThrow("Not a DeepSeek model");
  expect(deepSeekEndpoint.keyPath().endsWith("/env/deepseek.key")).toBe(true);
  expect(deepSeekEndpoint.baseUrl).toBe("https://api.deepseek.com/anthropic");
});

test("a run that does not think carries no thinking, and its effort is the program's lowest", async () => {
  expect((await deepSeekEndpoint.model(run("deepseek-flash", "none"))).thinking).toBe(false);
  expect((await deepSeekEndpoint.model(run("deepseek-flash", "high", false))).thinking).toBe(false);
  expect(deepSeekEndpoint.effort({ thinking: true, effort: "high" })).toBe("high");
  expect(deepSeekEndpoint.effort({ thinking: true, effort: "" })).toBe("high");
  expect(deepSeekEndpoint.effort({ thinking: false, effort: "none" })).toBe("low");
});

test("an agent's thinking off is DeepSeek's own none, and a level the run takes stands", async () => {
  expect(await deepSeekProvider.reasoning.effort("high", { thinking: false, effort: null }, "deepseek-flash")).toBe("none");
  expect(await deepSeekProvider.reasoning.effort("high", { thinking: true, effort: "max" }, "deepseek-flash")).toBe("max");
  expect(await deepSeekProvider.reasoning.effort("low", { thinking: null, effort: null }, "deepseek-flash")).toBe("low");
  expect(deepSeekProvider.efforts).toEqual([null, "none", "low", "medium", "high", "max"]);
  expect(deepSeekProvider.lines).toEqual(["chat", "worker", "editor"]);
  expect(deepSeekProvider.lendsCoworkers).toBe(true);
});

test("a pin naming one of its models is taken and another is refused", async () => {
  const archive = await scratchArchive();

  expect(await deepSeekProvider.namedModelFault?.("deepseek-v4-pro", archive)).toBeNull();
  expect(await deepSeekProvider.namedModelFault?.("someone-else", archive)).toContain("Not a DeepSeek model");
});
