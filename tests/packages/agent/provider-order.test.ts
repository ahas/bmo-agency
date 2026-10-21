import "@agency/test-support/setup";

import { expect, test } from "bun:test";

import { Archive } from "@agency/archive/archive";
import { defaultProviderOrder, isDefaultProviderOrder, isProviderOrder, providerOrder, readProviderOrder, setProviderOrder, setProviderEnabled, enabledProviders } from "@agency/agent/provider-order";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

test("the default order is Claude Code, Codex, OpenRouter, the registry and DeepSeek, and reading it writes nothing", async () => {
  const archive = await scratchArchive();

  expect(await enabledProviders(archive)).toEqual(["claude-code", "codex", "openrouter", "registered", "deepseek"]);
  expect(defaultProviderOrder).toEqual(["claude-code", "codex", "openrouter", "registered", "deepseek"]);
  expect(await archive.settings().getSetting("provider_order")).toBeNull();
});

test("an order is every provider exactly once", () => {
  expect(isProviderOrder(["registered", "openrouter", "codex", "claude-code", "deepseek"])).toBe(true);
  expect(isProviderOrder(["claude-code", "codex", "openrouter"])).toBe(false);
  expect(isProviderOrder(["claude-code", "codex", "codex", "registered"])).toBe(false);
  expect(isProviderOrder(["claude-code", "codex", "openrouter", "gemini"])).toBe(false);
  expect(isProviderOrder(["claude-code", "codex", "openrouter", "registered", "retired"])).toBe(false);
  expect(isProviderOrder(["claude-code", "codex", "openrouter", "registered", "auto"])).toBe(false);
  expect(isProviderOrder("claude-code,codex,openrouter,registered")).toBe(false);
  expect(isProviderOrder([1, 2, 3, 4, 5])).toBe(false);
});

test("a changed order is kept, and setting the default back removes the row", async () => {
  const archive = await scratchArchive();

  await setProviderOrder(archive, ["codex", "claude-code", "openrouter", "registered", "deepseek"]);
  expect(await enabledProviders(archive)).toEqual(["codex", "claude-code", "openrouter", "registered", "deepseek"]);
  expect(isDefaultProviderOrder(await providerOrder(archive))).toBe(false);

  await setProviderOrder(archive, ["claude-code", "codex", "openrouter", "registered", "deepseek"]);
  expect(await archive.settings().getSetting("provider_order")).toBeNull();
  expect(isDefaultProviderOrder(await providerOrder(archive))).toBe(true);
});

test("a stored value that is not an order reads as the default", async () => {
  const archive = await scratchArchive();

  for (const stored of ["not json", "[\"codex\",\"codex\",\"openrouter\",\"retired\"]", "{\"order\":1}", "[\"claude-code\",\"codex\",\"openrouter\",\"dsh\"]", JSON.stringify(defaultProviderOrder.map((name) => ({ name, enabled: false })))]) {
    await archive.settings().setSetting("provider_order", stored);
    expect(await enabledProviders(archive)).toEqual([...defaultProviderOrder]);
  }
});

test("a saved order read under a provider it does not name keeps its order and flags, and gains that provider on at the end", () => {
  const five = ["claude-code", "codex", "openrouter", "retired", "fifth"] as const;
  const saved: { name: (typeof five)[number]; enabled: boolean }[] = [
    { name: "openrouter", enabled: true },
    { name: "claude-code", enabled: false },
    { name: "retired", enabled: true },
    { name: "codex", enabled: false },
  ];

  expect(readProviderOrder(saved, five)).toEqual([...saved, { name: "fifth", enabled: true }]);
  // A plain list of names reads every name on, and the missing provider on after it.
  expect(readProviderOrder(["retired", "codex", "claude-code", "openrouter"], five)).toEqual([
    { name: "retired", enabled: true },
    { name: "codex", enabled: true },
    { name: "claude-code", enabled: true },
    { name: "openrouter", enabled: true },
    { name: "fifth", enabled: true },
  ]);
});

test("a saved order naming an unknown provider or one provider twice is still refused", () => {
  const five = ["claude-code", "codex", "openrouter", "retired", "fifth"] as const;

  expect(readProviderOrder(["claude-code", "codex", "openrouter", "gemini"], five)).toBeNull();
  expect(readProviderOrder(["claude-code", "codex", "codex", "retired"], five)).toBeNull();
  expect(readProviderOrder([{ name: "codex", enabled: true }, { name: "codex", enabled: false }], five)).toBeNull();
  expect(readProviderOrder([{ name: "codex", enabled: true }, "openrouter"], five)).toBeNull();
  expect(readProviderOrder("claude-code,codex", five)).toBeNull();
});

test("a stored order naming only some of the providers is kept, and the rest follow it, on", async () => {
  const archive = await scratchArchive();

  await archive.settings().setSetting("provider_order", JSON.stringify([{ name: "openrouter", enabled: true }, { name: "claude-code", enabled: false }]));
  expect(await providerOrder(archive)).toEqual([
    { name: "openrouter", enabled: true },
    { name: "claude-code", enabled: false },
    ...defaultProviderOrder.filter((name) => name !== "openrouter" && name !== "claude-code").map((name) => ({ name, enabled: true })),
  ]);
  // Saving still needs every provider, once each.
  expect(() => setProviderOrder(archive, ["openrouter", "claude-code"])).toThrow("The order must name every provider once.");
});

 test("legacy names read all on; plain reorders retain off flags even in default order", async () => {
  const archive = await scratchArchive();
  await archive.settings().setSetting("provider_order", JSON.stringify(["codex", "openrouter", "registered", "claude-code"]));
  expect((await providerOrder(archive)).every(entry => entry.enabled)).toBe(true);
  await setProviderEnabled(archive, "claude-code", false);
  await setProviderOrder(archive, defaultProviderOrder);
  expect((await providerOrder(archive))[0]).toEqual({ name: "claude-code", enabled: false });
  expect(await archive.settings().getSetting("provider_order")).not.toBeNull();
  for (const name of ["codex", "openrouter", "deepseek"] as const) await setProviderEnabled(archive, name, false);
  expect(() => setProviderEnabled(archive, "registered", false)).toThrow("Not every provider can be turned off");
  expect(await enabledProviders(archive)).toEqual(["registered"]);
  expect(() => setProviderOrder(archive, defaultProviderOrder.map(name => ({ name, enabled: false })))).toThrow();
  await setProviderOrder(archive, defaultProviderOrder.map(name => ({ name, enabled: true })));
  expect(await archive.settings().getSetting("provider_order")).toBeNull();
});
