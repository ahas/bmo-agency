import declaredClaudeCode from "@agency/policy/providers/claude-code.toml";
import { configuredProviderEnabled } from "./model-settings";
import { spawnAgentProcess } from "./agent-process";
import { ExecutionStopped, wasStopped } from "./pane-process";
import { serviceBinary } from "./own-llm-call";
import { ClaudeLimitError, ClaudeOutageError } from "./claude-failure";
import { RunDied } from "./doctor";
import { failureText } from "./failure-text";
import { keepalivePrompt } from "./harness";
import { dataRoot, sourceRoot } from "@agency/support/roots";
import { claudeSpendOf } from "./spend";
import { workflowTask } from "./workflow-log";

import type { AgentSessionReply, SessionUsage, UsageWindowReading } from "./agent-reply";
import type { LiveTask, RunWorkflow } from "./finished";
import type { ClaudeModelUsage, ModelSpend } from "./spend";

interface ClaudeTokenCounts {
  input_tokens?: number;
  cache_creation_input_tokens?: number;
  cache_read_input_tokens?: number;
}

interface ClaudeUsage extends ClaudeTokenCounts {
  output_tokens?: number;
  iterations?: ClaudeTokenCounts[];
}

// What the CLI hands back as one object at the end of a run. A line that asks for json alone
// gets exactly this, so a runner reading its result rather than a stream reads what a run cost
// and what it ended carrying out of the same object the answer is in.
export interface ClaudeJsonResult {
  result?: string;
  session_id?: string;
  is_error?: boolean;
  usage?: ClaudeUsage;
  modelUsage?: Record<string, ClaudeModelUsage>;
}

interface ClaudeRateLimitWindow {
  utilization?: number;
  resetsAt?: number;
}

interface ClaudeRateLimitInfo {
  status?: string;
  resetsAt?: number;
  unifiedWindows?: Record<string, ClaudeRateLimitWindow | undefined>;
}

interface ClaudeBackgroundTask {
  task_id?: string;
  description?: string;
}

interface ClaudeStreamEvent extends ClaudeJsonResult {
  subtype?: string;
  error_status?: number;
  type?: string;
  rate_limit_info?: ClaudeRateLimitInfo;
  tasks?: ClaudeBackgroundTask[];
  // What the three task events carry about one background task: which call opened it, what kind
  // of task it is, what it was launched as, and how it ended.
  task_id?: string;
  tool_use_id?: string;
  task_type?: string;
  workflow_name?: string;
  description?: string;
  status?: string;
  message?: { content?: { type?: string; text?: string }[] };
}

interface ClaudeStreamOutput {
  result: ClaudeJsonResult | null;
  rateLimit: ClaudeRateLimitInfo | null;
  // The CLI's own retries of a failing API call, with the last status it saw.
  retries: number;
  lastErrorStatus: number | null;
  // The background tasks still live at the moment the result was produced.
  liveTasks: LiveTask[];
  // The Claude Workflows the run launched, and what its stream said became of each.
  workflows: RunWorkflow[];
}

const sourceDir = sourceRoot();

// What every line sets on the process it starts, named apart from the environment it is set on.
// A command a host runs is handed the whole environment with these on top; a process that is a
// pane's own is handed these by name, because the environment of whoever launched it is not
// something to pass on — a launcher that is itself inside a Claude session carries that session's
// markers, and a CLI that inherits them turns its transcript saving off.
//
// Which they are, and why, is Claude Code's own declaration in the shared policy, read by every side
// that starts one of its lines (docs/launch-lines.md).
export const launchVariables: Readonly<Record<string, string>> = (declaredClaudeCode as { launch: { variables: Record<string, string> } }).launch.variables;

export function launchEnv(): Record<string, string | undefined> {
  return { ...process.env, ...launchVariables };
}

