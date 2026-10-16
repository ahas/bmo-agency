import { mkdir } from "node:fs/promises";
import { dirname } from "node:path";

// The holder keeps the OS lock across preparation, local summarising and delivery, not only
// across the model process. Closing stdin releases it even when the owning runner dies.
//
// A caller that will not wait can still give the lock a moment: `waitSeconds` is for one that means
// to refuse a holder who is really working but not one who is on the way out, since a holder
// letting go is gone within a second or two and one at work is there for hours.
export async function withAgentLock<T>(
  path: string,
  nonBlocking: boolean,
  run: () => Promise<T>,
  waitSeconds?: number,
): Promise<T | null> {
  await mkdir(dirname(path), { recursive: true });
  const waiting = nonBlocking
    ? [...(waitSeconds === undefined ? ["--nonblock"] : ["--wait", String(waitSeconds)]), "--conflict-exit-code", "75"]
    : [];
  const holder = Bun.spawn(["flock", ...waiting, path,
    "sh", "-c", "printf 'locked\n'; cat >/dev/null"], { env: { ...process.env }, stdin: "pipe", stdout: "pipe", stderr: "pipe" });
  const errors = new Response(holder.stderr).text();
  const reader = holder.stdout.getReader();
  const first = await reader.read();
  reader.releaseLock();
  if (first.done || !new TextDecoder().decode(first.value).includes("locked")) {
    holder.stdin.end(); const code = await holder.exited; const error = await errors;
    if (code === 75) return null;
    throw new Error(`agent lock: ${error || code}`);
  }
  try { return await run(); }
  finally { holder.stdin.end(); await holder.exited; await errors; }
}
