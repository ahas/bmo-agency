import "@agency/test-support/setup";

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { Archive } from "@agency/archive/archive";
import { installPlacer, spawnAgentProcess } from "@agency/agent/agent-process";
import { installProviderSettings } from "@agency/agent/provider-order";
import { standResident } from "@agency/agent/resident-pane";
import { readProgram } from "@agency/agent/resident-program";
import { MCP_TOKEN_VARIABLE, ToolHostDown, composeToolRecord, issueToolToken } from "@agency/agent/tool-host";
import declaredRoles from "@agency/policy/roles.toml";
import { crateBinary } from "@agency/test-support/cargo";
import { scratchDir } from "@agency/test-support/scratch";

import type { Placer } from "@agency/agent/pane-process";
import type { ResidentProgram } from "@agency/agent/resident-program";
import { codeRoot } from "@agency/support/roots";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

// A line standing on the hosts (docs/agent-control.md, 「The host」), as a launcher
// meets them: the data root's agent host, the real binary on a scratch root, and a stand-in for the
// tool host's issuing socket that hands out tokens and notes what it was told. The worker the runs are
// issued for holds the worker role, whose bundles are all the worker line's runs are given.

const root = codeRoot;
const data = join(scratchDir("bmo-root-host-"), "data");
const sockets = scratchDir("bmo-root-host-sockets-");
const told: { issued: Record<string, unknown>[]; revoked: string[] } = { issued: [], revoked: [] };
const previous = { data: process.env.BMO_DATA_DIR, socket: process.env.BMO_MCP_HOST_SOCKET };
const archive = await scratchArchive();

// The bundles a role gives, as the roles' policy names them.
function bundlesOf(role: string | null): readonly string[] {
  return (declaredRoles.roles as { name: string; bundles: string[] }[]).find((one) => one.name === role)?.bundles ?? [];
}
let toolHost: ReturnType<typeof Bun.serve>;
let agentHost: ReturnType<typeof Bun.spawn>;

beforeAll(async () => {
  mkdirSync(join(data, "executions"), { recursive: true });
  process.env.BMO_DATA_DIR = data;
  process.env.BMO_MCP_HOST_SOCKET = join(sockets, "mcp-host.sock");
  toolHost = Bun.serve({
    unix: process.env.BMO_MCP_HOST_SOCKET,
    async fetch(request) {
      const path = new URL(request.url).pathname;

      if (request.method === "POST" && path === "/sessions") {
        told.issued.push((await request.json()) as Record<string, unknown>);
        return Response.json({ token: `token-${told.issued.length}` });
      }

      if (request.method === "DELETE") {
        told.revoked.push(decodeURIComponent(path.slice("/sessions/".length)));
        return new Response(null, { status: 204 });
      }

      return new Response(null, { status: 404 });
    },
  });
  agentHost = Bun.spawn([crateBinary(root, "agent-host"), "serve", "--data-root", data, "--code-root", data, "--poll-ms", "10"], { stdout: "ignore", stderr: "ignore" });
  installProviderSettings(archive);
  composeToolRecord((run) => ({ granted: bundlesOf(run.agent === "worker" ? "worker" : null) }));

  for (let i = 0; i < 200 && !existsSync(join(data, "executions", "agent-host", "claim")); i++) await Bun.sleep(25);
}, 120_000);

afterAll(async () => {
  agentHost.kill();
  await agentHost.exited;
  toolHost.stop(true);
  installProviderSettings(null);
  installPlacer(null);
  composeToolRecord(() => ({}));
  process.env.BMO_DATA_DIR = previous.data;
  if (previous.socket === undefined) delete process.env.BMO_MCP_HOST_SOCKET; else process.env.BMO_MCP_HOST_SOCKET = previous.socket;
});

