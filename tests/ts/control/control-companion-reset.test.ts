import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { Archive } from "@agency/archive/archive";
import { dataRoot } from "@agency/support/roots";
import { emptyDatabase } from "@agency/test-support/store";
import { frontAlone, frontBinary } from "./control-contract-base";

import type { ControlBase } from "./control-contract-base";

// A companion's reset by range (docs/companions.md, 「Starting it afresh」; docs/api.md, 「Agents」), through
// agency's own server on a store of the test's. Each test stands a companion of its own: a room it
// answers alone and a room it shares, its room sessions with their kept turns and windows, its run log
// lines and their run records, and the records the registered harness keeps of the sessions run in its
// home and in its brain folder. What it remembers is the brain's, so the brain is a stand-in that writes
// down what it was asked.

const token = "secret";
// The binary is built as the file loads, so no hook spends its own time on a build.
const binary = frontBinary();
const brainRoot = join(dataRoot(), "brain");
// What the stand-in brain says it could not do, where a test gives it something to say.
let brainCouldNot: string[] = [];
const brainAsked: { path: string; body: Record<string, unknown> }[] = [];
const brain = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  async fetch(request) {
    const path = new URL(request.url).pathname;
    const body = (await request.json()) as Record<string, unknown>;

    brainAsked.push({ path, body });

    return Response.json(path === "/agents/reset" ? { took: { memories: 3, folder: true }, couldNot: brainCouldNot } : { ok: true });
  },
});

let front: ControlBase | null = null;
let archive: Archive | null = null;

interface Stood {
  agent: string;
  alone: number;
  shared: number;
  // The registered harness's records of the session run in its home, and of the one run in its brain folder.
  home: string[];
  recollector: string[];
}

interface ResetAnswer {
  agent: string;
  label: string;
  ranges: string[];
  alone: { chatId: number; title: string | null }[];
  shared: { chatId: number; title: string | null }[];
  rows: { table: string; range: string; rows: number }[];
  files: string[];
  brain: { took: unknown; couldNot: string[] } | null;
  failures?: string[];
}

function ask(query: Record<string, string>, authorization = `Bearer ${token}`): Promise<Response> {
  return fetch(`http://127.0.0.1:${front!.port}/companions/reset?${new URLSearchParams(query)}`, { headers: { authorization } });
}

function reset(body: Record<string, unknown>): Promise<Response> {
  return fetch(`http://127.0.0.1:${front!.port}/companions/reset`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({ by: "tester", ...body }),
  });
}

async function answered(response: Promise<Response>): Promise<ResetAnswer> {
  const done = await response;
  const body = (await done.json()) as ResetAnswer & { error?: string };

  expect([done.status, body.error]).toEqual([200, undefined]);

  return body;
}

async function refusal(response: Promise<Response>): Promise<[number, string]> {
  const done = await response;

  return [done.status, ((await done.json()) as { error: string }).error];
}

const count = async (sql: string, ...args: (string | number)[]): Promise<number> => Number(((await archive!.sql.get(`SELECT count(*) AS n FROM ${sql}`, ...args)) as { n: number }).n);

const rowsOf = (answer: ResetAnswer): Record<string, number> => Object.fromEntries(answer.rows.filter((one) => one.rows > 0).map((one) => [one.table, one.rows]));

// The records the registered harness keeps of one session run in `cwd`: its transcript and its folder.
function harnessRecords(cwd: string, session: string): string[] {
  const project = join(dataRoot(), "claude-roads", "registered", "m", "projects", cwd.replace(/[^A-Za-z0-9]/g, "-"));
  const records = [join(project, `${session}.jsonl`), join(project, session)];

  mkdirSync(records[1]!, { recursive: true });
  writeFileSync(records[0]!, "{}\n");

  return records;
}