// What a gate run is given: the rulebook and the fixed instructions as the system prompt, which
// is the same from call to call and so stays in the prompt cache, and the data of this one call
// as the prompt.
export interface GateRequest {
  system: string;
  prompt: string;
  // False for a judgement that needs no thinking: the run is started with thinking off, and
  // answers in the time the model takes to write the answer alone.
  thinking?: boolean;
  // Which room's message this judgement is about, for the live view; a run that is about no
  // message — a keepalive, the console's own try — carries neither.
  chatId?: number | null;
  messageId?: number | null;
  // How this request is cut where its road sends it back as too large. The gate that composed the
  // request supplies it; a request without one is never cut.
  split?: GateSplit;
  // True where the caller bounds how many of its own requests are out at once: the request stands
  // beside the gate's other runs rather than waiting its turn behind them (`docs/gates.md`).
  alongside?: boolean;
}

// A gate's own way of cutting its request and putting the answers back together. The parts are
// consecutive and each is judged as a gate run of its own, so a part the road also finds too large
// is cut again, and a part the road cannot answer for another reason walks the gate's fallback
// policy alone.
export interface GateSplit {
  // The request as consecutive smaller parts, or null where it cannot be made smaller. A part the
  // gate already has the answer to — text it would not ask about — is given as that answer.
  halve(): Array<GateRequest | { answer: string }> | null;
  // The answer to the whole out of the answers to its parts, in their order; null stands for a
  // part no road answered, and null back means the whole has no answer.
  join(answers: Array<string | null>): string | null;
}

// A gate's prefix as the keepalive needs it: the model and the system prompt it is run
// with, the same for every call, so the prefix a ping warms is the one the judgement runs on.
export interface GateWarmLine {
  name: string;
  model: string;
  system: () => string;
  // Whether a run of this gate thinks, where the gate itself has an answer to that. Left out, the
  // road decides. A gate that says so here is answered the same way wherever it is put to a
  // question, and nothing that runs gates has to know which gate it is holding.
  thinking?: boolean;
}

// Every line of a stream that parses as JSON. A stream read from a run that was killed ends
// mid-line, and that last broken line is simply not an event.
function streamEvents(stdout: string): ClaudeStreamEvent[] {
  const events: ClaudeStreamEvent[] = [];

  for (const line of stdout.split("\n")) {
    const trimmed = line.trim();

    if (!trimmed.startsWith("{")) {
      continue;
    }

    try {
      events.push(JSON.parse(trimmed) as ClaudeStreamEvent);
    } catch {
      continue;
    }
  }

  return events;
}

// What the run wrote, in the order it wrote it, taken from the stream rather than from the
// result frame. A run that was killed never reaches its result frame, so this is the only
// place its work survives.
export function assistantTexts(stdout: string): string[] {
  return streamEvents(stdout)
    .filter((event) => event.type === "assistant")
    .flatMap((event) => event.message?.content ?? [])
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text as string);
}

function parseStream(stdout: string): ClaudeStreamOutput {
  let result: ClaudeJsonResult | null = null;
  let rateLimit: ClaudeRateLimitInfo | null = null;
  let retries = 0;
  let lastErrorStatus: number | null = null;
  // The CLI reports its whole set of live background tasks whenever the set changes, so the
  // last report before the result frame is what was still running when the run ended.
  let running: LiveTask[] = [];
  let liveTasks: LiveTask[] = [];
  // A Claude Workflow is reported through the task events rather than as a background task, so it is
  // gathered on its own: kept under the call that launched it, since an ending may name only
  // the task, and read across the whole stream rather than cut off at the result frame — an
  // ending that arrives after that frame is the CLI saying what it did to a Claude Workflow as the run
  // went down, which is the very thing worth knowing.
  const workflows = new Map<string, RunWorkflow>();
  const workflowCalls = new Map<string, string>();

  for (const event of streamEvents(stdout)) {
    if (event.type === "result") {
      result = event;
      liveTasks = running;
    } else if (event.type === "system" && event.subtype === "task_started") {
      // Only a task whose own start said it was a Claude Workflow. The CLI runs its other background
      // tasks through these same three events, and those are somebody else's business.
      if (event.task_type === workflowTask && event.tool_use_id !== undefined) {
        if (event.task_id !== undefined) {
          workflowCalls.set(event.task_id, event.tool_use_id);
        }

        workflows.set(event.tool_use_id, {
          call: event.tool_use_id,
          name: event.workflow_name ?? event.description ?? "",
          status: null,
        });
      }
    } else if (event.type === "system" && event.subtype === "task_notification") {
      const call = event.tool_use_id ?? (event.task_id === undefined ? undefined : workflowCalls.get(event.task_id));
      const flow = call === undefined ? undefined : workflows.get(call);

      if (flow !== undefined && event.status !== undefined) {
        flow.status = event.status;
      }
    } else if (event.type === "system" && event.subtype === "background_tasks_changed") {
      running = (event.tasks ?? []).map((task) => ({
        id: task.task_id ?? "",
        description: task.description ?? "",
      }));
    } else if (event.type === "rate_limit_event" && event.rate_limit_info !== undefined) {
      rateLimit = event.rate_limit_info;
    } else if (event.type === "system" && event.subtype === "api_retry") {
      retries += 1;
      lastErrorStatus = typeof event.error_status === "number" ? event.error_status : lastErrorStatus;
    }
  }

  return { result, rateLimit, retries, lastErrorStatus, liveTasks, workflows: [...workflows.values()] };
}

