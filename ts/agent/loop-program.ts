import { appendFileSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";

import { deploymentNames } from "@agency/support/names";
import declaredTurn from "@agency/policy/resident-turn.toml";
import { join } from "node:path";

import { harnessFailure } from "./harness";
import { guardClaudeLaunch } from "./live-claude-guard";
import { provider } from "./agent-providers";
import { isLoopSurface, loopInboxDir, loopRecordPath, readLoopRun, writeLoopRun, writeWhole, type DshTurnUsage, type LoopRecord, type LoopRoad, type LoopSurface } from "./loop-record";

import type { HarnessRun, AgentProviderName } from "./harness";

// Agency's own program standing in the pane of an agent whose provider has no terminal conversation of
// its own (docs/agent-control.md, 「The interactive surface a provider declares」). It takes a turn
// from its inbox — the bot's — or a line typed at its screen — a person's — and runs one headless
// dsh for it, showing the reasoning dsh streams to stderr as it comes and then the answer. What it
// did goes into its own record, the steps dsh took relayed into it as they are written, which is what
// the surface reads back. It takes nothing while a turn works: a line typed then waits for the turn
// to end, and the bot's next turn waits in the inbox.
// Escape at its screen ends the dsh child it is running. The road each turn goes out on — a key in
// its own environment, an adapter it opens — is its surface's, and never comes from its screen.

// One turn as the bot puts it in: its id, the run's name and what it is, how long it may take where
// the bot bounds it, and the run itself with only the variables that belong to this turn, never a
// key and never the loop's own environment.
export interface LoopTurn {
  id: string;
  name: string;
  title: string;
  timeoutMs?: number | null;
  run: HarnessRun;
}

export function putLoopTurn(place: string, turn: LoopTurn): void {
  const inbox = loopInboxDir(place);

  mkdirSync(inbox, { recursive: true, mode: 0o700 });
  writeWhole(join(inbox, `${String(Date.now()).padStart(15, "0")}-${turn.id}.json`), turn);
}

// The oldest turn waiting in the inbox, taken out of it.
export function takeLoopTurn(place: string): LoopTurn | null {
  let names: string[];

  try {
    names = readdirSync(loopInboxDir(place)).filter((name) => name.endsWith(".json")).sort();
  } catch {
    return null;
  }

  for (const name of names) {
    const path = join(loopInboxDir(place), name);

    try {
      const turn = JSON.parse(readFileSync(path, "utf8")) as LoopTurn;

      rmSync(path, { force: true });
      return turn;
    } catch {
      rmSync(path, { force: true });
    }
  }

  return null;
}

export function readLoopRecord(place: string): LoopRecord[] {
  let text: string;

  try {
    text = readFileSync(loopRecordPath(place), "utf8");
  } catch {
    return [];
  }

  return text.split("\n").flatMap((line) => {
    if (line.trim() === "") return [];

    try {
      return [JSON.parse(line) as LoopRecord];
    } catch {
      return [];
    }
  });
}

// Whether the loop's record holds a turn begun at or after `since` — when the loop standing now was
// started — that has not ended. A turn begun under an earlier loop is one that loop never finished.
export function loopTurnOpen(place: string, since: number): boolean {
  const records = readLoopRecord(place);
  const start = records.findLast((record) => record.kind === "start");

  if (start === undefined || (start.at ?? 0) < since) return false;

  return !records.some((record) => record.id === start.id && (record.kind === "end" || record.kind === "failed" || record.kind === "interrupted"));
}

function appendRecord(place: string, record: LoopRecord): void {
  mkdirSync(place, { recursive: true, mode: 0o700 });
  appendFileSync(loopRecordPath(place), `${JSON.stringify(record)}\n`, { mode: 0o600 });
}

// Where dsh files the sessions of runs made in one working directory: the path with its slashes as
// dashes, between a dash and two (measured).
function sessionsDirOf(cwd: string): string {
  return `-${cwd.replaceAll("/", "-")}--`;
}

const figure = (value: unknown): number => (typeof value === "number" && Number.isFinite(value) ? value : 0);

// The usage one line of dsh's session log carries, and whether it is a whole response's or a streamed
// chunk's. A line is `{ type, seq, time, data }`: a response's usage stands on its message, and a
// stream's in a chunk of type `usage` (measured). The share read from the cache stands apart from the input
// where the upstream reports one, and counts as none where it does not.
function usageOf(event: { type?: unknown; data?: unknown }): ({ whole: boolean } & DshTurnUsage) | null {
  const data = (typeof event.data === "object" && event.data !== null ? event.data : {}) as {
    usage?: unknown;
    message?: { usage?: unknown };
    chunk?: { type?: unknown; usage?: unknown };
  };
  const whole = event.type === "assistant/message";
  const usage = (whole ? data.message?.usage ?? data.usage : data.chunk?.type === "usage" ? data.chunk.usage : undefined) as
    | { inputTokens?: unknown; outputTokens?: unknown; cacheReadTokens?: unknown }
    | undefined;

  if (typeof usage?.inputTokens !== "number" && typeof usage?.outputTokens !== "number") return null;

  return { whole, inputTokens: figure(usage.inputTokens), outputTokens: figure(usage.outputTokens), cacheReadTokens: figure(usage.cacheReadTokens) };
}

// The session logs a turn's dsh child wrote. dsh keeps one zstd-compressed JSONL log per session
// under its home, filed by the directory the run worked in; a turn's log is one of that directory's
// written since the turn started.
function dshTurnLogs(home: string, since: number, cwd: string): string[] {
  const logs: string[] = [];
  const walk = (directory: string, depth: number): void => {
    let names: string[];

    try {
      names = readdirSync(directory);
    } catch {
      return;
    }

    for (const name of names) {
      const path = join(directory, name);

      if (name === "session.jsonl.zstd") {
        try {
          if (statSync(path).mtimeMs >= since) logs.push(path);
        } catch {
          continue;
        }
      } else if (depth < 3) {
        walk(path, depth + 1);
      }
    }
  };

  walk(join(home, "sessions", sessionsDirOf(cwd)), 0);

  return logs;
}

// The lines of one session log, each one dsh has ended. dsh writes the log as whole zstd frames, one
// for each batch of lines, so it reads while the child works; null where the log ends inside a frame
// still being written, which the next reading finds whole.
function dshLogLines(path: string): string[] | null {
  try {
    const lines = new TextDecoder().decode(Bun.zstdDecompressSync(readFileSync(path))).split("\n");

    lines.pop();
    return lines;
  } catch {
    return null;
  }
}

// What dsh's own session log says the turn's requests used. A request's usage is counted once: from
// its whole responses where the log carries any, and from its streamed usage chunks otherwise, since a
// response carries the same figures its chunks did. Null where no log was written or none carries a
// figure.
export function dshTurnUsage(home: string, since: number, cwd: string): DshTurnUsage | null {
  const whole = { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, found: false };
  const chunks = { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, found: false };

  for (const path of dshTurnLogs(home, since, cwd)) {
    for (const line of dshLogLines(path) ?? []) {
      let event: Record<string, unknown>;

      try {
        event = JSON.parse(line) as Record<string, unknown>;
      } catch {
        continue;
      }

      const usage = usageOf(event);

      if (usage === null) continue;

      const into = usage.whole ? whole : chunks;

      into.found = true;
      into.inputTokens += usage.inputTokens;
      into.outputTokens += usage.outputTokens;
      into.cacheReadTokens += usage.cacheReadTokens;
    }
  }

  const counted = whole.found ? whole : chunks;

  const { inputTokens, outputTokens, cacheReadTokens } = counted;

  return counted.found && inputTokens + outputTokens + cacheReadTokens > 0 ? { inputTokens, outputTokens, cacheReadTokens } : null;
}

// One block of what dsh's session log says a message held.
interface DshBlock {
  type?: unknown;
  text?: unknown;
  id?: unknown;
  name?: unknown;
  arguments?: unknown;
  toolCallId?: unknown;
  content?: unknown;
  isError?: unknown;
}

// A tool call's arguments as the object dsh wrote them as; text that reads as none stays the text it is.
function argumentsOf(written: unknown): unknown {
  if (typeof written !== "string") return written ?? {};

  try {
    return JSON.parse(written) as unknown;
  } catch {
    return written;
  }
}

// What a tool handed back as the text it reads as, cut to the record's length.
function answerOf(content: unknown, cut: number): string {
  const parts: unknown[] = Array.isArray(content) ? content : [content];
  const text = parts
    .map((part) => (typeof part === "string" ? part : typeof (part as DshBlock | null)?.text === "string" ? ((part as DshBlock).text as string) : ""))
    .join("\n");

  return text.length > cut ? text.slice(0, cut) : text;
}

// One line of dsh's session log as the step of the loop's record it is, written as Claude Code's record
// writes it: a response the model gave as an `assistant` line with its reasoning, its words, each tool
// call and the request's tokens, and what a tool handed back as a `user` line, cut to `cut`. Every other
// line of the log is none.
export function stepOfDsh(event: { type?: unknown; data?: unknown }, id: string, cut: number): LoopRecord | null {
  const data = (typeof event.data === "object" && event.data !== null ? event.data : {}) as {
    message?: { id?: unknown; content?: unknown; usage?: unknown };
    usage?: unknown;
  };
  const blocks = (Array.isArray(data.message?.content) ? data.message.content : []) as DshBlock[];

  if (event.type === "assistant/message") {
    const usage = (data.message?.usage ?? data.usage) as { inputTokens?: unknown; outputTokens?: unknown; cacheReadTokens?: unknown } | undefined;
    const content = blocks.flatMap((block): Record<string, unknown>[] => {
      if (block.type === "reasoning" && typeof block.text === "string") return [{ type: "thinking", thinking: block.text }];
      if (block.type === "text" && typeof block.text === "string") return [{ type: "text", text: block.text }];
      if (block.type === "tool-call" && typeof block.name === "string") return [{ type: "tool_use", id: block.id, name: block.name, input: argumentsOf(block.arguments) }];

      return [];
    });

    if (content.length === 0 && usage === undefined) return null;

    return {
      kind: "step",
      id,
      type: "assistant",
      message: {
        ...(typeof data.message?.id === "string" ? { id: data.message.id } : {}),
        content,
        // dsh counts what the cache gave apart from the input, as Claude Code's record does.
        ...(usage === undefined
          ? {}
          : { usage: { input_tokens: figure(usage.inputTokens), cache_read_input_tokens: figure(usage.cacheReadTokens), output_tokens: figure(usage.outputTokens) } }),
      },
    };
  }

  if (event.type === "tool/result") {
    const content = blocks
      .filter((block) => block.type === "tool-result")
      .map((block) => ({ type: "tool_result", tool_use_id: block.toolCallId, content: answerOf(block.content, cut), is_error: block.isError === true }));

    return content.length === 0 ? null : { kind: "step", id, type: "user", message: { content } };
  }

  return null;
}

// The steps a turn's dsh child takes, relayed out of its session logs into the loop's record as they are
// written: each log is read whole on every relay, and only the lines it gained since the last one go in.
export class StepRelay {
  private readonly relayed = new Map<string, number>();

  constructor(
    private readonly options: { home: string; since: number; cwd: string; id: string; cut: number; write: (step: LoopRecord) => void },
  ) {}

  relay(): void {
    for (const path of dshTurnLogs(this.options.home, this.options.since, this.options.cwd)) {
      const lines = dshLogLines(path);
      const from = this.relayed.get(path) ?? 0;

      if (lines === null || lines.length <= from) continue;

      for (const line of lines.slice(from)) {
        let event: { type?: unknown; data?: unknown };

        try {
          event = JSON.parse(line) as { type?: unknown; data?: unknown };
        } catch {
          continue;
        }

        const step = stepOfDsh(event, this.options.id, this.options.cut);

        if (step !== null) this.options.write(step);
      }

      this.relayed.set(path, lines.length);
    }
  }
}

const clock = new Intl.DateTimeFormat("en-GB", { timeZone: "Asia/Seoul", hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
const dim = (text: string) => `\x1b[2m${text}\x1b[0m`;

// One running dsh child, as the loop holds it.
interface Child {
  stdout: ReadableStream<Uint8Array>;
  stderr: ReadableStream<Uint8Array>;
  exited: Promise<number>;
  kill(): void;
}

export interface LoopOptions {
  // The provider whose program runs each turn: the loop stands in the pane for it.
  provider: AgentProviderName;
  place: string;
  line: HarnessRun["line"];
  model: string;
  // The loop's own environment, which every dsh child is started on.
  env: Record<string, string | undefined>;
  // The road each turn goes out on, where it is not the provider's own surface's.
  road?: LoopSurface["road"];
  // Where the screen is drawn.
  write: (text: string) => void;
  // How a dsh child is started. Its own process group, so the interrupt reaches dsh under npx.
  spawn?: (args: string[], options: { cwd: string; env: Record<string, string | undefined> }) => Child;
  pollMs?: number;
  now?: () => number;
}

function defaultSpawn(args: string[], options: { cwd: string; env: Record<string, string | undefined> }): Child {
  const proc = Bun.spawn(["setsid", ...args], { cwd: options.cwd, env: options.env, stdin: "ignore", stdout: "pipe", stderr: "pipe" });

  return {
    stdout: proc.stdout,
    stderr: proc.stderr,
    exited: proc.exited,
    kill() {
      try {
        process.kill(-proc.pid, "SIGTERM");
      } catch {
        proc.kill();
      }
    },
  };
}

export class ResidentLoop {
  private working = false;
  private child: Child | null = null;
  private interrupted = false;
  private typing = "";
  private typed: string[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;
  private turns = 0;
  private readonly now: () => number;

  constructor(private readonly options: LoopOptions) {
    this.now = options.now ?? Date.now;
  }

  // The screen it opens on, then the inbox watched until the loop is stopped.
  start(): void {
    this.options.write(`${deploymentNames().sender} loop · ${this.options.model} · ${this.options.line}\n`);
    this.rest();
    this.timer = setInterval(() => void this.next(), this.options.pollMs ?? 200);
  }

  stop(): void {
    if (this.timer !== null) clearInterval(this.timer);
    this.timer = null;
    this.child?.kill();
  }

  // Whether a turn is working, for a caller that waits on one.
  get busy(): boolean {
    return this.working;
  }

  // What arrives at the screen: Escape and Ctrl-C cut a working turn short; a line ended by Enter is
  // a person's turn; everything else is the line being typed. A key's own escape sequence — an arrow,
  // a function key — is no Escape.
  input(chunk: string): void {
    for (let at = 0; at < chunk.length; at += 1) {
      const character = chunk[at]!;

      if (character === "\x1b") {
        const next = chunk[at + 1];

        if (next === "[" || next === "O") {
          at += 2;
          while (at < chunk.length && !/[A-Za-z~]/.test(chunk[at]!)) at += 1;
          continue;
        }

        this.interrupt();
      } else if (character === "\x03") {
        this.interrupt();
      } else if (character === "\r" || character === "\n") {
        const text = this.typing.trim();

        this.typing = "";
        this.options.write("\n");

        if (text !== "") {
          this.typed.push(text);

          if (this.working) this.options.write(dim("  (put in once the run now going ends)\n"));
          else void this.next();
        }
      } else if (character === "\x7f" || character === "\b") {
        if (this.typing !== "") {
          this.typing = this.typing.slice(0, -1);
          this.options.write("\b \b");
        }
      } else if (character >= " ") {
        this.typing += character;
        this.options.write(character);
      }
    }
  }

  interrupt(): void {
    if (!this.working || this.child === null) return;

    this.interrupted = true;
    this.child.kill();
  }

  // The next turn, if one is waiting and none is working: a line typed before the bot's next turn,
  // since a person at the screen is answered first.
  async next(): Promise<void> {
    if (this.working) return;

    const typed = this.typed.shift();

    if (typed !== undefined) {
      const template = this.template();

      if (template === null) {
        this.options.write(dim("  this agent has been handed no run yet, so a typed line cannot be run\n"));
        return;
      }

      this.turns += 1;
      await this.runTurn({ id: `typed-${this.now()}-${this.turns}`, name: "typed line", title: typed.split("\n")[0]!.slice(0, 40), run: { ...template, prompt: typed } }, true);
      return;
    }

    const turn = takeLoopTurn(this.options.place);

    if (turn === null) return;

    writeLoopRun(this.options.place, turn.run);
    await this.runTurn(turn, false);
  }

  private template(): HarnessRun | null {
    return readLoopRun(this.options.place);
  }

  // The road a turn goes out on: the one the loop was handed, or its provider's surface's own.
  private road(): LoopSurface["road"] {
    if (this.options.road !== undefined) return this.options.road;

    const surface = provider(this.options.provider).resident;

    if (!isLoopSurface(surface)) throw new Error(`${this.options.provider} stands no loop`);

    return surface.road;
  }

  private rest(): void {
    this.options.write(dim(`resting · since ${clock.format(new Date(this.now()))}\n`));
  }

  private async runTurn(turn: LoopTurn, typed: boolean): Promise<void> {
    const place = this.options.place;
    const startedAt = this.now();

    this.working = true;
    this.interrupted = false;
    appendRecord(place, { kind: "start", id: turn.id, name: turn.name, title: turn.title, at: startedAt, typed });
    this.options.write(`\n── ${turn.name} · ${turn.title} · ${clock.format(new Date(startedAt))} ──\n`);

    if (!typed) this.options.write(`${turn.run.prompt.split("\n").slice(0, 3).join("\n")}\n`);

    let road: LoopRoad | null = null;

    try {
      road = this.road()(turn.run, this.options.env, turn.timeoutMs ?? null);

      if ("error" in road) {
        appendRecord(place, { kind: "failed", id: turn.id, text: road.error });
        this.options.write(`failed · ${road.error}\n`);
        return;
      }

      // The loop's own environment under the turn's own variables, and the road's over both.
      const run: HarnessRun = { ...turn.run, env: { ...this.options.env, ...turn.run.env, ...road.env } };
      const harness = provider(this.options.provider).adapter;
      const command = await harness.command(run);

      // A launch of any program goes past the one launch guard; the guard lets through what opens no
      // Claude Code session.
      await guardClaudeLaunch(run.line, command.args, command.env);
      const child = (this.options.spawn ?? defaultSpawn)(command.args, { cwd: command.cwd, env: command.env });

      this.child = child;

      // The program keeps its session logs under the home its command names, and the steps it takes go
      // into the record out of them while it works, and once more when it has ended.
      const home = command.env.DSH_HOME;
      const relay =
        home === undefined
          ? null
          : new StepRelay({ home, since: startedAt, cwd: run.cwd, id: turn.id, cut: declaredTurn.loop_relay_chars, write: (step) => appendRecord(place, step) });
      // A step that could not be relayed is left out of the record; the turn goes on, and its answer with it.
      const relayed = () => {
        try {
          relay?.relay();
        } catch {
          // Nothing of the turn depends on its steps.
        }
      };
      const relaying = relay === null ? null : setInterval(relayed, declaredTurn.loop_relay_ms);

      const decoder = (stream: ReadableStream<Uint8Array>, show: (text: string) => void): Promise<string> =>
        (async () => {
          let all = "";
          const text = new TextDecoder();
          // Read through the reader rather than iterated: the console's typecheck reads this file
          // against the DOM's stream, which declares no async iterator.
          const reader = stream.getReader();

          for (let read = await reader.read(); !read.done; read = await reader.read()) {
            const piece = text.decode(read.value, { stream: true });

            all += piece;
            show(piece);
          }

          return all;
        })();
      const [stdout, stderr, exitCode] = await Promise.all([
        decoder(child.stdout, (piece) => this.options.write(piece)),
        decoder(child.stderr, (piece) => this.options.write(dim(piece))),
        child.exited,
      ]).finally(() => {
        if (relaying !== null) clearInterval(relaying);
      });

      relayed();

      if (this.interrupted) {
        appendRecord(place, { kind: "interrupted", id: turn.id });
        this.options.write("\nstopped\n");
        return;
      }

      if (exitCode === 0 && stdout.trim() !== "") {
        const usage = home === undefined ? null : dshTurnUsage(home, startedAt, run.cwd);

        appendRecord(place, { kind: "answer", id: turn.id, text: stdout.trim(), model: run.model, ...(usage === null ? {} : { usage }) });
        appendRecord(place, { kind: "end", id: turn.id });
        this.options.write("\nended\n");
        return;
      }

      const text = harnessFailure(harness, { stdout, stderr, exitCode });

      appendRecord(place, { kind: "failed", id: turn.id, text });
      this.options.write(`\nfailed · ${text}\n`);
    } catch (error) {
      const text = error instanceof Error ? error.message : String(error);

      appendRecord(place, { kind: "failed", id: turn.id, text });
      this.options.write(`\nfailed · ${text}\n`);
    } finally {
      if (road !== null && !("error" in road)) road.close();
      this.child = null;
      this.working = false;
      this.rest();

      if (this.typed.length > 0) void this.next();
    }
  }
}
