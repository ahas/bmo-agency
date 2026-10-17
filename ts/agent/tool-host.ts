// The launcher's side of the tool host: a run's session record built out of its line and its command, a
// token issued for it over the host's local socket before anything starts, and revoked when the run ends. A
// host that does not answer refuses the launch, naming itself and its socket.

import { storeAddress } from "@agency/archive/archive-place";
import { lineCapabilities } from "./line-declarations";
import { PLACE_VARIABLE, dataRoot, mcpHostPort, mcpHostSocket } from "@agency/support/roots";
import { isTurnVariable } from "./agent-place";

/** The variable a run's program reads its session token from, which its configuration names. */
export const MCP_TOKEN_VARIABLE = "BMO_MCP_TOKEN";

/** A session record, in the fields the tool host reads. */
export interface ToolSessionRecord {
  line: string;
  // The role of the agent the run is issued for, where it holds one.
  role?: string;
  capabilities: string[];
  checkout?: string;
  dataRoot: string;
  archive: string;
  place?: string;
  runVariables?: Record<string, string>;
  fragmentDesk?: string;
  research?: Record<string, unknown>;
  deadline?: number;
}

/** A run a record is issued for: the directory it works in, its environment and the agent it runs for. */
export interface ToolRecordRun {
  cwd: string;
  env: Record<string, string | undefined>;
  agent?: string;
}

/**
 * What the deployment adds to a record out of the run: its research settings, and what its agent is
 * given.
 */
export type ToolRecordReader = (run: ToolRecordRun) => ToolRecordReading | Promise<ToolRecordReading>;

export type ToolRecordReading = Pick<ToolSessionRecord, "research" | "role"> & {
  // The capabilities the run's settings leave out, for a run holding less than it is given.
  withheld?: readonly string[];
  // The capabilities the run holds beside its line's: the bundles its agent's role gives.
  granted?: readonly string[];
};

let reader: ToolRecordReader = () => ({});

/** The deployment's reader of a run's own settings, composed at registration. */
export function composeToolRecord(read: ToolRecordReader): void {
  reader = read;
}

/** A launch refused because the tool host did not answer. */
export class ToolHostDown extends Error {
  constructor(socket: string, why: string) {
    super(`The tool host did not answer on ${socket}, so nothing ran: ${why}`);
    this.name = "ToolHostDown";
  }
}

/** Where a namespace is served, as every line's configuration names it. */
export function toolHostUrl(namespace: string): string {
  return `http://127.0.0.1:${mcpHostPort()}/mcp/${namespace}`;
}

/** The header every configuration hands the host, with the token read from the program's environment. */
export const TOOL_HOST_AUTHORIZATION = `Bearer \${${MCP_TOKEN_VARIABLE}}`;

/**
 * How long a program waits on one call of the tool host before giving it up, in seconds: past the longest
 * call the host carries, a launching tool called in sync waiting for its run. Codex's and dsh's own
 * defaults give a call up after a minute, and a client that gave up first would lose a result the call had
 * already taken from the next turn.
 */
export const TOOL_HOST_WAIT_SECONDS = 240;

/** An HTTP server as agency's configuration files write one: its URL and the headers it is handed. */
export interface HttpServerEntry {
  url: string;
  headers: Record<string, string>;
}

/** An entry of a configuration file read as an HTTP server, or null for any other. */
export function httpServerEntry(entry: unknown): HttpServerEntry | null {
  const said = entry as { type?: unknown; url?: unknown; headers?: unknown } | null;

  if (said?.type !== "http" || typeof said.url !== "string") return null;

  const headers = Object.fromEntries(
    Object.entries(typeof said.headers === "object" && said.headers !== null ? said.headers : {}).filter((one): one is [string, string] => typeof one[1] === "string"),
  );

  return { url: said.url, headers };
}

/**
 * The variable a bearer header reads its token from, in the `${NAME}` form agency's files write it: for
 * a program that takes the variable's name rather than the header.
 */
export function bearerVariable(headers: Record<string, string>): string | null {
  return /^Bearer \$\{([A-Za-z_][A-Za-z0-9_]*)\}$/.exec(headers.Authorization ?? "")?.[1] ?? null;
}

function present(value: string | undefined): string | undefined {
  return value === undefined || value === "" ? undefined : value;
}

/**
 * A run's record: its line, its agent's role and the capabilities its line declares with what that role
 * grants, less what its own settings withhold; the directory it works in, which is the checkout the graph
 * is read from; the data root and archive it reads; the place and the run's own variables its calls read
 * their turn from; its desk and its research settings; and how long it may stand.
 */
export async function toolSessionRecord(line: string, command: ToolRecordRun, deadline?: number): Promise<ToolSessionRecord> {
  const env = command.env;
  const own = await reader(command);
  const withheld = new Set(own.withheld ?? []);
  const capabilities = [...new Set([...lineCapabilities(line), ...(own.granted ?? [])])].filter((one) => !withheld.has(one));
  const runVariables = Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => isTurnVariable(entry[0]) && entry[1] !== undefined),
  );
  const place = present(env[PLACE_VARIABLE]);
  const desk = present(env.BMO_FRAGMENT_DESK);

  return {
    line,
    ...(own.role === undefined ? {} : { role: own.role }),
    capabilities,
    checkout: command.cwd,
    dataRoot: present(env.BMO_DATA_DIR) ?? dataRoot(),
    archive: storeAddress(env, present(env.BMO_DATA_DIR) ?? dataRoot()) ?? "",
    ...(place === undefined ? {} : { place }),
    ...(Object.keys(runVariables).length === 0 ? {} : { runVariables }),
    ...(desk === undefined ? {} : { fragmentDesk: desk }),
    ...(own.research === undefined ? {} : { research: own.research }),
    ...(deadline === undefined ? {} : { deadline }),
  };
}