const limitWordings: RegExp[] = [
  /claude (?:ai )?usage limit reached/i,
  /usage limit reached\|\d+/i,
  /you'?ve (?:hit|reached) your [^\n]{0,40}limit/i,
];

export function mentionsLimit(text: string): boolean {
  return limitWordings.some((pattern) => pattern.test(text));
}

export function limitResetFromText(text: string): number | null {
  const match = /limit reached\|(\d{9,11})/i.exec(text);
  const epoch = match?.[1] === undefined ? null : Number.parseInt(match[1], 10);

  return epoch === null || Number.isNaN(epoch) ? null : epoch;
}

function sessionUsageOf(rateLimit: ClaudeRateLimitInfo | null): SessionUsage | null {
  const window = rateLimit?.unifiedWindows?.five_hour;

  if (window?.utilization === undefined || window.resetsAt === undefined) {
    return null;
  }

  const weekly = rateLimit?.unifiedWindows?.seven_day;

  return {
    utilization: window.utilization,
    resetsAt: window.resetsAt,
    weeklyUtilization: weekly?.utilization ?? null,
    weeklyResetsAt: weekly?.resetsAt ?? null,
  };
}

// `ownRun` reads what the run spent off its own usage, under the one model it names, rather than off
// the model-by-model usage, which a resumed session carries as its running total (docs/own-llm.md,
// 「How requests go」).
export function readClaudeOutput(stdout: string, stderr: string, exitCode: number, launched: string | null = null, options: { ownRun?: boolean } = {}): AgentSessionReply {
  const { result, rateLimit, retries, lastErrorStatus, liveTasks, workflows } = parseStream(stdout);

  if (
    exitCode !== 0 ||
    result === null ||
    result.is_error === true ||
    result.result === undefined ||
    result.session_id === undefined
  ) {
    const errorText = `${stderr}\n${result?.result ?? ""}`;

    if (rateLimit?.status === "rejected" || mentionsLimit(errorText)) {
      throw new ClaudeLimitError(
        rateLimit?.resetsAt ??
          rateLimit?.unifiedWindows?.five_hour?.resetsAt ??
          limitResetFromText(errorText),
      );
    }

    // A run that only ever retried its API call and then stopped met an outage, not a fault.
    if (result === null && retries > 0) {
      throw new ClaudeOutageError(lastErrorStatus, retries);
    }

    // What the reader of a run knows about how it died, carried with the sentence rather than
    // only inside it: whoever tells the owner why the run broke reads these rather than taking the
    // message back apart (`ts/agent/doctor.ts`).
    // A run that wrote its result before it exited says why in that result, where its standard error
    // may hold no more than the CLI's own warnings.
    if (exitCode !== 0) {
      const said = failureText(result?.result === undefined || result.result.trim() === "" ? stderr : `${stderr}\n${result.result}`);

      throw new RunDied({
        ending: { kind: "exit", code: exitCode },
        said,
        message: `claude exited with code ${exitCode}: ${said}`,
      });
    }

    throw new RunDied({
      ending: { kind: "error-result" },
      said: failureText(stdout),
      message: `claude returned an error result: ${failureText(stdout)}`,
    });
  }

  return {
    mode: "claude-code",
    model: result.modelUsage === undefined ? null : (Object.keys(result.modelUsage)[0] ?? null),
    effort: null,
    text: result.result,
    sessionId: result.session_id,
    contextRatio: contextRatioOf(result, launched),
    contextTokens: contextTokensOf(result),
    sessionUsage: sessionUsageOf(rateLimit),
    windows: windowsOf(rateLimit),
    spend: options.ownRun === true ? ownRunSpend(result) : claudeSpendOf(result.modelUsage),
    liveTasks,
    workflows,
  };
}