async function stand(agent: string, rooms: number): Promise<Stood> {
  const db = archive!;
  const alone = -rooms;
  const shared = -rooms - 1;
  const run = (sql: string, ...args: (string | number)[]) => db.sql.run(sql, ...args);

  await db.agents().standCompanion({ name: agent, label: "Tester", icon: "i", provider: "registered", model: "m", itemWorker: "worker" });

  for (const chatId of [alone, shared]) {
    await run("INSERT INTO room_sessions (agent, chat_id, generation, opened_at, harness_session, harness_road) VALUES (?, ?, 1, 1, ?, 'registered::m')", agent, chatId, `s-${agent}-${chatId}`);
    await run("INSERT INTO room_session_turns (agent, chat_id, generation, position, role, text, at) VALUES (?, ?, 1, 0, 'user', 'hi', 1), (?, ?, 1, 1, 'assistant', 'hello', 2)", agent, chatId, agent, chatId);
  }

  await run("INSERT INTO room_windows (chat_id, agent, open_until) VALUES (?, ?, 1)", shared, agent);

  // Its own line in the lone room, another agent's line there, whose room's whole log goes with it, and
  // another agent's line in the shared room, which stays; each with a run record of its own.
  for (const [who, chatId, runName] of [[agent, alone, `chat:${alone}:1`], ["other", alone, `chat:${alone}:2`], ["other", shared, `chat:${shared}:1`]] as const) {
    await run("INSERT INTO agent_log (agent, run, chat_id, at, kind, text) VALUES (?, ?, ?, 1, 'prompt', 'x')", who, runName, chatId);
    await run("INSERT INTO run_records (run, kind, text, at) VALUES (?, 'answer', 'x', 1)", runName);
  }

  return {
    agent,
    alone,
    shared,
    home: harnessRecords(join(dataRoot(), "companion-homes", agent), `s-${agent}`),
    recollector: harnessRecords(join(brainRoot, "agents", agent), `r-${agent}`),
  };
}

// Made at the file's top, as a scratch store must be.
const store = await emptyDatabase();

beforeAll(async () => {
  // The server migrates the store as it stands on it; the test writes its rows once it has.
  front = await frontAlone(binary, token, { BMO_ARCHIVE_URL: store, AGENCY_BRAIN_URL: `http://127.0.0.1:${brain.port}/` });
  archive = new Archive(store);
});

afterAll(async () => {
  await front?.close();
  await archive?.close();
  await brain.stop(true);
});

