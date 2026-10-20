import "@agency/test-support/setup";

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from "bun:test";

import { installProviderSettings } from "@agency/agent/provider-order";
import { codeRoot } from "@agency/support/roots";
import { scratchArchive } from "@agency/test-support/store";
import { processPaneFixture } from "./pane-process-fixture";

// The launch checks read the providers' switches from a settings archive made once for the file, and
// the panes a run would open stand in a fixture of the file's own.
const launchArchive = await scratchArchive();
let processFixture: ReturnType<typeof processPaneFixture>;
const previousPaneConfig = process.env.XDG_CONFIG_HOME;

beforeEach(() => installProviderSettings(launchArchive));
afterEach(() => installProviderSettings(null));
beforeAll(() => {
  processFixture = processPaneFixture();
  process.env.XDG_CONFIG_HOME = processFixture.root;
});
afterAll(async () => {
  await processFixture.close();
  if (previousPaneConfig === undefined) delete process.env.XDG_CONFIG_HOME;
  else process.env.XDG_CONFIG_HOME = previousPaneConfig;
});

const stand = "deepseek/deepseek-chat:free";

describe("what the archive keeps", () => {
  test("Codex and OpenRouter choices share a work key and reset independently", async () => {
    const archive = await scratchArchive();

    await archive.fallbacks().setCodexFallback("bmo", "gpt-5.6-terra", "high", 10);
    await archive.fallbacks().setOpenRouterFallback("bmo", stand, 11);
    await archive.fallbacks().setFallbackRouteMode("bmo", "codex", 12);

    expect(await archive.fallbacks().fallbackRoute("bmo")).toEqual({
      key: "bmo",
      // Nothing pins this key to a login, so its runs walk the account order.
      account: null,
      claudeModel: null,
      codexModel: "gpt-5.6-terra",
      codexEffort: "high",
      openrouterModel: stand,
      setAt: 12,
      routeMode: "codex",
    });

    await archive.fallbacks().resetCodexFallback("bmo", 13);
    expect(await archive.fallbacks().fallbackRoute("bmo")).toMatchObject({ codexModel: null, codexEffort: null, openrouterModel: stand, routeMode: "codex" });
    await archive.fallbacks().clearOpenRouterFallback("bmo", 14);
    expect(await archive.fallbacks().fallbackRoute("bmo")).toMatchObject({ openrouterModel: null, routeMode: "codex" });
    await archive.fallbacks().setFallbackRouteMode("bmo", "auto", 15);
    expect(await archive.fallbacks().fallbackRoute("bmo")).toBeNull();
  });

});

describe("no Claude launch line moved", () => {
  const root = codeRoot;
  const source = (path: string): string => readFileSync(join(root, path), "utf8");

  test("the chat flags remain in their fixed order", () => {
    const text = source("packages/agent/claude-code.ts");
    const chat = text.slice(text.indexOf("function chatLine"), text.indexOf("function withPrompt"));

    expect([...chat.matchAll(/"(--[a-zA-Z-]+)"/g)].map((match) => match[1])).toEqual([
      "--model", "--effort", "--system-prompt-file",
      "--setting-sources", "--strict-mcp-config",
      // agency's own server, beside the strict flag that keeps every other out.
      "--mcp-config", "--disable-slash-commands", "--no-chrome",
      // What a run keeps back, written only where it names any — a companion with a worker's hands —
      // and before the mode, since the list would swallow a word after it.
      "--disallowedTools",
      "--permission-mode", "--tools", "--allowedTools",
      // Only the resident program is handed its settings, after the list that would swallow them.
      "--settings",
    ]);
  });
});