// What one run spent by its own usage, under the one model the result names; the model-by-model
// figures where it names more than one, or no usage of its own.
function ownRunSpend(result: ClaudeJsonResult): ModelSpend[] {
  const models = Object.keys(result.modelUsage ?? {});
  const usage = result.usage;

  if (models.length !== 1 || usage === undefined) return claudeSpendOf(result.modelUsage);

  return claudeSpendOf({
    [models[0]!]: {
      inputTokens: usage.input_tokens,
      outputTokens: usage.output_tokens,
      cacheReadInputTokens: usage.cache_read_input_tokens,
      cacheCreationInputTokens: usage.cache_creation_input_tokens,
    },
  });
}

// Every window the answer carried, under the name the API gave it. A window without a figure
// says nothing and is left out rather than recorded as zero.
export function windowsOf(rateLimit: ClaudeRateLimitInfo | null): UsageWindowReading[] {
  const windows = rateLimit?.unifiedWindows ?? {};

  return Object.entries(windows)
    .filter(([, window]) => typeof window?.utilization === "number")
    .map(([name, window]) => ({
      name,
      utilization: window?.utilization ?? 0,
      resetsAt: window?.resetsAt ?? null,
    }));
}

// What the turn's last request carried: everything the model was sent, whether it came out of
// the cache or was written into it. That whole sum is what a resume writes again once the
// prefix behind it has been invalidated.
export function contextTokensOf(parsed: ClaudeJsonResult): number | null {
  const iterations = parsed.usage?.iterations ?? [];
  const counts = iterations.length > 0 ? iterations[iterations.length - 1] : parsed.usage;

  if (counts === undefined) {
    return null;
  }

  const tokens =
    (counts.input_tokens ?? 0) +
    (counts.cache_creation_input_tokens ?? 0) +
    (counts.cache_read_input_tokens ?? 0);

  return tokens > 0 ? tokens : null;
}

// How full the context is: what the turn carried against the window of the model it was launched
// on. A run reports a window for every model it spent on, the CLI's own small housekeeping model
// among them, so the launched model's own window is the one asked for and the widest reported is
// what answers where the caller did not say which model it launched.
function contextRatioOf(parsed: ClaudeJsonResult, launched: string | null): number | null {
  const tokens = contextTokensOf(parsed) ?? 0;
  const windows = Object.values(parsed.modelUsage ?? {})
    .map((usage) => usage.contextWindow ?? 0)
    .filter((window) => window > 0);
  const contextWindow = (launched === null ? undefined : parsed.modelUsage?.[launched]?.contextWindow) ?? Math.max(0, ...windows);

  if (tokens <= 0 || contextWindow === undefined || contextWindow <= 0) {
    return null;
  }

  return tokens / contextWindow;
}

// How long a step has taken since it began, in whole milliseconds.
function stopwatch(): () => number {
  const started = performance.now();

  return () => Math.round(performance.now() - started);
}

// What a gate's own road answered, as the gate unit reads it: the reply where its road is an own
// service that answered, whether the request was too large for that road, the service's labels, and the
// road it stands on — an own service, or a provider and the Claude model it names.
export interface PrimaryGateAnswer {
  reply: { text: string; model?: string | null; mode: string } | null;
  tooLarge: boolean;
  service: { label: string; shortLabel: string } | null;
  route: { ownService: boolean; provider?: string; claudeModel?: string } | null;
}

/**
 * A gate's road, its wait and its own-service ask are the gate unit's (docs/api.md, 「Hooks」): the
 * consumer that owns the gates registers them here. Agency runs a gate's Claude road and reads nothing
 * of its options.
 */
