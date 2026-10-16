// The process mark a native co-worker place's waiter and caller are known by (docs/coworker-pool.md,
// 「Native co-worker places」): a process by its id and the kernel's start time, so a pid the kernel
// handed to a later process is not taken for the one that was marked. The hooks that admit a native
// call and give its place back are `bmo-agency coworker hook`'s, and the reclaim runs on the pool's own
// clock in agency's server.

import { readFileSync } from "node:fs";

export interface ProcessMark {
  pid: number;
  start: string;
}

// The kernel's start time of a process, so a pid the kernel handed to a later process is not taken
// for the one that was marked.
export function processMark(pid: number): ProcessMark | null {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
    const start = fields[19];
    return start === undefined ? null : { pid, start };
  } catch {
    return null;
  }
}

export function processAlive(mark: ProcessMark): boolean {
  return processMark(mark.pid)?.start === mark.start;
}
