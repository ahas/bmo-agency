import "@agency/test-support/setup";

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "bun:test";

import { scratchDir } from "@agency/test-support/scratch";

import { claudeConfigFile, trustFolder } from "@agency/agent/claude-trust";

// Claude Code's folder trust as its configuration file keeps it: which file a run reads, and the
// folder written as trusted there without disturbing anything else the file holds.

const root = scratchDir("bmo-claude-trust-");

afterAll(() => rmSync(root, { recursive: true, force: true }));

let dirs = 0;

function configDir(): string {
  const dir = join(root, `config-${++dirs}`);

  mkdirSync(dir, { recursive: true });

  return dir;
}

describe("the configuration file a run reads its trust from", () => {
  test("is the one in the configuration directory it is handed, and the home's own otherwise", () => {
    expect(claudeConfigFile({ CLAUDE_CONFIG_DIR: "/logins/main" })).toBe("/logins/main/.claude.json");
    expect(claudeConfigFile({})).toBe(join(process.env.HOME ?? "", ".claude.json"));
    expect(claudeConfigFile({ CLAUDE_CONFIG_DIR: "" })).toBe(join(process.env.HOME ?? "", ".claude.json"));
  });
});

describe("trusting a folder", () => {
  test("writes nothing where the folder is already trusted", () => {
    const file = join(configDir(), ".claude.json");
    const text = JSON.stringify({ projects: { "/work/a": { hasTrustDialogAccepted: true } } });

    writeFileSync(file, text);

    expect(trustFolder(file, "/work/a")).toBe("trusted");
    expect(readFileSync(file, "utf8")).toBe(text);
  });

  test("leaves alone a directory that is not there, a file that is not the CLI's, and a file held by its lock", () => {
    const missing = join(root, "no-such-login", ".claude.json");
    const broken = join(configDir(), ".claude.json");
    const locked = join(configDir(), ".claude.json");

    writeFileSync(broken, "{ half");
    writeFileSync(locked, "{}");
    mkdirSync(`${locked}.lock`);

    expect(trustFolder(missing, "/work/a")).toBe("no-config-dir");
    expect(existsSync(join(root, "no-such-login"))).toBe(false);
    expect(trustFolder(broken, "/work/a")).toBe("unreadable");
    expect(readFileSync(broken, "utf8")).toBe("{ half");
    expect(trustFolder(locked, "/work/a", { lockWaitMs: 0 })).toBe("busy");
    expect(readFileSync(locked, "utf8")).toBe("{}");
    expect(() => trustFolder(broken, "work/a")).toThrow();
  });
});