export interface GateRouteHook {
  // Whether the gate stands on Laya, where no model is run for it.
  onLaya(stage: string): Promise<boolean>;
  // The gate's own row's time limit, or null where it holds none and the gate is not run.
  timeoutMs(stage: string): Promise<number | null>;
  // Whether the gate's primary road holds no session, so a keepalive ping has nothing to warm.
  statelessPrimary(stage: string, claudeModel: string): Promise<boolean>;
  // The gate asked on its own road where that road is an own service.
  primary(stage: string, claudeModel: string, request: GateRequest, timeoutMs: number): Promise<PrimaryGateAnswer>;
}

// Where no consumer registered: no gate has a time limit, so none is run and each has no answer.
const noGateRoute: GateRouteHook = {
  onLaya: async () => false,
  timeoutMs: async () => null,
  statelessPrimary: async () => false,
  primary: async () => ({ reply: null, tooLarge: false, service: null, route: null }),
};

let gateRoute: GateRouteHook = noGateRoute;

/** The consumer's gate route registered, or taken away where none is given. */
export function registerGateRoute(hook: GateRouteHook | null): void {
  gateRoute = hook ?? noGateRoute;
}

// A gate run: one judgement on the gate's one road — an own service asked from here, or the gate's
// pool on Claude Code asked through the service binary, where the request's system prompt is the
// program's whole prompt and no tool, MCP server or instruction file rides along. stage names the
// caller in the log line. One gate's runs take turns unless the request stands
// alongside the others.
const gateTurns = new Map<string, Promise<unknown>>();

export async function runGateModel(
  model: string,
  stage: string,
  request: GateRequest,
): Promise<string | null> {
  if (request.alongside === true) {
    return runGateTurn(model, stage, request);
  }

  const turn = (gateTurns.get(stage) ?? Promise.resolve()).catch(async () => undefined)
    .then(() => runGateTurn(model, stage, request));
  gateTurns.set(stage, turn);
  void turn.finally(() => { if (gateTurns.get(stage) === turn) gateTurns.delete(stage); }).catch(async () => undefined);
  return turn;
}

async function runGateTurn(
  model: string,
  stage: string,
  request: GateRequest,
): Promise<string | null> {
  // A gate on Laya has no model to run: whatever asked for one — a warm line, a test from the
  // console — is told so and given no answer, never a run on another road in Laya's place.
  if (await gateRoute.onLaya(stage)) {
    console.error(`${stage}: this gate stands on Laya, so no model is run for it.`);

    return null;
  }

  // What this gate waits is its own row's time limit, whoever asked for the run. A gate whose row
  // holds none is not run, and ends as a gate with no answer: the turn that asked goes on its own way
  // rather than breaking (docs/own-llm.md, 「Per-gate time limit」).
  const timeoutMs = await gateRoute.timeoutMs(stage);

  if (timeoutMs === null) {
    console.error(`${stage} has no time limit to run on; the gate is not run and has no answer`);

    return null;
  }

  return spawnGateModel(model, stage, request, timeoutMs);
}

async function spawnGateModel(
  model: string,
  stage: string,
  request: GateRequest,
  timeoutMs: number,
): Promise<string | null> {
  // A ping warms a prefix, and a stateless HTTP turn has none to warm: the gate whose primary
  // road holds no session is asked nothing at all rather than asked something it would answer for
  // nothing.
  if (request.prompt === keepalivePrompt && await gateRoute.statelessPrimary(stage, model)) {
    return null;
  }

  const routeTime = stopwatch();
  // Somebody stopping this gate's own run is not this gate having no answer: the throw goes on to
  // the caller, and nothing below runs — no second road is walked, because a stop is not carried
  // to another provider (docs/agent-control.md 「Stopping」).
  const primary = await gateRoute.primary(stage, model, request, timeoutMs);
  const parts = primary.tooLarge ? request.split?.halve() ?? null : null;

  // Cut, each part is a gate run of its own on this same line, with the whole ceiling and the
  // whole route policy; this stays inside the queued turn rather than queueing behind itself.
  if (primary.tooLarge && request.split !== undefined) {
    const service = primary.service?.shortLabel;

    console.error(parts === null
      ? `${stage}: ${service} request-too-large and cannot be cut further; this gate has no answer`
      : `${stage}: ${service} request-too-large; cut into ${parts.length} parts`);
  }

  if (parts !== null && request.split !== undefined) {
    const answers: Array<string | null> = [];

    for (const part of parts) {
      answers.push("answer" in part ? part.answer : await spawnGateModel(model, stage, part, timeoutMs));
    }

    return request.split.join(answers);
  }

  if (primary.reply !== null) {
    console.log(`${stage}: answered on ${primary.reply.model ?? primary.reply.mode} through ${primary.service?.label ?? primary.reply.mode}`);

    return primary.reply.text;
  }

  // The road was an own service and it did not answer. That is the end of this gate: nothing
  // stands behind it, and the caller does with a missing judgement whatever it already does.
  if (primary.route?.ownService === true) {
    return null;
  }

  model = primary.route?.claudeModel ?? model;

  // A gate whose road is a stand-in provider is not run in this process: the stand-in runs are agency
  // server's, and the gate has no answer here.
  const standInProvider = primary.route?.provider;

  if (standInProvider !== undefined && standInProvider !== "claude-code") {
    return null;
  }

  if (!await configuredProviderEnabled("claude-code")) {
    return null;
  }
  const claudeTimeoutMs = timeoutMs - routeTime();

  if (claudeTimeoutMs <= 0) {
    return null;
  }

  return await claudeGateRunner(stage, model, request, claudeTimeoutMs);
}