describe("GET /companions/reset, POST /companions/reset", () => {
  test("shows ahead of time every row and record a range would take and the brain's preview, and changes nothing", async () => {
    const stood = await stand("tester-show", 710);

    brainAsked.length = 0;

    const both = await answered(ask({ agent: stood.agent, conversation: "true", person: "true", alone: String(stood.alone), shared: String(stood.shared) }));

    expect([both.agent, both.label, both.ranges]).toEqual([stood.agent, "Tester", ["conversation", "person"]]);
    expect([both.alone, both.shared]).toEqual([[{ chatId: stood.alone, title: null }], [{ chatId: stood.shared, title: null }]]);
    expect(rowsOf(both)).toEqual({ run_records: 2, agent_log: 2, room_session_turns: 4, room_sessions: 2, room_windows: 1 });
    expect(both.files.toSorted()).toEqual([...stood.home, ...stood.recollector].toSorted());
    expect(both.brain).toEqual({ took: { memories: 3, folder: true }, couldNot: [] });
    expect(brainAsked).toEqual([{ path: "/agents/reset", body: { agent: stood.agent, preview: true } }]);

    const conversation = await answered(ask({ agent: stood.agent, conversation: "1", alone: String(stood.alone) }));

    expect(conversation.files.toSorted()).toEqual(stood.home.toSorted());
    expect(conversation.brain).toBeNull();
    expect([await count("room_sessions WHERE agent = ?", stood.agent), await count("agent_log WHERE chat_id = ?", stood.alone)]).toEqual([2, 2]);
    expect([...stood.home, ...stood.recollector].every((path) => existsSync(path))).toBe(true);
  });

  test("of the conversation takes its rows and the lone room's whole log, ends its sessions on record, and keeps what it remembers", async () => {
    const stood = await stand("tester-talk", 720);

    await archive!.sql.run("INSERT INTO session_resets (agent, chat_id, asked_by, asked_at, reason) VALUES (?, ?, 'tester', 1, 'generation settings saved')", stood.agent, stood.alone);
    brainAsked.length = 0;

    const done = await answered(reset({ agent: stood.agent, conversation: true, alone: [stood.alone], shared: [stood.shared] }));

    expect(rowsOf(done)).toEqual({ run_records: 2, agent_log: 2, room_session_turns: 4, room_sessions: 2, room_windows: 1 });
    expect(done.failures).toEqual([]);
    expect([await count("room_sessions WHERE agent = ?", stood.agent), await count("room_session_turns WHERE agent = ?", stood.agent), await count("room_windows WHERE agent = ?", stood.agent)]).toEqual([0, 0, 0]);
    expect([await count("agent_log WHERE chat_id = ?", stood.alone), await count("agent_log WHERE chat_id = ?", stood.shared)]).toEqual([0, 1]);
    expect(await count("run_records WHERE run = ?", `chat:${stood.shared}:1`)).toBe(1);
    expect([stood.home.some((path) => existsSync(path)), stood.recollector.every((path) => existsSync(path))]).toEqual([false, true]);
    expect([done.brain, brainAsked]).toEqual([null, []]);
    expect(await archive!.sql.all("SELECT chat_id AS room, asked_by AS by, reason, CASE WHEN done_at IS NULL THEN 'waiting' ELSE 'done' END AS state FROM session_resets WHERE agent = ? ORDER BY id", stood.agent)).toEqual([
      { room: stood.alone, by: "tester", reason: "generation settings saved", state: "done" },
      { room: stood.shared, by: "tester", reason: "companion reset", state: "done" },
      { room: stood.alone, by: "tester", reason: "companion reset", state: "done" },
    ]);
  });

  test("of what it remembers has the brain take it and read its books from now on, takes its recollectors' records, and keeps the conversation", async () => {
    const stood = await stand("tester-mind", 730);
    const before = Math.floor(Date.now() / 1000);

    brainAsked.length = 0;
    brainCouldNot = ["the folder is locked"];

    const done = await answered(reset({ agent: stood.agent, person: true, alone: String(stood.alone), shared: String(stood.shared) }));

    brainCouldNot = [];
    expect([done.rows, done.files.toSorted()]).toEqual([[], stood.recollector.toSorted()]);
    expect(done.brain).toEqual({ took: { memories: 3, folder: true }, couldNot: ["the folder is locked"] });
    expect(done.failures).toEqual(["brain: the folder is locked"]);
    expect(brainAsked.map((one) => [one.path, one.body.agent, one.body.chatId ?? null, one.body.preview ?? null])).toEqual([
      ["/agents/reset", stood.agent, null, false],
      ["/agents/book-since", stood.agent, stood.alone, null],
      ["/agents/book-since", stood.agent, stood.shared, null],
    ]);
    expect(brainAsked.slice(1).every((one) => Number(one.body.at) >= before)).toBe(true);
    expect([stood.home.every((path) => existsSync(path)), stood.recollector.some((path) => existsSync(path))]).toEqual([true, false]);
    expect([await count("room_sessions WHERE agent = ?", stood.agent), await count("session_resets WHERE agent = ?", stood.agent)]).toEqual([2, 0]);
  });

  test("refuses an agent that is no companion, an ask naming no companion or no range, and a caller without the token", async () => {
    const stood = await stand("tester-none", 740);

    await archive!.sql.run("INSERT INTO agents (name, kind, session_scope, turn_shape, default_answerer) VALUES ('chat-agent', 'chat', 'agent', 'window', 1)");

    expect(await refusal(reset({ agent: "chat-agent", conversation: true }))).toEqual([400, "chat-agent is not a companion, so it cannot be reset."]);
    expect(await refusal(reset({ agent: "nobody", conversation: true }))).toEqual([400, "nobody is not a companion, so it cannot be reset."]);
    expect(await refusal(ask({ conversation: "true" }))).toEqual([400, "Name the companion to reset."]);
    expect((await refusal(reset({ agent: stood.agent, conversation: false, person: false })))[1]).toStartWith("Choose at least one range to take");
    expect((await ask({ agent: stood.agent, conversation: "true" }, "Bearer wrong")).status).toBe(401);
    expect(await count("room_sessions WHERE agent = ?", stood.agent)).toBe(2);
  });
});
