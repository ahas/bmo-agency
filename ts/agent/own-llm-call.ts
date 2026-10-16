// A request with no tools on a registration — a gate's or a chat round's — run in its
// agent's host like every other model call (`docs/agent-control.md`, 「Ownership and scope」).
//
// The caller's side keeps the part that is model discovery — the catalogue that names the model the
// request is checked against — and hands the request itself, with that catalogue and the service it is
// for, to the service binary's `own-llm-call`, which makes it and prints the reply as one JSON line, as
// it does for the gates agency's server runs itself. The command's failure words are the service's failure
// kinds, carried back and raised here as the same errors a request made in this process would raise.

import { join } from "node:path";

import { spawnAgentProcess } from "./agent-process";
import { OwnLlmError, ownLlmCatalogue, ownLlmWindow, type OwnLlmFailure, type OwnLlmOptions, type OwnLlmService } from "./own-llm";
import { codeRoot, dataRoot, deployedTree } from "@agency/support/roots";

import type { AgentReply } from "./agent-reply";
import type { GateRequest } from "./claude";

// The service as the command is handed it: its data, which is all the request needs.
export type OwnLlmCallService = OwnLlmService;

// What the command reads on its standard input: the catalogue by its models' ids, and where the key
// files stand.
export interface OwnLlmCallInput {
  service: OwnLlmCallService;
  system: string;
  prompt: string;
  model: string;
  timeoutMs: number;
  options: OwnLlmOptions;
  catalogue: string[];
  keyDir: string;
}

// What the command prints: the reply, or the failure's kind.
export type OwnLlmCallOutput =
  | { reply: AgentReply }
  | { failure: { kind: OwnLlmFailure; status: number | null; transient: boolean } };

// The service binary of the deploy tree where it holds one, the code tree's otherwise.
export function serviceBinary(): string {
  return join(deployedTree(join("bin", "bmo-agency")), "bin", "bmo-agency");
}

// How long past the request's own deadline the command is given before it is stopped: its own
// retries end at that deadline, so this is only its start and its printing.
const commandGraceMs = 5_000;

export async function ownLlmReplyInHost(
  service: OwnLlmService,
  agent: string,
  gate: GateRequest,
  model: string,
  timeoutMs: number,
  options: OwnLlmOptions = {},
  line: "gate" | "chat" = "gate",
): Promise<AgentReply> {
  const started = Date.now();
  const catalogue = await ownLlmCatalogue(service, timeoutMs);
  const left = Math.max(1, timeoutMs - (Date.now() - started));
  // The host holds no archive, so the registration's window goes with the request as this side reads it.
  const sized = { ...options, contextTokens: options.contextTokens ?? await ownLlmWindow(service) };
  const input: OwnLlmCallInput = { service, system: gate.system, prompt: gate.prompt, model, timeoutMs: left, options: sized, catalogue: catalogue.map((one) => one.id), keyDir: join(dataRoot(), "env") };
  const proc = await spawnAgentProcess(
    { args: [serviceBinary(), "own-llm-call"], cwd: codeRoot, env: process.env, stdin: JSON.stringify(input) },
    line,
    service.name,
    { agent, workKey: agent, model, effort: options.thinking ? "thinking" : null, sessionId: null },
  );
  const timer = setTimeout(() => proc.kill(), left + commandGraceMs);

  try {
    const [stdout] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
    const last = stdout.trim().split("\n").at(-1) ?? "";
    let output: OwnLlmCallOutput;

    try {
      output = JSON.parse(last) as OwnLlmCallOutput;
    } catch {
      throw new OwnLlmError(Date.now() - started >= timeoutMs ? "timeout" : "malformed", { transient: true }).named(service);
    }

    if ("failure" in output) {
      throw new OwnLlmError(output.failure.kind, { status: output.failure.status, transient: output.failure.transient }).named(service);
    }

    return output.reply;
  } finally {
    clearTimeout(timer);
  }
}