// Where a gate's judgement on its Claude road is asked: the service binary's gate run, or a check's
// stand-in for it.
type ClaudeGateRunner = (stage: string, model: string, request: GateRequest, timeoutMs: number) => Promise<string | null>;

let claudeGateRunner: ClaudeGateRunner = claudeGateReply;

export function setClaudeGateRunner(runner: ClaudeGateRunner | null): void {
  claudeGateRunner = runner ?? claudeGateReply;
}

// What the service binary's gate run prints: the judgement or none, a stop, or why it raised.
type GateRunOutput = { answer: string | null } | { stopped: string } | { fault: string };

// How long past the gate's wait the binary is given before it is stopped: the judgement ends at that
// wait, so this is only its start, the clearing of the program it was put into, and its printing.
const gateRunGraceMs = 15_000;

// The judgement on the gate's Claude road, put into a program of the gate's pool by the service
// binary's gate run, on the logins the gate's key is offered in turn (docs/gates.md, 「A gate on
// Claude Code」). A stop is raised to the caller as a stop anywhere is; anything else that did not
// answer is this gate having no answer.
async function claudeGateReply(stage: string, model: string, request: GateRequest, timeoutMs: number): Promise<string | null> {
  const asked = {
    stage,
    system: request.system,
    prompt: request.prompt,
    chatId: request.chatId ?? null,
    messageId: request.messageId ?? null,
    thinking: request.thinking !== false,
  };
  let stdout = "";
  let stderr = "";
  let timer: ReturnType<typeof setTimeout> | undefined;

  // A host that cannot take the run is this gate having no answer, the same as a run that failed.
  try {
    const proc = await spawnAgentProcess(
      { args: [serviceBinary(), "gate-run", "-", "--data-root", dataRoot()], cwd: sourceDir, env: process.env, stdin: JSON.stringify(asked) },
      "gate",
      "claude-code",
      { agent: stage, workKey: stage, model, effort: null, sessionId: null },
    );

    timer = setTimeout(() => proc.kill(), timeoutMs + gateRunGraceMs);
    [stdout, stderr] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  } catch (error) {
    if (wasStopped(error)) {
      throw error;
    }

    console.error(`${stage}: the gate run could not be started: ${String(error)}`);

    return null;
  } finally {
    clearTimeout(timer);
  }

  let output: GateRunOutput;

  try {
    output = JSON.parse(stdout.trim().split("\n").at(-1) ?? "") as GateRunOutput;
  } catch {
    console.error(`${stage}: the gate run printed no verdict: ${failureText(stderr)}`);

    return null;
  }

  if ("stopped" in output) {
    throw new ExecutionStopped(output.stopped);
  }

  if ("fault" in output) {
    console.error(`${stage}: ${output.fault}`);

    return null;
  }

  if (output.answer !== null) {
    console.log(`${stage}: answered on ${model} through the Claude line`);
  }

  return output.answer;
}
