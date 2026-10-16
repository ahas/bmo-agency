import declaredRest from "@agency/policy/rest.toml";

import { enabledClaudeAccounts, recordedAccount, SEED_ACCOUNT_ID } from "./claude-account";

import type { Archive } from "@agency/archive/archive";
import type { SessionUsage } from "./agent-reply";
import { mapInOrder } from "@agency/support/in-order";

// A rest belongs to one Claude account or to the service, and never to the agent as a whole. A
// limit is one login's: its window fills, that login answers nothing until the window resets, and
// every other login answers through it — so each account holds its own rest and its own moment of
// coming back, and no account's can be swallowed by another's. An outage is the service's and
// belongs to no account at all.
//
// What the rest of the program asks is still one question — "is the agent resting?" — and it is an
// aggregate: the agent is at rest when nothing will answer, which is when the service is down or
// when every enabled account has reached its own ceiling. The first login back ends the silence.
//
// This is the rest as it stands in the archive. Who is told of a rest beginning or lifting, and in
// what words, is the deployment's (`rest.ts` for agency's).
const accountRestKey = (account: string) => `rest_until:${account}`;
// A rest that names no account. An outage is always one — it is the service's — and a limit rest
// written with no login named is the login this machine already had, which is the account that
// stands for it.
const restUntilKey = "rest_until";
// Why the rest began: a usage limit or the service being down.
const restKindKey = "rest_kind";

export type RestKind = "limit" | "outage";

// The ceilings a run's usage is read against, and how long a limit that named no reset moment
// lasts. The deployment's configuration holds them.
export interface RestSettings {
  restFallbackMs: number;
  restThreshold: number;
  restWeeklyThreshold: number;
}

// How long a login is put aside when its limit named no reset moment and the caller holds no
// configuration of its own, and the ceilings a usage window is read against: the shared policy's, which
// agency's server reads too.
export const restFallbackDefaultMs = declaredRest.fallback_seconds * 1000;
export const restThresholdDefault = declaredRest.threshold;
export const restWeeklyThresholdDefault = declaredRest.weekly_threshold;

// What the rest standing now is about. An outage outranks a limit: while the service itself will
// not answer, which login's window is full says nothing about why the rooms are quiet.
export async function restKind(archive: Archive, now: number = Date.now()): Promise<RestKind> {
  return await outageRest(archive, now) !== null ? "outage" : "limit";
}

async function storedMoment(archive: Archive, key: string): Promise<number | null> {
  const value = await archive.settings().getSetting(key);

  if (value === null) {
    return null;
  }

  const parsed = Number.parseInt(value, 10);

  return Number.isNaN(parsed) ? null : parsed;
}

function standing(until: number | null, now: number): number | null {
  return until !== null && until > Math.floor(now / 1000) ? until : null;
}

/** The service's own rest, which belongs to no account, or nothing where none stands. */
export async function outageRest(archive: Archive, now: number = Date.now()): Promise<number | null> {
  return await archive.settings().getSetting(restKindKey) === "outage" ? standing(await storedMoment(archive, restUntilKey), now) : null;
}

/**
 * How long one account's own limit rest stands, or nothing. A rest written with no login named is
 * this machine's own login's, which is the account that stands for it, so that account reads its
 * own key and the unnamed one alike.
 */
export async function accountRestUntil(archive: Archive, account: string): Promise<number | null> {
  const own = await storedMoment(archive, accountRestKey(account));

  if (own !== null) {
    return own;
  }

  if (account !== SEED_ACCOUNT_ID || await archive.settings().getSetting(restKindKey) === "outage") {
    return null;
  }

  return storedMoment(archive, restUntilKey);
}

/** Whether this account will answer nothing until its own window resets. */
export async function isAccountResting(archive: Archive, account: string, now: number = Date.now()): Promise<boolean> {
  return standing(await accountRestUntil(archive, account), now) !== null;
}

