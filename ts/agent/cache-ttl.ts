import { homeDir } from "@agency/support/roots";
import { join } from "node:path";
import { agentProviderNames } from "./harness";
import { userBus } from "@agency/support/user-bus";

import declaredClaudeCode from "@agency/policy/providers/claude-code.toml";
import declaredCodex from "@agency/policy/providers/codex.toml";

import type { AgentMode } from "./agent-reply";

// How long a prompt cache entry lives, and how the watch on this machine is told.
//
// This is a fact about the API rather than about agency: a request that comes back to a session
// inside it reads its prefix, and one that comes back after it pays for the whole thing again.
// Two programs on this machine judge by it. The bot decides when to keep a session warm, and
// cache-siren — the watch that reads every Claude transcript on the machine — decides whether a
// request that lost its prefix lost it to the clock or to the prefix itself.
//
// It is written down here and nowhere else. The watch keeps its own value in its own config
// file, which is machine state rather than its source, so the number goes there from here: the
// bot writes it on every start, and the watch reads its own config exactly as it always has. So
// the direction stays the right way up — the watch is the thing underneath, seeing every session
// on this machine and knowing nothing of the bot, and what reaches it is a number in the file its
// own config mechanism already reads, not a question it asks of a thing built on top of it.

export interface PromptCachePolicy {
  ttlMs: number;
  // Null means this road keeps no resumable session for agency to ping. It may still have an API
  // cache, and that lifetime is worth publishing to a watch that can see the road.
  keepaliveIdleMs: number | null;
}

const CLAUDE_DEFAULT: PromptCachePolicy = { ttlMs: declaredClaudeCode.cache.ttl_minutes * 60_000, keepaliveIdleMs: declaredClaudeCode.cache.keepalive_idle_minutes * 60_000 };

// Longest model prefix wins. The wildcard is the contract of that mode where its provider has
// one. A provider with no row publishes no lifetime here: OpenRouter's cache belongs to whichever
// upstream a routed model reaches, and that lifetime is read by upstream below; an own LLM service
// is asked statelessly and keeps no cache at all.
const policies: Partial<Record<AgentMode, Readonly<Record<string, PromptCachePolicy>>>> = {
  "claude-code": {
    "*": CLAUDE_DEFAULT,
  },
  // Codex's lifetimes stand in its own declaration, by the model names they begin.
  codex: Object.fromEntries((declaredCodex as { cache: { models: { prefix: string; ttl_minutes: number; keepalive_idle_minutes: number }[] } }).cache.models
    .map((entry) => [entry.prefix, { ttlMs: entry.ttl_minutes * 60_000, keepaliveIdleMs: entry.keepalive_idle_minutes * 60_000 }])),
};

// The old name remains the Claude default for callers and old tests that ask about the ordinary
// Opus line. New scheduling always asks for a mode and model instead of applying this to all.
export const promptCacheTtlMs = CLAUDE_DEFAULT.ttlMs;

// The same number in the unit the watch keeps it in.
export function promptCacheTtlSeconds(): number {
  return promptCacheTtlMs / 1000;
}

// The key the watch reads it under, and the unit it runs as on this machine.
const ttlKey = "ttl_s";
const ttlByModeModelKey = "ttl_by_mode_model_s";
const sirenUnit = "cache-siren";
// Where the watch keeps that config: the config.json of the install root its own repository
// declares. It is a path on one machine rather than anything about the cache, so the environment
// may say where it went — the way it says where the archive, the collected files and the
// synthesiser's interpreter are. A machine with no watch has no file there and nothing happens.
export function sirenConfigPath(): string {
  const named = process.env.BMO_SIREN_CONFIG;

  return named === undefined || named === "" ? join(homeDir(), ".local", "lib", "cache-siren", "config.json") : named;
}

export interface TtlPublish {
  // The whole config as it should stand, or null where it already stands that way. Every other
  // key is carried over untouched: the file is another program's and this changes one number in
  // it, never the rest.
  next: Record<string, unknown> | null;
  // What it said before, or null where it said nothing — a config written before the watch had
  // the key at all falls back to the watch's own default, which is the very drift this closes.
  was: number | null;
}

export function promptCacheTtlsSeconds(): Record<AgentMode, Record<string, number>> {
  return Object.fromEntries(
    agentProviderNames.map((mode) => [
      mode,
      Object.fromEntries(Object.entries(policies[mode] ?? {}).map(([model, policy]) => [model, policy.ttlMs / 1000])),
    ]),
  ) as Record<AgentMode, Record<string, number>>;
}

function sameTtls(value: unknown, wanted: Record<AgentMode, Record<string, number>>): boolean {
  return typeof value === "object" && value !== null && JSON.stringify(value) === JSON.stringify(wanted);
}

export function ttlPublish(current: Record<string, unknown>): TtlPublish {
  const held = current[ttlKey];
  const was = typeof held === "number" ? held : null;
  const wanted = promptCacheTtlsSeconds();

  return was === promptCacheTtlSeconds() && sameTtls(current[ttlByModeModelKey], wanted)
    ? { next: null, was }
    : { next: { ...current, [ttlKey]: promptCacheTtlSeconds(), [ttlByModeModelKey]: wanted }, was };
}

export type CacheTtlOutcome = "absent" | "agreed" | "written" | "failed";

async function restartSiren(): Promise<boolean> {
  const proc = Bun.spawn(["systemctl", "--user", "restart", sirenUnit], { env: userBus(), stdout: "ignore", stderr: "ignore" });

  return (await proc.exited) === 0;
}

// Tell the watch what the cache's life is, where there is a watch on this machine to tell.
//
// Nothing happens in the ordinary case: the file already says it, and this reads one small file
// and stops. It writes only when the number moved, and then the watch is restarted, because the
// config is read once at its start — and the moment the number moved is exactly the moment the
// watch is judging by a dead one and saying so to nobody.
export async function publishCacheTtl(options: {
  path?: string;
  restart?: () => Promise<boolean>;
} = {}): Promise<CacheTtlOutcome> {
  const path = options.path ?? sirenConfigPath();
  const file = Bun.file(path);

  if (!(await file.exists())) {
    return "absent";
  }

  let current: Record<string, unknown>;

  try {
    current = (await file.json()) as Record<string, unknown>;
  } catch (error) {
    console.error(`cache siren config at ${path} could not be read: ${String(error)}`);

    return "failed";
  }

  const { next, was } = ttlPublish(current);

  if (next === null) {
    return "agreed";
  }

  try {
    await Bun.write(path, `${JSON.stringify(next, null, 1)}\n`);
  } catch (error) {
    console.error(`cache siren config at ${path} could not be written: ${String(error)}`);

    return "failed";
  }

  const restarted = await (options.restart ?? restartSiren)();

  console.log(
    `cache siren told each mode and model how long its prompt cache lives (default ${promptCacheTtlSeconds()}s; it had ${was === null ? "no value" : `${was}s`})${restarted ? "" : "; its unit did not restart"}`,
  );

  return "written";
}