describe("a structured run on a line standing on the hosts", () => {
  test("is issued a token, runs under the data root's agent host, shows in its agent's view and gives the token back", async () => {
    const cwd = scratchDir("bmo-root-host-checkout-");
    const proc = await spawnAgentProcess(
      { args: [process.execPath, "-e", `process.stdout.write(process.env.${MCP_TOKEN_VARIABLE} ?? "none")`], cwd, env: { BMO_TURN_CHAT: "-100" }, stdin: null },
      "worker",
      "claude-code",
      { agent: "worker" },
    );
    const [stdout, exit] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
    const token = stdout;

    expect(exit).toBe(0);
    expect(token).toStartWith("token-");
    expect(told.issued.at(-1)).toMatchObject({ line: "worker", capabilities: ["graph.read", "queue.read", "web.read"], checkout: cwd, runVariables: { BMO_TURN_CHAT: "-100" } });

    for (let i = 0; i < 100 && !told.revoked.includes(token); i++) await Bun.sleep(20);
    expect(told.revoked).toContain(token);

    // The host wrote its beginning and, once the collector had it, its rest; the collector its end.
    const view = join(data, "executions", "agent-host", "views", "worker");
    let lines: string[] = [];

    for (let i = 0; i < 100 && !lines.some((line) => line.startsWith("resting")); i++) {
      await Bun.sleep(20);
      lines = existsSync(view) ? readFileSync(view, "utf8").trim().split("\n") : [];
    }

    expect(lines[0]).toMatch(/^── \d\d:\d\d worker · claude-code · attempt 1 \(.+\) started$/);
    expect(lines.some((line) => /^── \d\d:\d\d worker · claude-code · attempt 1 ended$/.test(line))).toBe(true);
    expect(lines.at(-1)).toStartWith("resting");
  }, 60_000);

  test("is refused, naming the tool host, before anything is placed when the tool host does not answer", async () => {
    let placed = 0;
    const counting: Placer = async () => {
      placed += 1;
      return { ok: false, error: { code: "unused", message: "unused" } };
    };
    const socket = process.env.BMO_MCP_HOST_SOCKET;

    installPlacer(counting);
    process.env.BMO_MCP_HOST_SOCKET = join(sockets, "nobody.sock");

    try {
      const proc = await spawnAgentProcess({ args: [process.execPath, "-e", "0"], cwd: data, env: {}, stdin: null }, "worker", "claude-code", { agent: "worker" });

      expect(proc.exited).rejects.toThrow(/The tool host did not answer on .*nobody\.sock/);
      await proc.exited.catch(() => {});
      expect(placed).toBe(0);
    } finally {
      installPlacer(null);
      process.env.BMO_MCP_HOST_SOCKET = socket;
    }
  }, 30_000);
});

describe("a resident program on a line standing on the hosts", () => {
  const cwd = scratchDir("bmo-root-host-resident-");
  const program = (identity: string): ResidentProgram => ({ identity, command: { args: ["sleep", "60"], cwd, env: { PATH: "/usr/bin" } }, restart: null });

  test("is not written when the tool host does not answer", async () => {
    const place = join(scratchDir("bmo-root-host-place-"), "worker");
    const socket = process.env.BMO_MCP_HOST_SOCKET;

    mkdirSync(place, { recursive: true });
    process.env.BMO_MCP_HOST_SOCKET = join(sockets, "nobody.sock");

    try {
      const stood = await standResident({ place, home: { agent: "worker", label: "worker", head: { command: [], cwd, env: {} } }, program: program("one"), line: "worker" });

      expect(stood.ok).toBe(false);
      expect(!stood.ok && stood.error.code).toBe("tool-host-down");
      expect(readProgram(place)).toBeNull();
    } finally {
      process.env.BMO_MCP_HOST_SOCKET = socket;
    }
  }, 30_000);
});

test("a host that answers and refuses the record is said at once, not waited for", async () => {
  const refusing = Bun.serve({ unix: join(sockets, "refusing.sock"), fetch: () => Response.json({ detail: "unknown field" }, { status: 400 }) });

  try {
    const began = Date.now();

    expect(issueToolToken({ line: "worker", capabilities: [], dataRoot: data, archive: join(data, "bmo.db") }, { socket: join(sockets, "refusing.sock") })).rejects.toBeInstanceOf(ToolHostDown);
    await issueToolToken({ line: "worker", capabilities: [], dataRoot: data, archive: join(data, "bmo.db") }, { socket: join(sockets, "refusing.sock") }).catch(() => {});
    expect(Date.now() - began).toBeLessThan(2_000);
  } finally {
    refusing.stop(true);
  }
});
