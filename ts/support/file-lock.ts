import { closeSync, mkdirSync, openSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";

// A lock on a file held by a process of this one's own, for as long as this process wants it
// rather than for its whole life. The holder is flock(1) keeping a pipe open: the lock is taken by
// the same flock(2) call every `flock <file> <command>` makes, so it excludes those runs as much
// as it excludes another holder, and it goes the moment the pipe closes — on release, or when this
// process dies without releasing.
//
// flock alone hands a released lock to whichever waiter the kernel wakes, so a process that lets
// the lock go and asks again at once can win it back over and over while others wait out their
// time. So every taker first stands a ticket in a queue beside the lock and goes for the lock only
// once no living ticket stands ahead of its own; the ticket stays while the lock is held and goes
// on release.
//
// A taker that may come back keeps its place when its wait runs out: its ticket passes to a
// stand-in process that lives for the seconds asked, and a taker asking again under the same key
// within them takes the ticket back where it stood, ahead of everyone who stood after it. A kept
// place holds no one up while its taker is away: the takers behind it go for the lock past it. A
// place nobody comes back for goes when its stand-in ends, as any ticket whose process is gone does.
//
// A taker may stand at the front: its ticket sorts ahead of every ordinary one, so it takes the lock
// the moment the holder lets go, whoever asked before it. Front takers among themselves keep the
// order they asked in. The holder is never cut: standing at the front waits for the lock all the
// same. A taker may stand next instead: behind every front ticket and ahead of every ordinary one,
// in the order such takers asked.

export interface HeldLock {
  release: () => Promise<void>;
}

// How often a queued taker looks whether its turn has come.
const queuePollMs = 500;

let ticketCounter = 0;

function queueDirOf(path: string): string {
  return `${path}.queue`;
}

// A ticket's name sorts by when it was stood, and carries the process that stood it. A front
// ticket's name opens with a mark that sorts ahead of every time, and a next ticket's with one that
// sorts behind the front's and still ahead of every time, which opens with a zero digit.
const frontMark = "0-";
const nextMark = "0.";

function ticketName(rank: "front" | "next" | null): string {
  ticketCounter += 1;

  const mark = rank === "front" ? frontMark : rank === "next" ? nextMark : "";

  return `${mark}${String(Date.now()).padStart(15, "0")}-${String(ticketCounter).padStart(6, "0")}-${process.pid}`;
}

function ticketPid(name: string): number {
  return Number.parseInt(name.slice(name.lastIndexOf("-") + 1), 10);
}

// The same place in the line, carried by another process.
function ticketFor(name: string, pid: number): string {
  return `${name.slice(0, name.lastIndexOf("-") + 1)}${pid}`;
}

// A ticket's file holds the name its taker stands under on its first line, and, for a taker that
// keeps its place, the key it comes back under and, once kept, the word that says so.
const keptMark = "kept";

function ticketText(holder: string, keep: LockKeep | undefined, kept: boolean): string {
  return keep === undefined ? holder : [holder, keep.key, ...(kept ? [keptMark] : [])].join("\n");
}

/** A ticket as the queue holds it. */
export interface QueueTicket {
  // The process standing it: the taker, or the stand-in keeping its place.
  pid: number;
  // The name its taker stands under; empty where it gave none.
  holder: string;
  // Whether a stand-in keeps it for a taker that gave up its wait and may come back.
  kept: boolean;
  // Whether it stands at the front, ahead of every ordinary ticket.
  front: boolean;
  // Whether it stands next: behind the front, ahead of every ordinary ticket.
  next: boolean;
}

/**
 * The living tickets of a lock's queue, in the order they stand: the holder's among them while the
 * lock is held, behind any front ticket stood since. Nothing is taken away; a ticket whose process is
 * gone is left out.
 */
export function queueTickets(path: string): QueueTicket[] {
  let names: string[];

  try {
    names = readdirSync(queueDirOf(path)).sort();
  } catch {
    return [];
  }

  return names.flatMap((name) => {
    const pid = ticketPid(name);

    if (!processAlive(pid)) return [];

    try {
      const lines = readFileSync(join(queueDirOf(path), name), "utf8").split("\n");

      return [{ pid, holder: lines[0] ?? "", kept: lines[2] === keptMark, front: name.startsWith(frontMark), next: name.startsWith(nextMark) }];
    } catch {
      return [];
    }
  });
}

/** Whether a process of this machine is still running. */
export function processAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) {
    return false;
  }

  try {
    process.kill(pid, 0);

    return true;
  } catch (error) {
    // Another user's process answers EPERM, and it is alive all the same.
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

/**
 * The living ticket standing ahead of this one, as the name it was stood under (empty where its
 * taker gave none), or undefined when none stands ahead. A kept place is passed over, since its
 * taker is away; a ticket whose process is gone is taken away.
 */
function ticketAhead(queue: string, ticket: string): string | undefined {
  let names: string[];

  try {
    names = readdirSync(queue).sort();
  } catch {
    return undefined;
  }

  for (const name of names) {
    if (name >= ticket) {
      return undefined;
    }

    if (!processAlive(ticketPid(name))) {
      rmSync(join(queue, name), { force: true });
      continue;
    }

    let lines: string[];

    try {
      lines = readFileSync(join(queue, name), "utf8").split("\n");
    } catch {
      // Let go between the listing and the read: the next look finds the queue as it now is.
      return "";
    }

    if (lines[2] !== keptMark) return lines[0] ?? "";
  }

  return undefined;
}

export interface LockWait {
  // The name this taker stands in the queue under, which a taker behind it is told while it waits.
  holder?: string;
  // Asked once the wait has run out, with the name of the taker it is waiting behind — null where
  // the lock is held from outside the queue: a later moment to wait until, or null to give up. A
  // wait that goes on keeps its place in the queue.
  waitOn?: (holder: string | null) => number | null | Promise<number | null>;
  // A place kept for a taker that comes back (above).
  keep?: LockKeep;
  // Stand at the front of the queue, ahead of every ordinary ticket (above).
  front?: boolean;
  // Stand next, behind the front and ahead of every ordinary ticket (above); the front wins where both are asked.
  next?: boolean;
  // How often a queued taker looks whether its turn has come; a test sets it short.
  pollMs?: number;
}

export interface LockKeep {
  // What a taker coming back asks under; the same key takes back a kept ticket.
  key: string;
  // How long a place is kept once the wait has run out.
  seconds: number;
}

// The kept ticket standing under the key, taken back for this process: renamed to carry it, in the
// same place, and its stand-in let go. Null where none is kept.
function takeBack(queue: string, key: string): string | null {
  let names: string[];

  try {
    names = readdirSync(queue).sort();
  } catch {
    return null;
  }

  for (const name of names) {
    let lines: string[];

    try {
      lines = readFileSync(join(queue, name), "utf8").split("\n");
    } catch {
      continue;
    }

    if (lines[1] !== key || lines[2] !== keptMark || !processAlive(ticketPid(name))) continue;

    const mine = ticketFor(name, process.pid);

    try {
      renameSync(join(queue, name), join(queue, mine));
    } catch {
      // Gone between the listing and the rename: its stand-in ended, and the place with it.
      continue;
    }

    try {
      process.kill(ticketPid(name), "SIGTERM");
    } catch {
      // Ended on its own already.
    }

    return mine;
  }

  return null;
}

// The ticket handed to a stand-in that keeps the place for the seconds asked. Answers whether it
// was kept; a ticket that could not be handed over is left to its taker to take away.
function keepPlace(queue: string, ticket: string, holder: string, keep: LockKeep): boolean {
  const standIn = Bun.spawn(["sleep", String(keep.seconds)], { stdio: ["ignore", "ignore", "ignore"], detached: true });

  standIn.unref();

  const kept = ticketFor(ticket, standIn.pid);

  try {
    writeFileSync(join(queue, ticket), ticketText(holder, keep, true));
    renameSync(join(queue, ticket), join(queue, kept));

    return true;
  } catch {
    standIn.kill();

    return false;
  }
}

/** The lock, or null when it could not be had within the wait. */
export async function takeLock(path: string, waitSeconds: number, wait: LockWait = {}): Promise<HeldLock | null> {
  closeSync(openSync(path, "a"));

  const queue = queueDirOf(path);

  mkdirSync(queue, { recursive: true });

  const holderName = wait.holder ?? "";
  const ticket = (wait.keep === undefined ? null : takeBack(queue, wait.keep.key)) ?? ticketName(wait.front === true ? "front" : wait.next === true ? "next" : null);
  const pollMs = wait.pollMs ?? queuePollMs;
  const ticketPath = join(queue, ticket);
  let deadline = Date.now() + Math.max(0, waitSeconds) * 1000;

  writeFileSync(ticketPath, ticketText(holderName, wait.keep, false));

  // Giving up the wait: the place kept where the taker asked for that, the ticket taken away otherwise.
  const giveUp = (): void => {
    if (wait.keep === undefined || !keepPlace(queue, ticket, holderName, wait.keep)) leave();
  };
  const leave = (): void => rmSync(ticketPath, { force: true });
  // Whether the wait goes on past its deadline, moving the deadline where it does.
  const waitsOn = async (holder: string | null): Promise<boolean> => {
    const later = await wait.waitOn?.(holder) ?? null;

    if (later === null || later <= Date.now()) return false;

    deadline = later;

    return true;
  };

  // A taker goes for the lock only while no living ticket stands ahead of its own, and only without
  // waiting on it: a taker blocked inside flock would be handed the lock by the kernel over a ticket
  // stood ahead of it meanwhile — a front ticket over a next one standing ahead of the holder's own.
  for (;;) {
    const ahead = ticketAhead(queue, ticket);

    if (ahead === undefined) {
      const holder = await tryHolding(path);

      if (holder !== null) return holding(holder, leave);
    }

    // Past the wait: asked whether to go on, naming the run ahead — none where the lock is held from
    // outside the queue, by a run holding it for its whole life, or by a holder standing behind this one.
    if (Date.now() >= deadline && !await waitsOn(ahead === undefined || ahead === "" ? null : ahead)) {
      giveUp();

      return null;
    }

    await Bun.sleep(Math.min(pollMs, Math.max(0, deadline - Date.now())));
  }
}

// The lock taken without waiting: the flock process keeping it, or null where another holds it.
async function tryHolding(path: string): Promise<Bun.Subprocess<"pipe", "pipe", "ignore"> | null> {
  const holder = Bun.spawn(["flock", "-n", path, "sh", "-c", "echo held; exec cat > /dev/null"], { stdin: "pipe", stdout: "pipe", stderr: "ignore" });
  const reader = holder.stdout.getReader();
  let said = "";

  while (!said.includes("\n")) {
    const { value, done } = await reader.read();

    if (done) break;

    said += new TextDecoder().decode(value);
  }

  reader.releaseLock();

  if (said.startsWith("held")) return holder;

  await holder.exited;

  return null;
}

/**
 * Any one of several locks — places of which a few may be held at once — or null when none came free
 * before `untilMs`. Each is asked without waiting, the first free one taken, and the round asked again
 * every half second until the moment passes; a moment already past asks one round.
 */
export async function takeAnyLock(paths: readonly string[], untilMs: number): Promise<HeldLock | null> {
  for (;;) {
    for (const path of paths) {
      const held = await takeLock(path, 0);

      if (held !== null) return held;
    }

    if (Date.now() + queuePollMs > untilMs) return null;

    await Bun.sleep(queuePollMs);
  }
}

// The lock as its taker holds it: the flock process kept open, and the ticket that goes with it.
function holding(holder: Bun.Subprocess<"pipe", "pipe", "ignore">, leave: () => void): HeldLock {
  let released = false;

  return {
    release: async () => {
      if (released) {
        return;
      }

      released = true;
      await holder.stdin.end();
      await holder.exited;
      leave();
    },
  };
}

// Where a checkout's job lock is kept: one file per working directory under the data root's
// locks, named so a person reading the directory can tell which checkout it belongs to.
//
// The name is two halves, and each answers something the other cannot. The slug is for a person
// reading the lock directory: it says which checkout the lock belongs to. It is lossy on purpose
// — every run of characters a file name will not take collapses to a single hyphen, and a path
// made of nothing else is left with no slug at all — so it can never be what tells two
// directories apart, and two paths differing only in those characters would otherwise queue
// behind one lock while sharing nothing. The digest is what separates them, and it is taken over
// the whole path rather than over the slug, so directories that differ anywhere differ here.
const lockSlugMax = 64;
const lockDigestChars = 12;

export function lockFileName(cwd: string): string {
  const slug = cwd.replaceAll(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "");
  // A file name is bounded and a path is not, so a long one is cut from the front: the tail is
  // the part that names the checkout, and the digest carries what the cut throws away.
  const kept = slug.slice(Math.max(0, slug.length - lockSlugMax)).replace(/^-+/, "");
  const digest = createHash("sha256").update(cwd).digest("hex").slice(0, lockDigestChars);

  return `${kept === "" ? "root" : kept}-${digest}.lock`;
}

export function workerLockPath(dataDir: string, cwd: string): string {
  return join(dataDir, "locks", lockFileName(cwd));
}

// A worker's pane lock beside the checkouts' locks, named by the worker, so two workers pointed at
// one checkout each hold their own (docs/agent-control.md, 「A worker's pane lock」).
export function workerPaneLockPath(dataDir: string, worker: string): string {
  return join(dataDir, "locks", `pane-${lockFileName(worker)}`);
}