// How long a launch waits for a host restarting under a deployment before it refuses.
const issueWaitMs = 10_000;
const retryEveryMs = 200;

/** A token issued for a record, waited for up to ten seconds of the host not answering. */
export async function issueToolToken(record: ToolSessionRecord, options: { socket?: string; waitMs?: number } = {}): Promise<string> {
  const socket = options.socket ?? mcpHostSocket();
  const deadline = Date.now() + (options.waitMs ?? issueWaitMs);

  for (;;) {
    let why: string;

    try {
      const answer = await fetch("http://localhost/sessions", {
        unix: socket,
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(record),
      });
      const said = (await answer.json().catch(async () => null)) as { token?: unknown; detail?: unknown } | null;

      if (answer.ok && typeof said?.token === "string") return said.token;

      // A host that answered and refused the record is no host restarting: it is said at once.
      throw new ToolHostDown(socket, `${answer.status} ${typeof said?.detail === "string" ? said.detail : ""}`.trim());
    } catch (error) {
      if (error instanceof ToolHostDown) throw error;
      why = error instanceof Error ? error.message : String(error);
    }

    if (Date.now() >= deadline) throw new ToolHostDown(socket, why);

    await Bun.sleep(retryEveryMs);
  }
}

/**
 * What a namespace's owner says of it beside its tools, as the tool host hands it on; null where the host
 * does not answer or the namespace says nothing.
 */
export async function namespaceFacts(namespace: string, options: { socket?: string } = {}): Promise<Record<string, unknown> | null> {
  try {
    const answer = await fetch(`http://localhost/facts/${encodeURIComponent(namespace)}`, { unix: options.socket ?? mcpHostSocket() });
    const said: unknown = answer.ok ? await answer.json() : null;

    return typeof said === "object" && said !== null && !Array.isArray(said) ? (said as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

/** A token taken back; a host that does not answer has lost it already, so this never throws. */
export async function revokeToolToken(token: string, options: { socket?: string } = {}): Promise<void> {
  try {
    await fetch(`http://localhost/sessions/${encodeURIComponent(token)}`, { unix: options.socket ?? mcpHostSocket(), method: "DELETE" });
  } catch {
    // The record lapses with its deadline, or stands until the host is next told.
  }
}

/** A token for a run of `line`, or null where the run needs none: one whose record holds no capability. */
export async function toolTokenFor(
  line: string,
  command: ToolRecordRun,
  options: { deadline?: number; socket?: string } = {},
): Promise<string | null> {
  const record = await toolSessionRecord(line, command, options.deadline);

  return record.capabilities.length === 0 ? null : await issueToolToken(record, { socket: options.socket });
}

/** What a tool's entry answers a call with, in the fields a caller reads. */
export interface ToolAnswer {
  content?: { type: string; text?: string }[];
  isError?: boolean;
  structuredContent?: unknown;
}

/** An answer's text, its pieces joined a line each. */
export function toolAnswerText(answer: ToolAnswer): string {
  return (answer.content ?? []).map((piece) => piece.text ?? JSON.stringify(piece)).join("\n");
}

/**
 * A tool's entry called for the system on the host's control socket, under the record `bmo tool call`
 * writes: a shell's line holding the entry's own capabilities, privileged, over this process's data root
 * and archive. An entry the host does not serve, or a host that does not answer, throws; a refusal or a
 * failure of the entry is its answer, marked as an error. A call aborted before it answers hangs up on the
 * host, which ends every process the call started.
 */
export async function callToolEntry(
  tool: string,
  args: Record<string, unknown>,
  options: { socket?: string; signal?: AbortSignal } = {},
): Promise<ToolAnswer> {
  const socket = options.socket ?? mcpHostSocket();
  const asked = async (path: string, body?: unknown): Promise<Response> => {
    try {
      return await fetch(`http://localhost${path}`, {
        unix: socket,
        signal: options.signal,
        ...(body === undefined ? {} : { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) }),
      });
    } catch (error) {
      if (options.signal?.aborted === true) throw error;

      throw new Error(`the tool host at ${socket} did not answer: ${error instanceof Error ? error.message : String(error)}`);
    }
  };
  const entry = await asked(`/catalogue/${tool}`);

  if (!entry.ok) throw new Error(`the tool host serves no ${tool}`);

  const { capabilities } = (await entry.json()) as { capabilities?: string[] };
  const session: ToolSessionRecord = {
    line: "shell",
    capabilities: capabilities ?? [],
    dataRoot: dataRoot(),
    archive: storeAddress() ?? "",
    runVariables: { BMO_TURN_PRIVILEGED: "1" },
  };

  return (await (await asked("/calls", { session, tool, arguments: args })).json()) as ToolAnswer;
}