/** Every login the Claude road walks, with the rest it holds. */
export async function accountRests(archive: Archive): Promise<{ account: string; until: number | null }[]> {
  return mapInOrder((await enabledClaudeAccounts(archive)), async (account) => ({
    account: account.id,
    until: await accountRestUntil(archive, account.id),
  }));
}

/**
 * When the agent answers again, or nothing where it is answering now. The service's own rest says
 * its own moment; a limit rest is over the moment the first login is back, so what stands is the
 * earliest of the accounts' deadlines, and it stands at all only while every one of them rests.
 */
export async function restUntil(archive: Archive, now: number = Date.now()): Promise<number | null> {
  const outage = await outageRest(archive, now);

  if (outage !== null) {
    return outage;
  }

  const rests = (await accountRests(archive)).map((one) => standing(one.until, now));

  if (rests.length === 0 || rests.some((until) => until === null)) {
    return null;
  }

  return Math.min(...(rests as number[]));
}

export async function isResting(archive: Archive, now: number = Date.now()): Promise<boolean> {
  return await restUntil(archive, now) !== null;
}

export function restUntilFor(settings: Pick<RestSettings, "restFallbackMs">, resetsAt: number | null): number {
  return resetsAt ?? Math.floor((Date.now() + settings.restFallbackMs) / 1000);
}

// A run that leaves a usage window past its ceiling begins a rest lasting until that window
// resets. The weekly window has its own ceiling: hitting it blocks for days, not hours.
export function restUntilAfter(settings: RestSettings, usage: SessionUsage | null): number | null {
  if (usage === null) {
    return null;
  }

  if (usage.utilization >= settings.restThreshold) {
    return usage.resetsAt;
  }

  if (
    usage.weeklyUtilization !== null &&
    usage.weeklyUtilization >= settings.restWeeklyThreshold
  ) {
    return restUntilFor(settings, usage.weeklyResetsAt);
  }

  return null;
}

/**
 * A rest written down. The answer is whether this rest is the one that silenced the agent: nothing
 * was resting before it, and nothing answers now. That is the moment a deployment tells anyone of,
 * and a rest that changed nothing of the kind — a second outage while the first goes on, an account
 * already resting, a login's window filling while another still answers — answers false.
 *
 * A limit rest is written under the account that reached its ceiling, and an account already
 * resting is left as it stands — but another account's rest never stops one being written, which
 * is the whole of what a second login is for. An outage is the service's, so a second one while
 * the same outage goes on is the same fact and writes nothing.
 */
export async function writeRest(
  archive: Archive,
  until: number,
  kind: RestKind = "limit",
  account: string = recordedAccount(null),
): Promise<boolean> {
  const wasResting = await isResting(archive);

  if (kind === "outage") {
    if (await outageRest(archive) !== null) {
      return false;
    }

    // The unnamed pair is where an outage stands, and a limit rest written there by an older
    // hand is this machine's login's: it is put under that account's own name before the pair is
    // taken, so the service's rest never swallows a login's.
    const unnamed = await archive.settings().getSetting(restKindKey) === "outage" ? null : await storedMoment(archive, restUntilKey);

    if (unnamed !== null && await storedMoment(archive, accountRestKey(SEED_ACCOUNT_ID)) === null) {
      await archive.settings().setSetting(accountRestKey(SEED_ACCOUNT_ID), String(unnamed));
    }

    await archive.settings().setSetting(restUntilKey, String(until));
    await archive.settings().setSetting(restKindKey, kind);
  } else {
    if (await isAccountResting(archive, account)) {
      return false;
    }

    await archive.settings().setSetting(accountRestKey(account), String(until));
  }

  const whose = kind === "outage" ? "the service" : `account ${account}`;
  const silenced = !wasResting && await isResting(archive);

  console.log(`${whose} rests until ${new Date(until * 1000).toISOString()}${silenced ? ": nothing answers now" : ""}`);

  return silenced;
}

