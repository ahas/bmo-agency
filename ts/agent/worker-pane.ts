import { keptThroughLock, throughLock } from "@agency/archive/archive-lock";
import { positiveFromEnv } from "@agency/support/env-number";
import { NativeExecutions } from "./native-execution";
import { beginExecution, executionCancelled, executionRoot, type ExecutionIdentity } from "./execution-identity";
import { providerSettingsArchive } from "./provider-order";
// A job's turn, run where it can be watched and read back out of the record it wrote.
//
// A run that prints its answer and a run that is watched while it works are not the same run, and
// this is where the second one lives. The first is a process on a pipe: it is spawned, its stream
// is read as it arrives, and its last frame says what it answered, what it cost and which session
// it was in. The second holds a terminal instead, so there is no stream and no last frame — and in
// exchange it can be spoken to for as long as it is going, which is the whole point.
//
// What replaces the stream is the transcript the CLI writes for itself. Nothing is scraped off the
// screen: the session is named on the way in, so the record's place is known before the run
// starts, and every figure the result frame used to carry is in it but one. What the run cost comes
// last, on the account line the CLI writes as its process ends, so it is read once the pane is
// closed. The account's own windows are in neither the record nor anywhere else a run like this can
// reach, and are named as absent rather than guessed at.
//
// A turn is not one line of that record, and a request is not one line either. A turn that uses a
// tool makes a request for the call and another for the answer; a co-worker writes its own lines
// into the same file marked as a side chain; and the CLI writes one response as a line per content
// block — its thinking, its text, its tool call — with the same message id and the whole
// response's usage repeated on every one of them. So a turn's cost is every response it added,
// each counted once, and its answer is the text of the last response that was not a side chain's.

import { guardClaudeLaunch, guardPanePrompt } from "./live-claude-guard";
import { pointedElsewhere } from "./claude-road";
import type { Overtime, OvertimeAnswer } from "./overtime-clock";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { dataRoot } from "@agency/support/roots";
import { transcriptRecords } from "./transcript-tail";

import { herdrCall, isBlocked } from "./herdr";
import { closePane, openPane, relabelHome, stopInstruction, type AgentHome } from "./herdr-run";
import { agentPlace, beginPlaceRun, endPlaceRun, endsTurn, handPlaceRun, readHooks, readRun, residentEnvironment, runVariables, statusAfterTurn } from "./agent-place";
import { interruptProgram, programAlive, programDrawn, readRunning, standResident, type ResidentProgram } from "./resident-pane";
import { commandDigest } from "./resident";
import declaredTurn from "@agency/policy/resident-turn.toml";
import turnWords from "@agency/policy/door-words.toml";
import { fillMarks } from "@agency/support/marks";
import { choiceKeys, confirmationKey, confirmationOf, type ConfirmAnswer, type PaneConfirmation } from "./pane-confirm";
import { readClaudeResidentTurn, recordsDirIn, residentTurnsOwed, statusFigures } from "./claude-code";
import { claudeAccounts } from "./claude-account";
import { claudeConfigFile, trustFolder, type FolderTrust } from "./claude-trust";
import { accountOfLogin, programLogin } from "./program-login";
import type { ClaudeAccount } from "./claude-account";
import type { AgentProviderName, HarnessRun, ResidentInterrupt, RunLine } from "./harness";
import { cliProjectsRoot, transcriptPath } from "./cli-transcripts";

import type { AgentReply } from "./agent-reply";
import type { HerdrAnswer } from "./herdr";
import type { LiveTask, RunWorkflow } from "./finished";
import type { RunLog } from "./runlog";
import { claudeSpendOf } from "./spend";

import type { ClaudeModelUsage, ModelSpend } from "./spend";

// How long a pane is given to come up with an agent ready to be told something. The prefix is
// read off disk and the CLI draws itself before it is idle; measured, that is seconds. It also
// bounds a turn that was taken and never started anything. BMO_PANE_READY_MS shortens it for a test
// whose stand-in program is made to stay silent, so that bound is reached without the two minutes.
// It is read on every turn rather than once at import: a test that stands a program made to stay
// silent sets the variable when it stages the turn, and a value frozen when this module was first
// loaded would leave that test waiting out the whole two minutes it is asking to skip.
const readyMs = (): number => positiveFromEnv(process.env.BMO_PANE_READY_MS, declaredTurn.ready_seconds * 1_000);
// How often the agent is asked what it is doing. A turn runs for minutes or hours, so this is
// paced for the watching rather than for the answer.
const pollMs: number = declaredTurn.poll_ms;
// How long a person's turn working in a resident program is waited out before a turn of the bot's
// goes on regardless (docs/agent-control.md, 「Runs and sessions in one pane」).
const personTurnMs: number = declaredTurn.person_turn_minutes * 60_000;
// How long the record is watched, once the pane is closed, for the CLI's own account of what the
// run cost. It is written as the process ends; measured, that is well under a second.
const accountMs: number = declaredTurn.account_ms;
// How long a pane back at rest is given to write the line that closes its turn. The words of a long
// answer land after its thinking, once their stream is over, and the pane can read as at rest
// first; a turn that ends without that line is read as it stands once this has passed.
const settleMs: number = declaredTurn.settle_ms;

// How long a resident program at rest may leave its record unchanged, while the count of the turns
// its round owes still expects one, before the round is read as over. A program with words queued
// takes them within a second of its turn ending, so this is paced well past that.
const quietMs: number = declaredTurn.quiet_ms;

// How long a resident turn told to stop is given to end its work before it is cut short.
const stopGraceMs: number = declaredTurn.stop_grace_seconds * 1_000;

// How long a prompt that went in is given to be seen submitted, before `enter` is pressed once and
// again after it. A submitted prompt shows within a second or two, and one still in the input box
// after this is not going anywhere on its own.
const submitMs: number = declaredTurn.submit_ms;

// The program a standing agent's turn goes into: the turn's own command where the program has to be
// started for it, and the same command resuming the session for every start after that. Its
// environment is the pane's without the turn's own variables, which go into the place instead, so a
// program started for one run is the program every run after it finds (docs/agent-control.md,
// 「The keepalive ping goes into the same program as the turns」). It is compared by the resuming
// form, which is the same for the turn that opened the session and for every turn that continues it.
export function residentOf(home: AgentHome, turn: { cwd: string; command: readonly string[]; env: Record<string, string>; sessionId: string }): { place: string; program: ResidentProgram } {
  const place = agentPlace(home.agent);
  const env = residentEnvironment(turn.env, place);
  const restart = resumedCommand(turn.command, turn.sessionId);
  // A command that opens its session is started again on the resume only once that session's record
  // exists (「A session is resumed only once it exists」).
  const opened = turn.command.includes("--session-id") ? transcriptPath(turn.cwd, turn.sessionId, cliProjectsRoot(recordsDirIn(env))) : null;

  return {
    place,
    program: {
      identity: commandDigest({ args: restart, cwd: turn.cwd, env }),
      command: { args: [...turn.command], cwd: turn.cwd, env },
      restart: { args: restart, cwd: turn.cwd, env },
      opened,
    },
  };
}

// A person's turn working in the place's program, waited out: while the place names a person's turn
// still open and the pane reads working or waiting on a question, a turn of the bot's neither stands,
// replaces nor resets the program nor puts words in. Bounded, since a person's turn cut short by an
// Escape never has its end recorded. `onWait` is told once, as the wait begins. Answers whether there
// was one to wait for.
export async function personTurnDone(
  place: string,
  options: { forMs?: number; everyMs?: number; socket?: string; check?: () => void | Promise<unknown>; onWait?: () => void | Promise<unknown>; alive?: Parameters<typeof programAlive>[1] } = {},
): Promise<boolean> {
  const deadline = Date.now() + (options.forMs ?? personTurnMs);
  let waited = false;

  for (;;) {
    const run = readRun(place);
    const pane = programAlive(place, options.alive)?.paneId ?? null;

    if (run === null || !run.person || run.endedAt !== null || pane === null) return waited;

    const status = await statusOf(pane, { socket: options.socket });

    // A herdr that did not answer has not said the person's turn is over.
    if (status !== "working" && status !== "blocked" && status !== herdrSilent) return waited;
    if (Date.now() >= deadline) return waited;

    await options.check?.();
    if (!waited) await options.onWait?.();
    waited = true;
    await Bun.sleep(options.everyMs ?? pollMs);
  }
}

// A command that opens a session under a name it chose, as the command that resumes that session.
export function resumedCommand(args: readonly string[], sessionId: string): string[] {
  const at = args.indexOf("--session-id");
  const resumed = [...args];

  if (at !== -1) resumed.splice(at, 2, "--resume", sessionId);

  return resumed;
}

// The first line of a prompt put into a resident program: which run it is, what it is and when it
// went in, so the screen says where one run ends and the next begins.
export function runHeader(name: string, title: string, at: number): string {
  const clock = new Intl.DateTimeFormat("en-GB", { timeZone: "Asia/Seoul", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(new Date(at));

  return `[${name} · ${title} · ${clock}]`;
}

// What the typed lead says about the paste after it. Claude Code records what arrives by paste as
// pasted material and, on its default system prompt, acts on an instruction inside it only where the
// person's own words ask it to; typed text is cut once it is long, so the words go in pasted behind a
// typed line that is those words (docs/agent-control.md, 「The order for putting in one turn」).
export const pastedLead: string = declaredTurn.pasted_lead;
// What it says about a room's round pasted after it: a round grants nothing, and is answered as the
// program's instructions for a round say.
export const roundLead: string = declaredTurn.round_lead;

// Words put into a Claude Code program as one message: the lead typed, then the words pasted after it
// and submitted with it.
export async function promptWithLead(pane: string, lead: string, text: string, where: { socket?: string }): Promise<HerdrAnswer<unknown>> {
  const typed = await herdrCall("pane.send_text", { pane_id: pane, text: `${lead} ` }, where);

  if (!typed.ok) return typed;

  return herdrCall("agent.prompt", { target: pane, text }, where);
}

// A round of a resident program read out of what its record gained since it went in: where it stands
// and the answer built out of every turn it spanned.
function residentReading(records: TranscriptRecord[], execution: { model?: string | null; effort?: string | null }) {
  return readClaudeResidentTurn({ lines: records }, { model: execution.model ?? "", effort: execution.effort ?? "", account: null } as HarnessRun);
}

// The key Claude Code reads as an interruption, which leaves its session usable (「What a resident
// turn reads」): the interrupt of a turn whose caller named none.
const escapeInterrupt: ResidentInterrupt = { keys: ["escape"] };
// How long a question just answered may still stand on the screen before its answer is read as not
// taken and typed again. The CLI redraws within a poll or two of the keys.
const redrawMs: number = declaredTurn.redraw_ms;
// How many times one wait for a program to come up answers the folder-trust question, where an answer
// typed did not take and the question still stands.
const trustAnswers: number = declaredTurn.trust_answers;

// What one turn in a pane came to.
//
// `answered` is the ordinary ending. `blocked` means the CLI is asking a person something and
// nothing was delivered — the folder-trust confirmation on a directory nobody has accepted yet is
// the case that arrives at launch — and it is not a failed run. `vanished` is the pane going away
// under the run: herdr died, or somebody closed it. `overran` is a turn that was given a bound and
// did not finish inside it, and its pane has been closed. `silent` is a turn that came back to rest
// with no answer in its record. `errored` is a turn whose last word was the CLI's own error in the
// place of an answer — a usage limit, an overloaded API, a refused request, a lost login — and what
// it carries is that error's wording, which is what a limit is recognised by.
export type PaneTurn =
  | { outcome: "answered"; reply: AgentReply; records: number }
  | { outcome: "blocked"; screen: string }
  // The program would not get past the folder-trust question, for a run that trusts its folder: the
  // trust was written and the program stood up again, and the question still stood. Nobody is meant to
  // answer it, so it is not a question left for a person; what it names is the folder and the
  // configuration file the trust was written to.
  | { outcome: "untrusted"; screen: string; folder: string; config: string }
  | { outcome: "vanished"; screen: string }
  // Herdr answered none of the turn's looks for the whole of its patience. Nothing is known of the pane
  // or the run, so it is its own ending and never a pane gone.
  | { outcome: "unanswered"; screen: string }
  | { outcome: "overran"; screen: string }
  | { outcome: "silent"; screen: string }
  // What it carries is that error as the record holds it, which is the run's own words and not a
  // screen at all: the CLI wrote it where the answer would have gone. So it is the one payload here
  // a caller may hand on, and it is also what a limit and an outage are recognised by — a judgement
  // a screen must never reach (`docs/agent-control.md`, 「What a failed turn may say」).
  | { outcome: "errored"; error: string }
  // The prompt never became a turn: the call putting it in was refused twice, or it stood in the input
  // box and `enter` did not send it either. Nothing of the work began. The screen is what the box held.
  | { outcome: "unsubmitted"; screen: string }
  // Ended from outside while it was running: a cancellation stood over this run's own name or its
  // root, and the turn ended itself on meeting it. It is its own ending rather than a failure,
  // because nothing about the work went wrong and nothing is to be retried on another road.
  | { outcome: "stopped"; screen: string };

const activityDetailLimit = 160;

// The last line a turn wrote to its record, read for when it was written and what it was: the tool a
// response called with what it was given, words written, thinking, or a tool's answer coming back.
export function lastActivityOf(records: readonly TranscriptRecord[], from: number): { at: number | null; what: string | null } {
  const last = records.slice(from).findLast((record) => record.type === "assistant" || record.type === "user");

  if (last === undefined) return { at: null, what: null };

  const at = last.timestamp === undefined ? Number.NaN : Date.parse(last.timestamp);
  const blocks = blocksOf(last);
  const tool = blocks.findLast((block) => block.type === "tool_use");
  const detail = (input: Record<string, unknown> | undefined): string => {
    const value = [input?.command, input?.file_path, input?.path, input?.description].find((field) => typeof field === "string" && field !== "");

    return typeof value === "string" ? ` (${value.replace(/\s+/g, " ").slice(0, activityDetailLimit)})` : "";
  };
  const what = tool !== undefined
    ? `called ${tool.name ?? "a tool"}${detail(tool.input)}`
    : blocks.some((block) => block.type === "tool_result")
      ? "a tool's answer came back"
      : blocks.some((block) => block.type === "text")
        ? last.type === "assistant" ? "wrote words" : "was handed words"
        : blocks.some((block) => block.type === "thinking")
          ? "was thinking"
          : null;

  return { at: Number.isNaN(at) ? null : at, what };
}

interface TranscriptRecord {
  type?: string;
  // What kind of a system line it is; `turn_duration` is the one the CLI writes once a turn is over.
  subtype?: string;
  // What a queue line did with a word handed over: `enqueue` the moment it arrived.
  operation?: string;
  uuid?: string;
  sessionId?: string;
  isSidechain?: boolean;
  // Set on a line the CLI wrote itself in the place of a model's answer, when the request failed.
  isApiErrorMessage?: boolean;
  // On the account line the CLI writes as its process ends: what the session has cost so far, model
  // by model, in the words a print-mode result frame uses.
  modelUsage?: Record<string, ClaudeModelUsage>;
  effort?: string;
  message?: {
    id?: string;
    role?: string;
    model?: string;
    content?: unknown;
    usage?: Record<string, unknown>;
  };
  // When the CLI wrote the line, as an ISO time.
  timestamp?: string;
  // Interactive Claude Code records put structured evidence for a tool answer beside the
  // message. The activity ledger consumes it to distinguish a request from a proven outcome.
  toolUseResult?: unknown;
}

interface ContentBlock {
  type?: string;
  text?: string;
  name?: string;
  id?: string;
  tool_use_id?: string;
  input?: Record<string, unknown>;
}

const blocksOf = (record: TranscriptRecord): ContentBlock[] =>
  Array.isArray(record.message?.content) ? (record.message?.content as ContentBlock[]) : [];

const count = (usage: Record<string, unknown> | undefined, name: string): number => {
  const value = usage?.[name];

  return typeof value === "number" ? value : 0;
};

// What a turn added to the record, read as a reply.
//
// Given the record and how much of it had already been read, this is the turn: its cost is every
// response it added, side chains included, because a co-worker's tokens are that run's tokens; its
// answer is the last response that was not a side chain's, because a side chain answers the agent
// and not the caller. A response is known by its message id, since the lines it was split into all
// carry it; a line with no id is a response of its own.
export function readTurn(records: TranscriptRecord[], from: number): {
  text: string;
  model: string | null;
  effort: string | null;
  spend: ModelSpend[];
  contextTokens: number | null;
  // The CLI's own error where the turn ended on one instead of an answer, and nothing otherwise.
  error: string | null;
  liveTasks: LiveTask[];
  workflows: RunWorkflow[];
} {
  const turn = records.slice(from);
  const assistants = turn.filter((record) => record.type === "assistant");
  const mine = assistants.filter((record) => record.isSidechain !== true);
  const last = mine[mine.length - 1];
  const lastResponse =
    last?.message?.id === undefined ? (last === undefined ? [] : [last]) : mine.filter((record) => record.message?.id === last.message?.id);
  const byModel = new Map<string, ModelSpend>();
  // One entry per response. The last of a response's lines is the one kept, since it is the one
  // written once the response was whole.
  const responses = new Map<string, TranscriptRecord>();

  assistants.forEach((record, at) => responses.set(record.message?.id ?? `line ${at}`, record));

  for (const record of responses.values()) {
    const model = record.message?.model ?? "";
    const usage = record.message?.usage;
    const held = byModel.get(model) ?? {
      mode: "claude-code" as const,
      model,
      tokens: 0,
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheWriteTokens: 0,
      // The record carries the four token counts and no price. A figure nobody reported is left
      // at nothing rather than worked out here from a table that would go out of date.
      cost: 0,
    };
    const inputTokens = count(usage, "input_tokens");
    const outputTokens = count(usage, "output_tokens");
    const cacheReadTokens = count(usage, "cache_read_input_tokens");
    const cacheWriteTokens = count(usage, "cache_creation_input_tokens");

    held.inputTokens += inputTokens;
    held.outputTokens += outputTokens;
    held.cacheReadTokens += cacheReadTokens;
    held.cacheWriteTokens += cacheWriteTokens;
    held.tokens += inputTokens + outputTokens + cacheReadTokens + cacheWriteTokens;
    byModel.set(model, held);
  }

  const spoken = lastResponse
    .flatMap(blocksOf)
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text ?? "")
    .join("")
    .trim();
  // How much the model was holding on its last request of the turn, which is what the fold's
  // clock is read against. The record states it per request rather than as a window, so it is the
  // sum of what that request carried.
  const contextTokens =
    last === undefined
      ? null
      : count(last.message?.usage, "input_tokens") +
        count(last.message?.usage, "cache_read_input_tokens") +
        count(last.message?.usage, "cache_creation_input_tokens");

  return {
    text: spoken,
    model: last?.message?.model ?? null,
    effort: last?.effort ?? null,
    spend: [...byModel.values()].filter((held) => held.model !== ""),
    contextTokens,
    error: last?.isApiErrorMessage === true ? spoken : null,
    ...leftGoing(records, from),
  };
}

// Whether the record holds an answer written after the point a turn began: an assistant line of the
// run's own, not a co-worker's.
export function answeredSince(records: TranscriptRecord[], from: number): boolean {
  return records.slice(from).some((record) => record.type === "assistant" && record.isSidechain !== true);
}

// Whether the record holds a prompt put in by a person after that point: a user line of the main
// conversation carrying words, never one carrying a tool's answer.
export function promptedSince(records: TranscriptRecord[], from: number): boolean {
  return records.slice(from).some((record) => {
    if (record.type !== "user" || record.isSidechain === true || (record as { isMeta?: boolean }).isMeta === true) return false;

    const content = record.message?.content;

    return typeof content === "string" || (Array.isArray(content) && content.some((part) => (part as { type?: string }).type === "text"));
  });
}

// Whether the record holds the line the CLI writes once a turn is over, written after that point.
export function turnClosedSince(records: TranscriptRecord[], from: number): boolean {
  return records.slice(from).some((record) => record.type === "system" && record.subtype === "turn_duration");
}

// The code a push answers with where the turn it was meant for has already ended: a refusal like any
// other, so its words go back to wait for the next round.
export const turnEndedCode = "turn_ended";

// Waits, bounded, until the record holds a word pushed at the point it stood: queued by a working
// turn, or submitted straight as a prompt by a program at rest.
async function wordWrittenDown(path: string, from: number, timing: { waitMs: number; everyMs: number }): Promise<void> {
  const by = Date.now() + timing.waitMs;

  for (;;) {
    const records = lines(path);

    if (promptedSince(records, from) || records.slice(from).some((record) => record.type === "queue-operation" && record.operation === "enqueue")) return;
    if (Date.now() >= by) return;

    await Bun.sleep(timing.everyMs);
  }
}

// The work a run walked away from, read out of the record the way the record says it.
//
// A run's background work — a Claude Workflow, a co-worker sent off on its own, a shell command left going —
// is launched by a call that is answered at once, with a line saying the work is now running in
// the background, and it ends later in a task notification naming the call it was launched by and
// the status it ended with. So what was still going when the turn stopped is a launch this turn
// made with no ending anywhere after it, and a Claude Workflow is one it walked away from unless its
// ending said it ran to the end. A call that never got an answer at all was cut off in the middle,
// which is the same state. Only this turn's own launches are counted: a session carries every
// earlier turn too, and a launch some earlier run abandoned is that run's to have been judged by.
const launchedWording = /running in background with ID|moved to the background|launched in background|Async agent launched/i;
const notification = /<task-notification>([\s\S]*?)<\/task-notification>/g;

function leftGoing(records: TranscriptRecord[], from: number): { liveTasks: LiveTask[]; workflows: RunWorkflow[] } {
  const turn = records.slice(from);
  const asked = new Map<string, ContentBlock>();
  const answers = new Map<string, string>();
  const endings = new Map<string, string>();

  for (const record of turn) {
    for (const block of blocksOf(record)) {
      if (block.type === "tool_use" && block.id !== undefined) {
        asked.set(block.id, block);
      }

      if (block.type === "tool_result" && block.tool_use_id !== undefined) {
        answers.set(block.tool_use_id, resultText(block));
      }
    }

    // A notification reaches the record as a queued prompt, an attachment or the user line it was
    // taken up as, so it is looked for in the whole of the line rather than in one field of it.
    for (const found of JSON.stringify(record).matchAll(notification)) {
      const body = (found[1] ?? "").replaceAll("\\n", "\n");
      const call = /<tool-use-id>([^<]+)<\/tool-use-id>/.exec(body)?.[1];
      const status = /<status>([^<]+)<\/status>/.exec(body)?.[1];

      if (call !== undefined && status !== undefined) {
        endings.set(call, status);
      }
    }
  }

  const liveTasks: LiveTask[] = [];
  const workflows: RunWorkflow[] = [];

  for (const [id, block] of asked) {
    const answer = answers.get(id);
    const launched = answer !== undefined && launchedWording.test(answer);

    if (block.name === "Workflow") {
      if (answer === undefined || launched) {
        workflows.push({ call: id, name: workflowName(block), status: endings.get(id) ?? null });
      }

      continue;
    }

    const branch = block.name === "Task" || block.name === "Agent";

    if ((branch && answer === undefined) || (launched && !endings.has(id))) {
      liveTasks.push({ id, description: taskDescription(block) });
    }
  }

  return { liveTasks, workflows };
}

function resultText(block: ContentBlock & { content?: unknown }): string {
  if (typeof block.content === "string") {
    return block.content;
  }

  return Array.isArray(block.content)
    ? (block.content as ContentBlock[]).map((part) => part.text ?? "").join("")
    : "";
}

function taskDescription(block: ContentBlock): string {
  const said = block.input?.["description"];
  const kind = block.input?.["subagent_type"];

  return typeof said === "string" && said !== "" ? said : typeof kind === "string" ? kind : (block.name ?? "");
}

// What a Claude Workflow was launched as. The call carries the script rather than a name of its own, and
// the script names itself in the block every one of them opens with, so that is where the name is
// read from; a script that named itself some other way is reported by its call instead.
function workflowName(block: ContentBlock): string {
  const script = block.input?.["script"];
  const named = typeof script === "string" ? /name:\s*['"`]([^'"`]+)['"`]/.exec(script) : null;

  return named?.[1] ?? (typeof block.input?.["name"] === "string" ? (block.input["name"] as string) : "");
}

// The folder-trust question a CLI asks the first time it comes up in a directory nobody has
// accepted, read off the screen, and the keys that accept it — or nothing, where the screen is not
// that question.
//
// The cursor stands on refusing, so the one key that is obviously an answer ends the CLI, and so
// does escape. Accepting is moving the cursor down to the accepting row and confirming, which is
// counted off the screen rather than assumed: the rows are matched by the two options' own words,
// which fit on a line however narrow the pane, and the cursor row is the one the CLI marks.
const trustRefusal = "No, exit";
const trustAcceptance = "Yes, I trust this folder";
const cursorMark = "❯";

export function folderTrustKeys(screen: string): string[] | null {
  const rows = screen.split("\n");
  const refusal = rows.findIndex((row) => row.includes(trustRefusal));
  const acceptance = rows.findIndex((row) => row.includes(trustAcceptance));

  if (refusal < 0 || acceptance < 0) {
    return null;
  }

  const first = Math.min(refusal, acceptance);
  const cursor = rows.findIndex((row, at) => at >= first && at <= acceptance && row.trimStart().startsWith(cursorMark));

  if (cursor < 0) {
    return null;
  }

  return [...Array.from({ length: acceptance - cursor }, () => "down"), "enter"];
}

// The value a command line gives a flag, where it gives the flag at all.
function flagValue(command: readonly string[], flag: string): string | null {
  return command.includes(flag) ? command[command.indexOf(flag) + 1] ?? null : null;
}

// One turn, start to finish, in a pane of its own.
//
// The order is the measured one and every step of it is there for a reason. The pane comes up
// with no work in it, because a run told its work on the command line cannot be told a second
// thing; it is waited for until it is ready, because a prompt pushed at an agent that is not
// ready is refused; the prompt goes in without a wait attached, because a wait would hold this
// call for the whole length of the turn; and the turn is over when the agent has been seen
// working and has stopped, because the idle it starts in looks exactly like the idle it ends in.
export async function runTurnInPane(options: {
  identity?: ExecutionIdentity;
  name: string;
  // What the turn is, for a person watching herdr, and the standing agent whose workspace it opens
  // in — none for a turn that is nobody's standing session.
  title: string;
  home?: AgentHome | null;
  cwd: string;
  command: readonly string[];
  env: Record<string, string>;
  prompt: string;
  sessionId: string;
  // Where the record already stood, so the answer read back is this turn's and not the session's.
  from: number;
  log?: RunLog | null;
  // Anything to say to the run while it is still working, asked on every look. It is handed the
  // way to say it rather than the pane, so what decides whether there is something to say — the
  // archive, in the job's case — stays with whoever knows, and the socket stays here.
  //
  // This is the whole reason a job holds a terminal instead of a pipe: a word can reach it while
  // it works. Measured: a word pushed at an agent that is `working` is accepted and acted on
  // inside the turn it is already in, not held over to the next one.
  aside?: (say: (text: string) => Promise<HerdrAnswer<unknown>>) => Promise<void>;
  // What a pane is given to come up, and how often it is asked. The turn itself is unbounded:
  // whatever is holding this run owns its lifetime, exactly as it did when the run was a process
  // on a pipe.
  readyMs?: number;
  pollMs?: number;
  // How long a question just answered may still stand on the screen before the answer is typed again.
  redrawMs?: number;
  // How long a person's turn working in the resident program is waited out before this turn goes on.
  personTurnMs?: number;
  // How long the closed pane's record is watched for the run's account of what it cost.
  accountMs?: number;
  settleMs?: number;
  // How long a prompt that went in is given to be seen submitted, before and after `enter` is pressed.
  submitMs?: number;
  // How long the turn itself may take before its caller is asked about it: a bounded run such as
  // upkeep or a word said to a worker. A job's turn and a chat round have no bound — whatever holds
  // the job owns its lifetime, and a round runs until it answers — so this is left out for both.
  turnMs?: number;
  // What is asked each time the bound passes, before anything is cut: the caller's policy, which
  // answers cut or the next moment to look again, while the turn goes on being watched. A turn
  // given none is cut at its bound (docs/agent-control.md, 「A clock asks before it cuts」).
  overtime?: Overtime;
  // Whether the run trusts the directory it works in. A CLI brought up in a directory nobody has
  // accepted sits on that one question before it will take anything, and a run whose caller says
  // the place is its own answers it and goes on; any other question is still a person's. It is
  // the caller's to say, because whether a place is trusted is a decision about the place and not
  // about how a turn is run.
  trustsFolder?: boolean;
  // What to do about a question the CLI puts to a person in the middle of the turn — a safety check
  // that asks even in bypass mode. Asked once per question while the turn goes on being watched,
  // and the answer is applied only if the pane still shows that question; the signal is aborted
  // when the pane stops asking first — a person answering it in herdr — or the turn ends. A turn given none
  // leaves such a question for a person.
  confirm?: (asked: PaneConfirmation, answeredElsewhere: AbortSignal) => Promise<ConfirmAnswer>;
  // Which herdr. Named only where it is not the machine's own — a test drives a herdr of its own
  // rather than handing the real one a pane.
  socket?: string;
  // How long a resident turn told to stop is given to end its work before it is interrupted.
  stopGraceMs?: number;
  // How long a resident program at rest may leave its record unchanged before a round whose count
  // still expects a turn is read as over.
  quietMs?: number;
  // How long herdr may answer none of the turn's looks before the turn ends on that silence, and how
  // a pane found missing is confirmed gone; a test shortens both.
  herdrPatienceMs?: number;
  goneConfirm?: { looks?: number; gapMs?: number };
  // Every line the record gains while the turn runs, as it is read.
  onLine?: (line: string) => void;
  // How the resident program's working turn is cut short: its surface's own interrupt.
  interrupt?: ResidentInterrupt;
  // The line the turn runs on, for its execution row: a job's by default, a room's round where the
  // chat's own resident program takes it.
  line?: RunLine;
  // The provider the turn goes out on, for its execution row: Claude Code's own by default, or the
  // road the program is pointed at.
  provider?: AgentProviderName;
  // What the typed lead says the pasted words are. A job's and an upkeep turn's are the owner's own
  // words; a room's round says what it is and grants nothing, since what a room says is not the owner's.
  lead?: string;
}): Promise<PaneTurn> {
  // A home on a work node names its own herdr (docs/work-nodes.md).
  const where = { socket: options.socket ?? options.home?.socket };
  const lead = options.lead ?? pastedLead;
  const everyMs = options.pollMs ?? pollMs;
  const goneConfirm = options.goneConfirm ?? {};
  // Since when herdr has answered none of the turn's looks, while it goes on answering none.
  let silentSince: number | null = null;
  // A pane that will hold a Claude Code session is a session opened, and one opened from inside a
  // job or a test is asked about first (docs/live-claude-guard.md).
  const archive = providerSettingsArchive();
  if (!archive) throw new Error("execution archive unavailable");
  // Which registered login answered the turn: the one its status reading names where there is one,
  // and otherwise the one the pane was launched on, as the environment it was handed names it. Left
  // out where neither is a registered login, so the one who launched the turn says.
  const loginAnswered = async (named: ClaudeAccount | null): Promise<{ account?: string }> => {
    const account = named ?? accountOfLogin(await claudeAccounts(archive), programLogin(options.env));

    return account === null ? {} : { account: account.id };
  };
  // The row the turn is, and the one write it cannot go on without: a lock refusing it is waited
  // out rather than read as a failure to launch.
  const execution = await throughLock(async () => beginExecution(archive, options.provider ?? "claude-code", options.line ?? "worker", {
    root: executionRoot(options.name), parent: options.name, sessionId: options.sessionId,
    model: flagValue(options.command, "--model"),
    effort: flagValue(options.command, "--effort"),
    // What the caller names stands over what the command and the run's name say; what it leaves
    // unnamed keeps them.
    ...Object.fromEntries(Object.entries(options.identity ?? {}).filter((entry) => entry[1] !== undefined)),
    // The pane is this turn's own process, whatever else the caller said about who it is. Written
    // by the insert rather than by a later save, so a collector killed before its first save still
    // leaves a row that says which evidence ends it.
    panePath: true,
  }));
  const owner = { pid: process.pid, birth: processBirth(process.pid) };
  // The turn's own record-keeping, and a lock another writer holds is not the turn's to die of:
  // what the caller is owed is the answer, so a row the lock never lets through is said in the
  // journal and left as it stood (docs/archive-locks.md).
  const recorded = async (what: string, work: () => Promise<void>): Promise<void> => {
    await keptThroughLock(`${options.name}: ${what}`, work);
  };
  const saveRow = () => recorded(`the execution row (${execution.state})`, () => archive.executions().saveExecution(execution));
  // Closing the row with how the turn ended: the envelope first, so a collector killed before the
  // archive's write still leaves the cause behind.
  const closeRow = async (state: PaneTurnEnd | "launch-error"): Promise<void> => {
    publishEnvelope(execution.id, { owner, cause: state });
    execution.state = state;
    execution.endedAt = Date.now();
    await saveRow();
  };
  publishEnvelope(execution.id, { owner });
  const cancelled = () => executionCancelled(archive.settings(), [options.name, execution.id, execution.root]);
  const checkLaunch = async () => {
    if (await cancelled()) throw new Error("execution cancelled");
    await guardClaudeLaunch("pane", options.command, options.env);
    // A program on another's road writes no Claude session and Claude being off does not close it.
    await guardPanePrompt({ claude: !pointedElsewhere(options.env) });
  };
  // An agent with a standing workspace takes its turn in the program standing in its pane, which is
  // stood up or replaced first where it is not the program this turn needs; a turn that is nobody's
  // standing session opens a pane of its own for as long as it runs (docs/agent-control.md, 「One
  // standing pane per agent」).
  const resident = options.home === undefined || options.home === null ? null : residentOf(options.home, options);
  // A run that trusts its folder has the folder written as trusted, in the file its own login reads,
  // before any program comes up in it, so the question never stands; the log is told what was written
  // and what could not be (`docs/agent-control.md`, 「A first interactive run stops at the
  // trust-this-folder check」).
  const trustConfig = claudeConfigFile(options.env);
  const trustHere = async (): Promise<void> => {
    if (options.trustsFolder !== true) return;

    let trust: FolderTrust | string;

    try {
      trust = trustFolder(trustConfig, options.cwd);
    } catch (error) {
      trust = String(error);
    }

    if (trust === "written") await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.trust_written, { config: trustConfig, cwd: options.cwd }) });
    else if (trust !== "trusted") await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.trust_unwritten, { config: trustConfig, cwd: options.cwd, why: trust }) });
  };
  let opened: Awaited<ReturnType<typeof openPane>>;
  try {
    await checkLaunch();
    await trustHere();
    if (resident !== null) {
      await personTurnDone(resident.place, {
        forMs: options.personTurnMs,
        everyMs: options.pollMs,
        socket: where.socket,
        check: checkLaunch,
        onWait: () => options.log?.write({ kind: "note", text: turnWords.notes.person_turn }),
      });
      const stood = await standResident({ place: resident.place, home: options.home as AgentHome, program: resident.program, line: options.line ?? "worker", socket: where.socket, waitMs: options.readyMs ?? readyMs() });

      opened = stood.ok ? { ok: true, result: { workspaceId: "", paneId: stood.result.paneId } } : stood;
    } else {
      opened = await openPane(
        { name: execution.id, title: fillMarks(turnWords.notes.title, { title: options.title, model: execution.model ?? "claude-code", attempt: String(execution.attempt) }), home: options.home, cwd: options.cwd, command: options.command, env: options.env },
        where,
      );
    }
  } catch (error) {
    await closeRow("launch-error");
    throw error;
  }

  if (!opened.ok) {
    await closeRow("launch-error");
    throw new Error(`herdr would not take ${options.name}: ${opened.error.code}: ${opened.error.message}`);
  }

  const pane = opened.result.paneId;
  execution.paneId = pane;
  // The records of the login this pane was handed, which is where its CLI writes them.
  const path = transcriptPath(options.cwd, options.sessionId, cliProjectsRoot(recordsDirIn(options.env)));
  // Everything the record has gained since a point, handed on as it is read; answers where it now ends.
  const readOn = async (at: number): Promise<number> => report(path, at, options.log ?? null, options.onLine);
  // A turn of a resident program says so on its row, with where its program's record is read from
  // and which process the program was, since the pane outlives the turn and cannot be what closes it.
  if (resident !== null) {
    const running = readRunning(resident.place);

    execution.resident = { place: resident.place, record: path, from: options.from, pid: running?.pid ?? null, birth: running?.birth ?? null };
  }
  const native = new NativeExecutions(archive, execution);
  let observed = options.from;
  // Where this turn's own lines begin in the record. It is read again as the prompt goes in: a program
  // started again for this turn writes lines of its own on the way up, and an earlier turn's lines —
  // an upkeep ping that went in just before — are not this turn's either.
  let from = options.from;
  const recordChildren = async () => {
    const records = lines(path);
    await recorded("a child run's row", async () => {
      for (const record of records.slice(observed)) await native.observe(record);
    });
    observed = records.length;
  };

  // A screen is evidence and never words to hand on (`docs/agent-control.md`, 「What a failed turn
  // may say」). Every screen this turn captures is written to the run's log where it is taken, so a
  // person has the whole of it to read, and what the turn itself says names the state instead.
  const keptScreen = async (why: string, screen: string): Promise<string> => {
    await options.log?.write({ kind: "note", text: screen === "" ? why : `${why}\n\n${screen}` });

    return screen;
  };

  let attemptedPrompt = false;
  let promptedAt = Date.now();
  let stopAskedAt: number | null = null;
  // The questions the pane puts to the caller while the turn is watched. Held out here so a turn
  // that ends while one is out withdraws it.
  const questions = new PaneQuestions(pane, where, options.log ?? null, options.confirm, options.redrawMs ?? redrawMs);
  // How the turn ended, for its execution row. A turn the collector gave up on is cancelled, one
  // past its time timed out; every other ending is the CLI's own and it exited.
  let ending: PaneTurnEnd = "exited";
  // Bringing a wedged program back to rest: it is ended, its loop starts it again on the same
  // session with an empty input box — the same reset an unsubmitted prompt makes below — and the
  // place is watched until the program standing is a different one, because until its record says
  // so a prompt put in would go into the program just ended. A turn with no resident program has
  // none of this: nothing of it outlives the turn and there is nothing standing to reset
  // (`docs/agent-control.md`, 「When a program will not come to rest」).
  const standAgain = async (why: string): Promise<boolean> => {
    if (resident === null || await cancelled()) return false;

    const was = readRunning(resident.place);

    await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.stand_again, { why }) });
    await interruptProgram(resident.place, pane, { endProgram: true }, where);

    return programStoodAgain(resident.place, was, { forMs: options.readyMs ?? readyMs(), everyMs, stop: cancelled });
  };

  try {
    await saveRow();
    // One reset and one retry per turn, never a loop. A turn given up here leaves the program as it
    // stands, and the same wedge meets the next round, and the one after that, until the room has
    // gone quiet with nothing in the record saying why.
    let reset = false;

    for (;;) {
      // Ready is the program at its input box: idle when it has just come up, and done once it has
      // finished a turn, which is how herdr reads a resident program between its turns (measured).
      const ready = await settled(pane, {
        check: checkLaunch,
        until: ["idle", "done"],
        forMs: options.readyMs ?? readyMs(),
        everyMs,
        socket: where.socket,
        confirm: goneConfirm,
        // A resident program started again for this turn reads at rest from the one before it until it
        // is up, and words put in meanwhile are lost whole.
        drawn: resident === null ? undefined : () => programDrawn(resident.place),
        answer: options.trustsFolder === true ? (screen) => acceptFolderTrust(pane, screen, where, options.log ?? null) : undefined,
        answerAgainMs: options.redrawMs,
      });

      if (ready.status === "gone") {
        // Gone before it was ever ready, and what it said on the way out is the whole of why.
        ending = "cancelled";
        return { outcome: "vanished", screen: await keptScreen(turnWords.screens.vanished, ready.screen) };
      }

      if (ready.status === herdrSilent) {
        // Herdr answered none of the looks by the end of the ready time: nobody has seen the program,
        // so it is not reset as a wedge, and the turn ends on the silence.
        await options.log?.write({ kind: "note", text: turnWords.notes.herdr_silent_ready });
        ending = "cancelled";
        return { outcome: "unanswered", screen: "herdr did not answer while the turn waited for its program" };
      }

      if (ready.status !== "idle" && ready.status !== "done") {
        // A question a person must answer, or no state at all. The two are not the same thing and the
        // wait ends the same way for both (`docs/agent-control.md`, 「blocked before the prompt is an
        // end in itself」); the screen the pane stood at goes to the log, where it is evidence.
        //
        // A question is left standing for whoever is to answer it, and only a turn that can put one
        // to somebody has such a question: a chat round is given no way to ask, so a pane blocked in
        // front of one is a question nobody is coming to answer, which is a wedge like any other.
        // Everything that is not a question is a wedge outright. The folder-trust question in a run that
        // trusts its folder is nobody's to answer: the trust is written again and the program stood up
        // again, which reads it afresh, and where even that leaves it standing the turn names it.
        const untrusted = options.trustsFolder === true && ready.status === "blocked" && folderTrustKeys(ready.screen) !== null;
        const asksSomebody = ready.status === "blocked" && options.confirm !== undefined && !untrusted;
        const screen = await keptScreen(
          turnWords.screens.not_ready,
          ready.screen === "" ? await screenOf(pane, where) : ready.screen,
        );

        if (!reset && !asksSomebody) {
          reset = true;

          if (untrusted) await trustHere();
          if (await standAgain(untrusted ? turnWords.notes.untrusted : turnWords.notes.not_ready)) continue;
        }

        ending = "cancelled";
        return untrusted ? { outcome: "untrusted", screen, folder: options.cwd, config: trustConfig } : { outcome: "blocked", screen };
      }

      if (await cancelled()) throw new Error("execution cancelled");
      await checkLaunch();
      attemptedPrompt = true;
      // A resident program stands across many runs, so the run this turn is — its name, what it is,
      // its own variables — goes into the place before the prompt, and the prompt's first line names
      // it where the screen shows it (「Runs and sessions in one pane」). A command of the program's own
      // — the fold word — is the whole of what it is told, since the program reads it as a command only
      // when nothing stands before it.
      promptedAt = Date.now();
      if (resident !== null) beginPlaceRun(resident.place, options.name, options.title, promptedAt, runVariables(options.env));
      from = Math.max(from, lines(path).length);
      if (execution.resident !== undefined && execution.resident !== null) execution.resident.from = from;
      const sent = options.prompt.startsWith("/")
        ? await herdrCall("agent.prompt", { target: pane, text: options.prompt }, where)
        : await promptWithLead(pane, resident === null ? lead : `${runHeader(options.name, options.title, promptedAt)} ${lead}`, options.prompt, where);

      if (!sent.ok) {
        // Nothing of the turn went in, and a program that will not take the words stands in a state
        // it should not be in. It is the same wedge and takes the same one reset — unless the refusal
        // is a question somebody is to be asked, which is left standing like the one above.
        const screen = await keptScreen(
          fillMarks(turnWords.screens.not_taken, { code: String(sent.error.code), message: sent.error.message }),
          await screenOf(pane, where),
        );

        if (isBlocked(sent) && options.confirm !== undefined) {
          ending = "cancelled";
          return { outcome: "blocked", screen };
        }

        if (!reset) {
          reset = true;

          if (await standAgain(turnWords.notes.not_taken)) continue;
        }

        // Refused twice, the words never became a turn, and nothing asks anybody anything. The lead
        // typed before the refusal is still in the box, so a resident program is started again with
        // it empty, as it is for words that stood there unsent.
        if (resident !== null) await interruptProgram(resident.place, pane, { endProgram: true }, where);
        ending = "cancelled";
        return { outcome: "unsubmitted", screen };
      }

      break;
    }

    // Accepted is not submitted: pasted words can stand in the input box unsent, and a program at rest
    // with words in its box reads exactly like one that has finished (「The measured, fixed order」).
    const taking = { waitMs: options.submitMs ?? submitMs, everyMs };
    let taken = await promptTaken(pane, path, from, where, taking);

    if (taken === "unsent") {
      await options.log?.write({ kind: "note", text: turnWords.notes.enter_pressed });
      await sendKeys(pane, ["enter"], where);
      taken = await promptTaken(pane, path, from, where, taking);

      if (taken === "unsent") {
        const screen = await screenOf(pane, where);

        await options.log?.write({ kind: "note", text: turnWords.notes.unsubmitted });
        // The words left in the box would go out in front of the next turn's prompt, so a resident
        // program is started again on its session with the box empty.
        if (resident !== null) await interruptProgram(resident.place, pane, { endProgram: true }, where);
        ending = "cancelled";
        return { outcome: "unsubmitted", screen: await keptScreen(turnWords.screens.unsubmitted, screen) };
      }
    }

    execution.state = "running"; await saveRow();
    let seen = from;
    // A look that saw the program at work while the prompt was being seen submitted is a look of this turn's.
    let worked = taken === "working";
    // Moved on, not lifted, by the caller's overtime policy each time it answers anything but a cut.
    let deadline = options.turnMs === undefined ? null : Date.now() + options.turnMs;
    // A prompt that was taken and never started anything. The turn itself is unbounded, but a run
    // that is still at rest, has never been seen at work and has written nothing is not working on
    // anything; given as long as a pane is given to come up, it is a turn that came back silent
    // rather than one to wait on for ever.
    let unstartedBy = Date.now() + (options.readyMs ?? readyMs());
    // Where this turn's answer is looked for. It moves when the run is told something more after a
    // refusal, since whatever it had written before that is not the end of the turn.
    let answerFrom = from;
    // A word pushed into the working turn (docs/agent-control.md, 「Words that arrive while a turn is
    // working」). Herdr's working can trail the turn's real end by a look, and making the push takes
    // seconds more, so whether the turn still works is read again just before the keys go: off the
    // place, whose stop hook closes the run at the end, and off the record. The run is marked as
    // handed the word first, so an end landing between that reading and the keys still finds the
    // word the bot's; a push that does not go in takes the mark back. One that does is waited on
    // until the record holds it, so the round does not read its end without it.
    const pushWord = async (text: string): Promise<HerdrAnswer<unknown>> => {
      if (resident !== null) handPlaceRun(resident.place);

      const run = resident === null ? null : readRun(resident.place);
      const over = (run !== null && (run.person || run.endedAt !== null)) || residentReading(lines(path).slice(from), execution).state !== "working";
      const at = lines(path).length;
      const said: HerdrAnswer<unknown> = over ? { ok: false, error: { code: turnEndedCode, message: "the turn ended before the words could go in" } } : await promptWithLead(pane, lead, text, where);

      if (!said.ok) {
        if (resident !== null) handPlaceRun(resident.place, false);

        return said;
      }

      await wordWrittenDown(path, at, { waitMs: options.submitMs ?? submitMs, everyMs });

      return said;
    };

    for (;;) {
      // Stopped from outside while the turn was running. The marker by itself only refuses the
      // next launch, so a turn already under way would go on for as long as its CLI kept working
      // — alive, and unable to take any step that needs a launch. It ends here instead, by its own
      // collector, which closes the pane in the ending below and hands its caller a stop to report.
      if (await cancelled()) {
        // A resident program is not closed to stop its turn. A job is asked to end the work first —
        // commit, report, exit — and cut short with the surface's interrupt once the grace is spent
        // while it still works; a chat round has no work of its own to wind down and is cut short at
        // once.
        const asksFirst = options.line !== "chat";

        if (resident !== null && stopAskedAt === null) {
          stopAskedAt = Date.now();

          if (asksFirst) {
            await guardPanePrompt({ claude: !pointedElsewhere(options.env) });
            await promptWithLead(pane, lead, stopInstruction, where);
            await Bun.sleep(everyMs);
            continue;
          }
        }

        if (resident !== null && stopAskedAt !== null) {
          const standing = await statusOf(pane, where);

          if (asksFirst && Date.now() - stopAskedAt < (options.stopGraceMs ?? stopGraceMs) && standing === "working") {
            seen = await readOn(seen);
            await Bun.sleep(everyMs);
            continue;
          }

          if (standing === "working") {
            await interruptProgram(resident.place, pane, options.interrupt ?? escapeInterrupt, where);
            // The row closes as a stop on the interrupted marker the program writes, looked for a
            // bounded while.
            const settleBy = Date.now() + (options.settleMs ?? settleMs);

            while (residentReading(lines(path).slice(from), execution).state === "working" && Date.now() < settleBy) {
              await Bun.sleep(everyMs);
            }
          }
        }

        await readOn(seen);

        ending = "cancelled";
        return { outcome: "stopped", screen: await keptScreen(turnWords.screens.stopped, await screenOf(pane, where)) };
      }

      if (deadline !== null && Date.now() >= deadline && options.overtime !== undefined && options.turnMs !== undefined) {
        seen = await readOn(seen);
        const last = lastActivityOf(lines(path), from);
        // A policy that cannot answer leaves the turn where a turn handed none stands: cut at its bound.
        const answer = await options.overtime({ startedAt: promptedAt, boundMs: options.turnMs, lastActivityAt: last.at, lastActivity: last.what }).catch(async (error: unknown): Promise<OvertimeAnswer> => {
          await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.overrun_unasked, { why: error instanceof Error ? error.message : String(error) }) });

          return { cut: true };
        });

        if (!answer.cut) deadline = Math.max(answer.askAgainAt, Date.now() + 1);
      }

      if (deadline !== null && Date.now() >= deadline) {
        await readOn(seen);

        if (resident !== null) await interruptProgram(resident.place, pane, options.interrupt ?? escapeInterrupt, where);

        ending = "timeout";
        return { outcome: "overran", screen: await keptScreen(turnWords.screens.overran, await screenOf(pane, where)) };
      }

      const now = await statusOf(pane, where, goneConfirm);

      if (now === null) {
        // Gone mid-turn, as herdr has said look after look, which is somebody closing it rather than
        // anything the run said. What it had written is already in the record and in the log.
        await readOn(seen);

        ending = "cancelled";
        return { outcome: "vanished", screen: "the pane went away while the turn was running" };
      }

      // Herdr said nothing either way: the record is read on and herdr asked again, and only a silence
      // that outlasts the patience ends the turn, as the silence it is.
      if (now === herdrSilent) {
        if (silentSince === null) {
          silentSince = Date.now();
          await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.herdr_silent, { cause: String(silenceCause) }) });
        }

        seen = await readOn(seen);

        if (Date.now() - silentSince >= (options.herdrPatienceMs ?? (options.home?.socket === undefined ? herdrPatienceMs : nodePatienceMs))) {
          await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.herdr_gave_up, { seconds: String(Math.round((Date.now() - silentSince) / 1_000)) }) });

          ending = "cancelled";
          return { outcome: "unanswered", screen: "herdr did not answer while the turn was running" };
        }

        await Bun.sleep(everyMs);
        continue;
      }

      // The silence's end is written as its start was, so the newest line of the job does not go on
      // saying herdr is silent (`docs/agent-control.md`, 「A silence is written down where it begins
      // and where it ends」).
      if (silentSince !== null) {
        await options.log?.write({ kind: "note", text: fillMarks(turnWords.notes.herdr_back, { seconds: String(Math.round((Date.now() - silentSince) / 1_000)), state: String(now) }) });
        silentSince = null;
      }

      // Everything the record has gained since the last look, handed on as it arrives. The lines
      // are the same shape the stream's were, so what reads them is what always read them.
      seen = await readOn(seen);
      await recordChildren();

      if (now === "working") {
        worked = true;
      }

      await questions.look(now);

      // Words owed after a refusal the screen could not carry go in once the run is back at rest,
      // and the turn goes on: what it does with them is still this turn's work.
      if ((now === "idle" || now === "done") && questions.owed !== null) {
        await checkLaunch();
        // Typed into a program at rest, the words are a prompt of their own, and the run's all the same.
        if (resident !== null) handPlaceRun(resident.place);
        const told = await promptWithLead(pane, lead, questions.owed, where);

        questions.paid();

        if (!told.ok && resident !== null) handPlaceRun(resident.place, false);

        if (told.ok) {
          answerFrom = lines(path).length;
          worked = false;
          unstartedBy = Date.now() + (options.readyMs ?? readyMs());
          await Bun.sleep(everyMs);
          continue;
        }
      }

      // Over when it has come back to rest having done something. Seeing it at work is one way to
      // know that, and the record is the other: a turn short enough to start and finish between
      // two looks is never seen working, but it has written its answer, and an answer after the
      // point this turn began is not something the idle before the prompt can have.
      if ((now === "idle" || now === "done") && (worked || answeredSince(lines(path), answerFrom))) {
        break;
      }

      if ((now === "idle" || now === "done") && !worked && Date.now() >= unstartedBy) {
        break;
      }

      // Only while it is working. A word pushed at an agent that has not started is a word the
      // turn's own prompt has not reached yet, and one pushed after it has stopped belongs to the
      // next turn rather than this one.
      if (options.aside !== undefined && now === "working") {
        await options.aside(async (text) => {
          await guardPanePrompt();

          return pushWord(text);
        });
      }

      await Bun.sleep(everyMs);
    }

    // At rest is not over. The record says when a turn is: its closing line comes after the last of
    // the answer's words, and an answer read before it can be missing all of them.
    if (answeredSince(lines(path), answerFrom)) {
      const settleBy = Date.now() + (options.settleMs ?? settleMs);

      while (!turnClosedSince(lines(path), answerFrom) && Date.now() < settleBy) {
        seen = await readOn(seen);
        await Bun.sleep(everyMs);
      }
    }

    // A resident program holds on to what it was handed and takes it as a turn of its own once the
    // turn it was in has ended, so the round is over only once nothing it was handed is still queued
    // and every turn it owes has written its end (「Words that arrive while a turn is working」).
    // A turn that answered nothing owes nothing more, and is read below as the silent turn it is.
    // The count can expect a turn the program will never write — a notice it kept once of two queued —
    // so a program at rest whose record has stopped growing has nothing still to come, and the wait
    // ends there and says what the count still expected.
    if (resident !== null && answeredSince(lines(path), from)) {
      let grown = lines(path).length;
      let quietSince = Date.now();

      while (!await cancelled() && (deadline === null || Date.now() < deadline)) {
        const round = lines(path).slice(from);

        if (residentReading(round, execution).state !== "working") break;

        seen = await readOn(seen);

        if (from + round.length !== grown) {
          grown = from + round.length;
          quietSince = Date.now();
        } else if (Date.now() - quietSince >= (options.quietMs ?? quietMs)) {
          const now = await statusOf(pane, where);

          if (now === "idle" || now === "done") {
            const { queued, owed } = residentTurnsOwed(round);
            const ended = round.filter((record) => record.type === "system" && record.subtype === "turn_duration").length;

            await options.log?.write({
              kind: "note",
              text: fillMarks(turnWords.notes.quiet, {
                seconds: String(Math.round((Date.now() - quietSince) / 1_000)),
                queued: String(queued),
                owed: String(owed),
                ended: String(ended),
              }),
            });
            break;
          }
        }

        await Bun.sleep(everyMs);
      }
    }

    await readOn(seen);

    // The round is read to its end, so no word of it is still to arrive, and a mark left by a word
    // whose submit never came would take the next person's line for the bot's.
    if (resident !== null) handPlaceRun(resident.place, false);

    const records = lines(path);

    // Back at rest with nothing written: whatever it did, there is no answer here to read, and a
    // turn counted as answered on that would record a session that does not exist. The screen goes
    // with it, since the likeliest reason is something the CLI is saying there — a transcript it
    // was told not to keep, most of all.
    if (!answeredSince(records, from)) {
      return { outcome: "silent", screen: await keptScreen(turnWords.screens.silent, await screenOf(pane, where)) };
    }

    await recordChildren();
    const read = readTurn(records, from);

    // The CLI writes its own error into the record in the answer's place, so an answer that is one
    // is not an answer: a usage limit read as a job's report would close the job on it.
    if (read.error !== null) {
      return { outcome: "errored", error: read.error };
    }

    if (resident !== null) {
      // The program stays up, so what the turn cost is its status line's running total after the
      // turn less the total before it, with the account's windows beside it (「What a resident turn
      // reads」). A reading that does not come keeps what the record says, with no price.
      // The turn ended where its Stop hook says, and the reading that follows comes about a second after.
      const stoppedAt = readHooks(resident.place).findLast((hook) => endsTurn(hook.kind) && hook.at >= promptedAt)?.at ?? Date.now();
      const around = await statusAfterTurn(resident.place, promptedAt, stoppedAt, {
        waitMs: options.accountMs ?? accountMs,
        pollMs: everyMs,
        sessionId: options.sessionId,
      });
      // The windows are the login's that made the reading, named by the reading itself — never the
      // login the caller meant to use — and a reading that names no registered login is written
      // under nobody.
      const wrote = around.login === null ? null : accountOfLogin(await claudeAccounts(archive), around.login);
      const figures = statusFigures(around.before, around.after);
      const spend = figures.money !== null && read.spend.length === 1 ? read.spend.map((row) => ({ ...row, cost: figures.money as number })) : read.spend;
      // The answer is built out of every turn the round spanned.
      const spanned = residentReading(records.slice(from), execution).reply.text;

      await closeRow("exited");
      return {
        outcome: "answered",
        reply: {
          ...replyOf(read, options.sessionId, spend),
          text: spanned === "" ? read.text : spanned,
          sessionUsage: wrote === null ? null : figures.sessionUsage,
          windows: wrote === null ? [] : figures.windows,
          contextRatio: figures.contextRatio,
          ...await loginAnswered(wrote),
          contextTokens: figures.contextTokens ?? read.contextTokens,
        },
        records: records.length,
      };
    }

    // What the run cost is written by the CLI as its process ends, so the pane is closed first and
    // the record watched for that line. A run whose account never comes keeps what its own lines
    // say, with no price.
    await closePane(pane, where);

    const spend = await accountOf(path, records.length, options.accountMs ?? accountMs);

    await closeRow("exited");
    return {
      outcome: "answered",
      reply: { ...replyOf(read, options.sessionId, spend ?? read.spend), ...await loginAnswered(null) },
      records: records.length,
    };
  } catch (error) {
    // A turn thrown out of is not a turn that exited: whatever it was doing was cut off, and the
    // row must not read as a clean ending because the throw happened to arrive first.
    ending = "cancelled";
    throw error;
  } finally {
    // A question still being put is about a pane that is going away.
    questions.withdraw();

    if (resident !== null) {
      // The program and its pane stay for the next turn; the run it was ends here, and the place
      // says so to the program's status line.
      endPlaceRun(resident.place);

      if (execution.endedAt === null) await closeRow(attemptedPrompt ? ending : "launch-error");

      await recorded("the child runs' rows", async () => native.finish());

      // A label that named the run goes back to what the agent is, once the row is closed.
      const home = options.home as AgentHome;

      if (home.resting !== undefined && home.resting !== home.label) {
        await relabelHome(home, home.resting, pane, where).catch(async () => undefined);
      }
    } else {
    // Close before archive writes: a broken archive must not strand the CLI.
    await closePane(pane, where);
    if (execution.endedAt === null) {
      // The pane is the CLI, so what herdr says about the pane is what it says about the turn's
      // process — asked here for its own answer rather than inferred from the close above, since a
      // close herdr refused and a close herdr never heard look the same from this side.
      const settled = paneTurnEnding(attemptedPrompt, ending, attemptedPrompt ? await paneStanding(pane, where) : "absent");
      execution.state = settled.state;
      // The durable half goes down before the archive's, so a collector killed between the two
      // leaves the reconciler the cause rather than only the absence.
      if (settled.ended && settled.state !== "unknown") publishEnvelope(execution.id, { owner, cause: settled.state });
      if (settled.ended) execution.endedAt = Date.now();
      await saveRow();
    }
    await recorded("the child runs' rows", async () => native.finish());
    }
  }
}

export type PaneTurnEnd = "exited" | "timeout" | "cancelled";

/**
 * What herdr says about a pane when its turn is over. Three answers, not two: the pane is gone,
 * the pane is still there, or herdr said nothing at all. Only the first is a process gone. The
 * other two both leave the row open, and they are kept apart because they are different facts —
 * one is a run that may still be working, the other is a question nobody answered.
 */
export type PaneEvidence = "absent" | "present" | "unanswered";

/**
 * What a pane turn's execution row says once its own collector has closed the pane. A turn that
 * never got its prompt did not launch; one whose pane herdr answers is gone has ended the way the
 * collector read; one whose pane herdr still holds, and one herdr does not answer about, stay
 * unknown and keep their owner alive.
 */
export function paneTurnEnding(attemptedPrompt: boolean, ending: PaneTurnEnd, pane: PaneEvidence): { state: PaneTurnEnd | "launch-error" | "unknown"; ended: boolean } {
  if (!attemptedPrompt) return { state: "launch-error", ended: true };
  return pane === "absent" ? { state: ending, ended: true } : { state: "unknown", ended: false };
}

// What herdr says about a pane. Asked after the collector has closed it, so the ordinary answer is
// that it is gone. A refused or unreachable call is not an absent pane and is not a present one
// either; it is the third answer.
async function paneStanding(pane: string, where: { socket?: string } = {}): Promise<PaneEvidence> {
  try {
    const listed = await paneListed(pane, where);
    if (listed === null) return "unanswered";
    return listed ? "present" : "absent";
  } catch {
    return "unanswered";
  }
}

// The envelope a pane turn keeps beside the supervised path's, under the same executions root.
//
// A collector cannot write anything at the moment it is killed, so what a killed collector leaves
// behind has to have been written before it died. This is that: one file, written when the pane
// opens with the identity of the collector that owns the row, and written again when the turn ends
// with how it ended. The reconciler reads both halves — the owner to say whose death left the row
// standing, the cause to close it with what the collector actually read rather than with a guess
// drawn from the pane's absence. It carries no exit code, because a pane turn's CLI is the pane's
// own process and its collector never sees one.
export interface PaneTurnEnvelope {
  owner: { pid: number; birth: string | null };
  cause?: PaneTurnEnd | "launch-error";
}

/** The envelope's file, named where it is written so that whatever reads it reads one name. */
export const PANE_ENVELOPE = "collector.json";

function processBirth(pid: number): string | null {
  try { return readFileSync(`/proc/${pid}/stat`, "utf8").split(") ").at(-1)!.split(" ")[19] ?? null; } catch { return null; }
}

// Written whole or not at all: a reader that catches this half-way through must see the old file
// or no file, never half a JSON document.
export function publishEnvelope(id: string, value: PaneTurnEnvelope): void {
  const directory = join(dataRoot(), "executions", id);
  const path = join(directory, PANE_ENVELOPE);
  try {
    mkdirSync(directory, { recursive: true, mode: 0o700 });
    writeFileSync(`${path}.new`, JSON.stringify(value), { mode: 0o600 });
    renameSync(`${path}.new`, path);
  } catch (error) {
    // The envelope is evidence for afterwards, never the turn's own path. A machine that cannot
    // write it still runs the turn; what it loses is the account of an abnormal death.
    console.error("pane execution envelope:", error);
  }
}

// The record as records, read off the file the CLI is appending to. A last line that is not JSON
// yet is a line still being written, and it is passed over rather than waited for — the next look
// will have the whole of it.
export function lines(path: string): TranscriptRecord[] {
  return transcriptRecords(path) as TranscriptRecord[];
}

// What the run said and did since the last look, in the shape a stream would have said it. The
// record's assistant and user lines are the stream's assistant and user events, so handing the
// whole record over gives the log, the job's spans and the Claude Workflow trail all at once. In
// particular, a user record's `toolUseResult` stands beside `message`, and reducing the record to
// those two fields would throw away the only structured evidence some file tools leave.
export function newLines(records: TranscriptRecord[], from: number): string {
  const fed = records
    .slice(from)
    .filter((record) => record.type === "assistant" || record.type === "user")
    .map((record) => JSON.stringify(record))
    .join("\n");

  return fed === "" ? "" : `${fed}\n`;
}

async function report(path: string, from: number, log: RunLog | null, onLine?: (line: string) => void): Promise<number> {
  const records = lines(path);

  // What the record gained is handed on line by line in the stream's own shape, which is the shape a
  // record line has, so whoever draws a run's progress reads a turn in a pane as it reads a stream.
  if (onLine !== undefined) {
    for (const record of records.slice(from)) onLine(JSON.stringify(record));
  }

  if (log !== null) {
    const fed = newLines(records, from);

    if (fed !== "") {
      await log.feed(new TextEncoder().encode(fed));
    }
  }

  return records.length;
}

// What a run cost, out of the account line its CLI wrote as it ended.
//
// The line states the whole session's running total — a resumed session's line carries every
// process before it — so what this run spent is its line less the last one written before the run
// began. A model whose total did not move was not used by this run. A process is one pane and a pane
// is one turn, so the difference is the turn's. Nothing is returned where no line was written after
// the run began, or where it names no model at all.
export function spendSince(records: TranscriptRecord[], from: number): ModelSpend[] | null {
  const accounts = records.map((record, at) => ({ record, at })).filter((one) => one.record.type === "cost-state");
  const after = accounts.filter((one) => one.at >= from).at(-1)?.record.modelUsage;
  const before = accounts.filter((one) => one.at < from).at(-1)?.record.modelUsage ?? {};

  if (after === undefined) {
    return null;
  }

  const moved: Record<string, ClaudeModelUsage> = {};

  for (const [model, total] of Object.entries(after)) {
    const earlier = before[model] ?? {};
    const less = (name: keyof ClaudeModelUsage) => Math.max(0, (total[name] ?? 0) - (earlier[name] ?? 0));
    const usage = {
      inputTokens: less("inputTokens"),
      outputTokens: less("outputTokens"),
      cacheReadInputTokens: less("cacheReadInputTokens"),
      cacheCreationInputTokens: less("cacheCreationInputTokens"),
      costUSD: less("costUSD"),
    };

    if (Object.values(usage).some((value) => value > 0)) {
      moved[model] = usage;
    }
  }

  return Object.keys(moved).length === 0 ? null : claudeSpendOf(moved);
}

async function accountOf(path: string, from: number, forMs: number): Promise<ModelSpend[] | null> {
  const deadline = Date.now() + forMs;

  for (;;) {
    const records = lines(path);

    if (records.slice(from).some((record) => record.type === "cost-state")) {
      return spendSince(records, from);
    }

    if (Date.now() >= deadline) {
      return null;
    }

    await Bun.sleep(100);
  }
}

function replyOf(turn: ReturnType<typeof readTurn>, sessionId: string, spend: ModelSpend[]): AgentReply {
  return {
    mode: "claude-code",
    model: turn.model,
    effort: turn.effort,
    text: turn.text,
    sessionId,
    // A record states what one request carried, never a share of a window, so the ratio is left
    // to whoever knows the window rather than worked out from a number that is not one.
    contextRatio: null,
    contextTokens: turn.contextTokens,
    // The account's windows are nowhere in a record. Absent is the truthful answer; a zero here
    // would read as a quota that is empty.
    sessionUsage: null,
    windows: [],
    spend,
    liveTasks: turn.liveTasks,
    workflows: turn.workflows,
  };
}

// Whether a program ended for a reset is standing again: its place's record naming a different
// process from the one just ended. The record is what says so, since a pane whose program is gone
// can still read as it did a moment before, and a prompt put in then would reach nothing. It is
// given as long as a pane is given to come up, and a turn cancelled while it waits stops here.
async function programStoodAgain(
  place: string,
  was: { pid: number; startedAt: number } | null,
  within: { forMs: number; everyMs: number; stop: () => Promise<boolean> },
): Promise<boolean> {
  const by = Date.now() + within.forMs;

  for (;;) {
    const now = readRunning(place);

    if (now !== null && (was === null || now.pid !== was.pid || now.startedAt !== was.startedAt)) return true;
    if (await within.stop() || Date.now() >= by) return false;

    await Bun.sleep(within.everyMs);
  }
}

// Whether a prompt put into a pane was submitted: the program's record gained a prompt of a person's
// after the point the turn began, or the pane went to work, within the wait. Any other line is not the
// prompt — a program resuming its session writes lines of its own as it comes up, and a pane that has
// only just come up can still read at rest from the program before it. `working` says the pane was seen
// at work, which the turn's watching counts as its own look. A pane that has gone, or one stopped on a
// question, counts as taken, so the turn's own watching reads what it is doing and says so.
async function promptTaken(
  pane: string,
  path: string,
  from: number,
  where: { socket?: string },
  timing: { waitMs: number; everyMs: number },
): Promise<"working" | "taken" | "unsent"> {
  const by = Date.now() + timing.waitMs;

  for (;;) {
    if (promptedSince(lines(path), from)) return "taken";

    const now = await statusOf(pane, where);

    if (now === "working") return "working";
    if (now === null || now === "blocked") return "taken";
    if (Date.now() >= by) return "unsent";

    await Bun.sleep(timing.everyMs);
  }
}

// The status a look reads when herdr answered none of it: no word on the pane either way, which is
// neither a pane at work nor a pane gone (`docs/agent-control.md`, 「A pane is gone only on herdr's
// own word」). A caller that only compares against the statuses it waits for reads it as neither.
export const herdrSilent = "herdr-silent";

// How a pane found missing is confirmed gone: looks in a row, each herdr answering without it.
const goneLooks: number = declaredTurn.gone_looks;
const goneGapMs: number = declaredTurn.gone_gap_ms;

// How long a turn waits out a herdr that answers none of its looks before ending on that silence.
export const herdrPatienceMs: number = declaredTurn.herdr_patience_seconds * 1_000;

// How long a turn on a work node waits for a herdr that does not answer: the node's link may be down
// while its program keeps working there, and the link comes back within this (docs/work-nodes.md,
// 「When a node drops」).
export const nodePatienceMs: number = declaredTurn.node_patience_seconds * 1_000;

// Why herdr last failed to answer a look, as its call said — a timeout, a socket it would not take
// — so a turn that meets the silence can write down what it was.
let silenceCause = "";

// One look: what the agent in the pane is doing, null where herdr answered that the pane is not
// there, or herdr's silence where it answered neither question.
async function lookAt(pane: string, where: { socket?: string }): Promise<string | null> {
  const answer = await listCall<{ agents: { pane_id?: string; agent_status?: string }[] }>("agent.list", where);

  if (!answer.ok) {
    silenceCause = `agent.list: ${answer.error.code}: ${answer.error.message}`;
    return herdrSilent;
  }

  const found = answer.result.agents.find((agent) => agent.pane_id === pane);

  if (found !== undefined) {
    return found.agent_status ?? "unknown";
  }

  // A pane with no agent detected in it yet is not a pane that has gone: the CLI is still drawing
  // itself. A pane herdr no longer lists at all is.
  const listed = await paneListed(pane, where);

  if (listed === null) {
    silenceCause = "pane.list did not answer";
    return herdrSilent;
  }

  return listed ? "unknown" : null;
}

// What the agent in a pane is doing; null only where herdr has said, look after look, that the pane
// is not there; `herdrSilent` where herdr did not answer.
export async function statusOf(
  pane: string,
  where: { socket?: string } = {},
  confirm: { looks?: number; gapMs?: number } = {},
): Promise<string | null> {
  let now = await lookAt(pane, where);

  for (let look = 1; now === null && look < (confirm.looks ?? goneLooks); look += 1) {
    await Bun.sleep(confirm.gapMs ?? goneGapMs);
    now = await lookAt(pane, where);
  }

  return now;
}

// One of a look's list calls, sent again once where herdr let the first time out: a stall under load
// runs a few seconds past one call's wait. A refusal or a broken connection is herdr's answer as it
// stands.
async function listCall<T>(method: "agent.list" | "pane.list", where: { socket?: string }): Promise<HerdrAnswer<T>> {
  const first = await herdrCall<T>(method, {}, where);

  return !first.ok && first.error.code === "timeout" ? herdrCall<T>(method, {}, where) : first;
}

// Whether herdr lists a pane, or nothing where it did not answer.
async function paneListed(pane: string, where: { socket?: string }): Promise<boolean | null> {
  const answer = await listCall<{ panes: { pane_id?: string }[] }>("pane.list", where);

  return answer.ok ? answer.result.panes.some((one) => one.pane_id === pane) : null;
}

// Keys pressed in a pane, as a person at its keyboard would.
function sendKeys(pane: string, keys: string[], where: { socket?: string }): Promise<HerdrAnswer<unknown>> {
  return herdrCall("pane.send_keys", { pane_id: pane, keys }, where);
}

// Waiting for a pane to be ready to be told something, and keeping what it said while we waited.
//
// The screen is read on every look, not only at the end, because the thing most worth reading is
// said by a run that is about to be gone: told to continue a session the CLI no longer has, it
// prints `No conversation found with session ID: …` and exits inside two seconds. That line is the
// only thing that tells the caller to drop the session it was holding, and a pane that has gone
// can no longer be read — so it is kept as it goes past.
//
// A question it is allowed to answer for itself is answered here, and the wait goes on: the CLI
// takes a few seconds to come up after the answer, and those seconds are inside the same bound. It
// is asked on every blocked look until it has answered, because the first look at a question may
// find it half drawn, and again once an answer has had its time to take and the question still
// stands, a bounded number of times, because keys typed may not reach the program.
async function settled(
  pane: string,
  options: {
    until: string[];
    forMs: number;
    everyMs: number;
    socket?: string;
    answer?: (screen: string) => Promise<boolean>;
    // How long an answer is given to take before the question still standing is answered again.
    answerAgainMs?: number;
    check?: () => void | Promise<unknown>;
    confirm?: { looks?: number; gapMs?: number };
    // Whether the program in the pane is the one the status is about. A status of the program before
    // it is not ready, however it reads; past the bound the status is handed back as it stands.
    drawn?: () => boolean;
  },
): Promise<{ status: string | "gone"; screen: string }> {
  const deadline = Date.now() + options.forMs;
  let screen = "";
  let answers = 0;
  let answeredAt: number | null = null;

  for (;;) {
    await options.check?.();
    const now = await statusOf(pane, { socket: options.socket }, options.confirm);

    if (now === null) {
      return { status: "gone", screen };
    }

    // Nothing to read off a screen herdr will not answer about; the wait goes on to its bound.
    if (now === herdrSilent) {
      if (Date.now() >= deadline) return { status: now, screen };

      await Bun.sleep(options.everyMs);
      continue;
    }

    const seen = await screenOf(pane, { socket: options.socket });

    if (seen.trim() !== "") {
      screen = seen;
    }

    if (options.until.includes(now) && (options.drawn?.() ?? true)) {
      return { status: now, screen };
    }

    if (Date.now() >= deadline) {
      return { status: now, screen };
    }

    const answerDue = answeredAt === null || Date.now() - answeredAt >= (options.answerAgainMs ?? redrawMs);

    if (now === "blocked" && answers < trustAnswers && answerDue && options.answer !== undefined && await options.answer(seen)) {
      answers += 1;
      answeredAt = Date.now();
    }

    await Bun.sleep(options.everyMs);
  }
}

// Accepting the folder-trust question in a pane, where that is the question on its screen.
// The log is told, since a question answered without a person is something whoever reads the run
// afterwards should find written down.
async function acceptFolderTrust(pane: string, screen: string, where: { socket?: string }, log: RunLog | null): Promise<boolean> {
  const keys = folderTrustKeys(screen);

  if (keys === null) {
    return false;
  }

  const sent = (await sendKeys(pane, keys, where)).ok;

  if (sent) {
    await log?.write({ kind: "note", text: turnWords.notes.trusted_folder });
  }

  return sent;
}

// A question put to a turn's caller, and the answer once it has come.
interface PendingConfirmation {
  asked: PaneConfirmation;
  stop: AbortController;
  answer: ConfirmAnswer | null;
}

// Answering a question in the pane the way the caller decided (`docs/agent-control.md`, 「A
// confirmation in the middle of a turn」), and the words still owed to the run where the screen could
// not carry them. The log is told either way, since a question answered by somebody other than the
// person watching is something whoever reads the run afterwards should find written down.
async function answerConfirmation(
  pane: string,
  screen: string,
  asked: PaneConfirmation,
  answer: ConfirmAnswer,
  left: Set<string>,
  where: { socket?: string },
  log: RunLog | null,
): Promise<string | null> {
  const keys = (list: string[]) => sendKeys(pane, list, where);

  if ("leave" in answer) {
    left.add(confirmationKey(asked));
    await log?.write({ kind: "note", text: turnWords.confirm.left_to_person });
    return null;
  }

  if (answer.allow) {
    const moves = asked.kind === "permission" ? choiceKeys(screen, "Yes") : null;

    if (moves === null) {
      left.add(confirmationKey(asked));
      await log?.write({ kind: "note", text: turnWords.confirm.no_allow });
      return null;
    }

    await keys([...moves, "enter"]);
    await log?.write({ kind: "note", text: turnWords.confirm.allowed });
    return null;
  }

  const moves = asked.kind === "permission" ? choiceKeys(screen, "No") : null;

  if (moves !== null && asked.amendable) {
    // The refusal row takes words of its own once `tab` opens it, and those words reach the run in
    // the refused command's place, inside the same turn. A line break would send them half typed.
    await keys([...moves, "tab"]);
    await herdrCall("pane.send_text", { pane_id: pane, text: answer.reason.replace(/\s*\n\s*/g, " ") }, where);
    await keys(["enter"]);
    await log?.write({ kind: "note", text: turnWords.confirm.refused_with_reason });
    return null;
  }

  await keys(moves === null ? ["esc"] : [...moves, "enter"]);
  await log?.write({ kind: "note", text: moves === null ? turnWords.confirm.closed : turnWords.confirm.refused });
  return answer.reason;
}

// How many times one answer is typed into a question that stays on the screen after it, before the
// question is left for a person.
const answerTries = 3;

// The questions a turn's pane puts to its caller while the turn is watched: the one out now, the
// ones left for a person, the answer each question already had, the one last typed in and not yet
// seen gone, and the words a refusal still owes the run where the screen could not carry them. A
// turn given no way to ask leaves every question for a person.
class PaneQuestions {
  private asking: PendingConfirmation | null = null;
  private readonly left = new Set<string>();
  // Every answer given in this turn, by its question: the same question again is answered with it
  // and never put to the caller a second time.
  private readonly decided = new Map<string, ConfirmAnswer>();
  private typed: { key: string; at: number; tries: number } | null = null;
  private words: string | null = null;

  constructor(
    private readonly pane: string,
    private readonly where: { socket?: string },
    private readonly log: RunLog | null,
    private readonly confirm: ((asked: PaneConfirmation, answeredElsewhere: AbortSignal) => Promise<ConfirmAnswer>) | undefined,
    private readonly redrawMs: number,
  ) {}

  // The words a refusal owes the run, until they have gone in.
  get owed(): string | null {
    return this.words;
  }

  paid(): void {
    this.words = null;
  }

  withdraw(): void {
    this.asking?.stop.abort();
  }

  // Typing an answer into the question on the screen, and remembering it was typed so the next looks
  // can see whether the question went.
  private async answer(screen: string, asked: PaneConfirmation, answer: ConfirmAnswer, tries: number): Promise<void> {
    const key = confirmationKey(asked);

    this.words = (await answerConfirmation(this.pane, screen, asked, answer, this.left, this.where, this.log)) ?? this.words;
    this.typed = this.left.has(key) ? null : { key, at: Date.now(), tries };
  }

  // Whether the answer last typed has been seen through: the question it answered gone from the
  // screen, or stood on past every try and left for a person. Otherwise the look is spent on it —
  // waiting out the redraw, or typing the same answer again.
  private async settleTyped(screen: string, asked: PaneConfirmation | null, key: string): Promise<boolean> {
    const typed = this.typed;

    if (typed === null) {
      return true;
    }

    // A screen not readable yet says nothing either way.
    if (asked === null) {
      return false;
    }

    if (typed.key !== key) {
      await this.typedGone();
      return true;
    }

    // A question just answered can still be on the screen for a look or two.
    if (Date.now() - typed.at < this.redrawMs) {
      return false;
    }

    if (typed.tries >= answerTries) {
      this.left.add(key);
      this.typed = null;
      await this.log?.write({ kind: "note", text: fillMarks(turnWords.confirm.tries, { tries: String(answerTries) }) });
      return false;
    }

    const answer = this.decided.get(key);

    if (answer === undefined) {
      this.typed = null;
      return true;
    }

    await this.log?.write({ kind: "note", text: turnWords.confirm.again });
    await this.answer(screen, asked, answer, typed.tries + 1);
    return false;
  }

  private async typedGone(): Promise<void> {
    await this.log?.write({ kind: "note", text: turnWords.confirm.answered });
    this.typed = null;
  }

  // One look at the pane, in the state herdr reads it in.
  async look(now: string): Promise<void> {
    const confirm = this.confirm;

    if (now === "blocked" && confirm !== undefined) {
      const screen = await screenOf(this.pane, this.where);
      const asked = confirmationOf(screen);
      const key = asked === null ? "" : confirmationKey(asked);
      const asking = this.asking;

      if (asking !== null && asking.answer !== null) {
        const askedKey = confirmationKey(asking.asked);

        this.decided.set(askedKey, asking.answer);
        this.asking = null;

        if (asked !== null && key === askedKey) {
          await this.answer(screen, asked, asking.answer, 1);
        } else {
          await this.log?.write({ kind: "note", text: turnWords.confirm.changed });
        }

        return;
      }

      if (!(await this.settleTyped(screen, asked, key)) || asking !== null || asked === null || this.left.has(key)) {
        return;
      }

      const given = this.decided.get(key);

      if (given !== undefined) {
        await this.log?.write({ kind: "note", text: turnWords.confirm.repeated });
        await this.answer(screen, asked, given, 1);
        return;
      }

      await this.put(screen, asked, confirm);
      return;
    }

    if (this.typed !== null) {
      await this.typedGone();
    }

    if (this.asking !== null) {
      // The pane stopped asking before the answer came: a person answered it where it stands.
      this.asking.stop.abort();
      await this.log?.write({ kind: "note", text: turnWords.confirm.closed_early });
      this.asking = null;
    }
  }

  // Putting a question to the caller and polling on while it decides.
  private async put(screen: string, asked: PaneConfirmation, confirm: (asked: PaneConfirmation, answeredElsewhere: AbortSignal) => Promise<ConfirmAnswer>): Promise<void> {
    const entry: PendingConfirmation = { asked, stop: new AbortController(), answer: null };

    this.asking = entry;
    // A question this reading knows is named by its own first line. One it does not know has
    // no question to name, so the whole of what stood there is kept here and nowhere else.
    await this.log?.write({
      kind: "note",
      text:
        asked.kind === "permission"
          ? fillMarks(turnWords.confirm.stopped_at, { question: asked.text.split("\n").find((row) => row.trim() !== "")?.trim() ?? "" })
          : fillMarks(turnWords.screens.unknown_window, { screen }),
    });
    confirm(asked, entry.stop.signal).then(
      (answer) => {
        entry.answer = answer;
      },
      async (error: unknown) => {
        await this.log?.write({ kind: "note", text: fillMarks(turnWords.confirm.unasked, { why: error instanceof Error ? error.message : String(error) }) });
        entry.answer = { leave: true };
      },
    );
  }
}

// What a person looking at the pane would read. Always the read, never the scroll: the CLI draws
// on the alternate screen, so the host's scrollback is empty however much has happened.
export async function screenOf(pane: string, where: { socket?: string } = {}): Promise<string> {
  const answer = await herdrCall<{ read: { text?: string } }>("pane.read", { pane_id: pane, source: "visible" }, where);

  return answer.ok ? (answer.result.read.text ?? "") : "";
}
