import controlAnswers from "@agency/policy/control-words.toml";
import { fillMarks } from "@agency/support/marks";
import { existsSync, statSync } from "node:fs";
import { isAbsolute, join } from "node:path";

import { machineConfigDir, SEED_ACCOUNT_ID } from "@agency/archive/account-seed";

import type { Archive } from "@agency/archive/archive";
import type { ClaudeAccountRow } from "@agency/archive/claude-account-store";
import type { AgentAccount } from "./harness";

// Agency holds more than one Claude login, and each of them is an account: a name the owner gives it, a
// place in the account order, an on/off, and where its credential is reached. The Claude Code
// entry of the provider order is the list of them, walked in that order, and everything a login
// owns — its sessions, its transcripts, its usage windows, its rest — belongs to the account
// rather than to the road.
//
// **What is kept is where a credential is, never a credential.** A row carries a path and
// nothing else: no token, no cookie and no part of a credential file's body enters the archive,
// a log line, a document or a report. Whoever has to know when a login ends opens the file for
// that one moment and keeps the moment.
//
// The list is data rather than code: the owner adds and removes accounts, so it lives in the
// archive's own table, and this module is what says which names, which ids and which paths are
// allowed into it.

// How a run reaches an account's login.
//
// - `directory` — the account names a configuration home of its own, and the CLI keeps that
//   login in that directory's `.credentials.json`. Two directories are two logins with nothing
//   shared.
// - `token` — the account names a file holding the long-lived token the CLI issues, handed to a
//   run as `CLAUDE_CODE_OAUTH_TOKEN`. Such a token stands for a year, carries no refresh of its
//   own and is inference-only, so it dies rather than lapsing.
export type ClaudeCredentialKind = "directory" | "token";

export interface ClaudeCredential {
  kind: ClaudeCredentialKind;
  // The absolute path the credential is reached by: the configuration directory itself, or the
  // file the token sits in. Never the credential.
  path: string;
}

export interface ClaudeAccount extends AgentAccount {
  // What every record of this account is keyed by, and what never moves: a session, a usage
  // window and a rest all name it. A rename leaves it alone.
  id: string;
  // What the owner calls it, which is what a screen and a report show.
  name: string;
  place: number;
  enabled: boolean;
  credential: ClaudeCredential;
  // When it was registered, which is the order the list falls back to when nothing else says.
  addedAt: number;
  // The user whose account it is, once the multi-user model is in force; null where no user stands.
  userId: number | null;
}

export interface NewClaudeAccount {
  id: string;
  name: string;
  credential: ClaudeCredential;
  enabled?: boolean;
}

// What an id may be. It stands in a work key's setting, in a CLI flag and in a session record's
// key, so it is the small alphabet all three take without quoting — the same one a worker's name
// is held to, and for the same reason.
export const CLAUDE_ACCOUNT_ID = /^[a-z0-9][a-z0-9._-]{1,31}$/;

export const CLAUDE_ACCOUNT_NAME_MAX = 40;

// The file the CLI keeps a directory account's login in, under that account's own configuration
// directory. Named here because validation and the expiry reading both ask for it; its contents
// are nobody's to keep.
export const CREDENTIAL_FILE = ".credentials.json";

// The one account a fresh archive holds, and the machine's own configuration directory it points
// at, stand with the archive, whose settling writes it (`account-seed.ts`).
export { machineConfigDir, SEED_ACCOUNT_ID, SEED_ACCOUNT_NAME, seedClaudeAccount } from "@agency/archive/account-seed";

/**
 * Which account a run is recorded under: the one it names, or — where it names none — the account
 * that stands for the login this machine already had. A run naming no account goes out on that
 * login, so that is the account its session, its windows and its rest belong to, and a record
 * written before there was more than one login belongs there too.
 */
export function recordedAccount(account: string | { id: string } | null | undefined): string {
  const named = typeof account === "string" ? account : account?.id;

  return named === undefined || named === "" ? SEED_ACCOUNT_ID : named;
}

/**
 * The directory the CLI keeps an account's own records under — its transcripts above all. It is
 * the account's configuration home where it has one; a token account has none, so the CLI keeps
 * its records where the machine keeps them, which is what the launch hands such a run.
 */
export function accountRecordsDir(credential: { kind: string; path: string } | null | undefined): string {
  return credential !== null && credential !== undefined && credential.kind === "directory"
    ? credential.path
    : machineConfigDir();
}

/**
 * The same for the account an id names, which is what a record of a session carries. An id
 * nothing answers to — an account the owner has since removed — reads as the machine's own, so a reader
 * looking for a transcript looks somewhere rather than throwing.
 */
export async function recordsDirOf(archive: Archive, id: string | null | undefined): Promise<string> {
  const row = id === null || id === undefined ? null : await archive.claudeAccounts().getClaudeAccount(id);

  return accountRecordsDir(row === null ? null : { kind: row.credentialKind, path: row.credentialPath });
}

/**
 * The account an id names, as a run carries it: its id and where its credential is reached, which
 * is what the launch needs to put the run on that login. Nothing where the id names no account —
 * a run carrying none goes out on the machine's own login, which is what every run did before
 * there was more than one.
 */
export async function runAccountFor(archive: Archive, id: string | null | undefined): Promise<AgentAccount | null> {
  const row = id === null || id === undefined ? null : await archive.claudeAccounts().getClaudeAccount(id);

  return row === null ? null : { id: row.id, name: row.name, credential: { kind: row.credentialKind, path: row.credentialPath } };
}

/**
 * Where a directory account's login sits. Opened only to learn when it ends. The shape it takes is
 * the neutral one a run carries, so a reader holding a run's own login needs no translation.
 */
export function credentialFilePath(credential: { kind: string; path: string }): string {
  return credential.kind === "token" ? credential.path : join(credential.path, CREDENTIAL_FILE);
}

/**
 * The id minted for a new account, from the name the owner gave it. An id is never the owner's to type: it is
 * what the records key on, so it is made once and left alone, and a name with nothing usable in
 * it still gets one.
 */
export function claudeAccountId(name: string, taken: readonly string[]): string {
  const slug = name
    .toLowerCase()
    .replaceAll(/[^a-z0-9._-]+/g, "-")
    .replace(/^[^a-z0-9]+/, "")
    .replace(/[-._]+$/, "")
    .slice(0, 32);
  const stem = CLAUDE_ACCOUNT_ID.test(slug) ? slug : "account";

  if (!taken.includes(stem)) {
    return stem;
  }

  for (let next = 2; next < 1000; next += 1) {
    const candidate = `${stem.slice(0, 28)}-${next}`;

    if (!taken.includes(candidate)) {
      return candidate;
    }
  }

  throw new Error(controlAnswers.accounts.no_id);
}

export function claudeAccountIdFault(id: string, taken: readonly string[]): string | null {
  if (!CLAUDE_ACCOUNT_ID.test(id)) {
    return controlAnswers.accounts.id_shape;
  }

  return taken.includes(id) ? fillMarks(controlAnswers.accounts.id_taken, { id }) : null;
}

// Two accounts with one name are two rows nobody can tell apart on a screen or in a report, so
// the name is taken as well as the id.
export function claudeAccountNameFault(name: string, taken: readonly string[]): string | null {
  const given = name.trim();

  if (given === "") {
    return controlAnswers.accounts.name_empty;
  }

  if (given.length > CLAUDE_ACCOUNT_NAME_MAX) {
    return fillMarks(controlAnswers.accounts.name_long, { most: String(CLAUDE_ACCOUNT_NAME_MAX) });
  }

  return taken.includes(given) ? fillMarks(controlAnswers.accounts.name_taken, { name: given }) : null;
}

/**
 * Why a credential cannot be reached, or nothing. A directory account needs a directory that is
 * there — whether a login has been made in it yet is a different question, and one the login
 * watch answers — and a token account needs the file the token sits in. Neither is opened here.
 */
export function claudeCredentialFault(credential: ClaudeCredential): string | null {
  const path = credential.path.trim();

  if (path === "") {
    return controlAnswers.accounts.path_empty;
  }

  if (!isAbsolute(path)) {
    return controlAnswers.accounts.path_relative;
  }

  if (!existsSync(path)) {
    return controlAnswers.accounts.path_missing;
  }

  const directory = statSync(path).isDirectory();

  if (credential.kind === "directory") {
    return directory ? null : controlAnswers.accounts.path_not_directory;
  }

  return directory ? controlAnswers.accounts.path_not_file : null;
}

/**
 * What is wrong with adding an account, or nothing. The checks are one call so that the console,
 * the control API and the shell all refuse the same things for the same reasons.
 */
export function newClaudeAccountFault(
  account: NewClaudeAccount,
  held: readonly ClaudeAccount[],
): string | null {
  return (
    claudeAccountIdFault(account.id, held.map((one) => one.id)) ??
    claudeAccountNameFault(account.name, held.map((one) => one.name)) ??
    claudeCredentialFault(account.credential)
  );
}

export interface ClaudeAccountChange {
  name?: string;
  credential?: ClaudeCredential;
  enabled?: boolean;
}

/** What is wrong with changing an account, or nothing. Only what was named is judged. */
export function claudeAccountChangeFault(
  id: string,
  change: ClaudeAccountChange,
  held: readonly ClaudeAccount[],
): string | null {
  if (!held.some((one) => one.id === id)) {
    return fillMarks(controlAnswers.accounts.unknown, { id });
  }

  const others = held.filter((one) => one.id !== id);

  return (
    (change.name === undefined ? null : claudeAccountNameFault(change.name, others.map((one) => one.name))) ??
    (change.credential === undefined ? null : claudeCredentialFault(change.credential)) ??
    (change.enabled === false && !others.some((one) => one.enabled) ? lastAccountOffReason : null)
  );
}

export const lastAccountOffReason: string = controlAnswers.accounts.last_off;

/** The accounts as the owner ordered them, whether they are on or off. */
export async function claudeAccounts(archive: Archive): Promise<ClaudeAccount[]> {
  return (await archive.claudeAccounts().claudeAccounts()).map(accountOf);
}

/** The accounts the Claude Code entry walks, in order. */
export async function enabledClaudeAccounts(archive: Archive): Promise<ClaudeAccount[]> {
  return (await claudeAccounts(archive)).filter((account) => account.enabled);
}

/**
 * One account by its id, or by the name the owner calls it. A shell asks by whichever the owner typed; the
 * records only ever ask by id.
 */
export async function claudeAccount(archive: Archive, named: string): Promise<ClaudeAccount | null> {
  const held = await claudeAccounts(archive);

  return held.find((one) => one.id === named) ?? held.find((one) => one.name === named.trim()) ?? null;
}

/**
 * An account added, or the refusal. The path is stored as it was given: a relative one is refused
 * rather than resolved, because the directory a run happens to stand in is nobody's statement
 * about where a login lives. The name is stored with its edges trimmed.
 */
export async function addClaudeAccount(archive: Archive, account: NewClaudeAccount): Promise<ClaudeAccount> {
  const asked: NewClaudeAccount = {
    ...account,
    name: account.name.trim(),
    credential: { kind: account.credential.kind, path: account.credential.path.trim() },
  };
  const fault = newClaudeAccountFault(asked, await claudeAccounts(archive));

  if (fault !== null) {
    throw new Error(fault);
  }

  await archive.claudeAccounts().addClaudeAccount({
    id: asked.id,
    name: asked.name,
    enabled: asked.enabled ?? true,
    credentialKind: asked.credential.kind,
    credentialPath: asked.credential.path,
  });

  return await claudeAccount(archive, asked.id) as ClaudeAccount;
}

/** An account named by the owner, minted its own id. */
export async function addNamedClaudeAccount(
  archive: Archive,
  account: { name: string; credential: ClaudeCredential; enabled?: boolean },
): Promise<ClaudeAccount> {
  return addClaudeAccount(archive, {
    ...account,
    id: claudeAccountId(account.name, (await claudeAccounts(archive)).map((one) => one.id)),
  });
}

/** What the owner changed about an account, or the refusal. */
export async function updateClaudeAccount(archive: Archive, id: string, change: ClaudeAccountChange): Promise<ClaudeAccount> {
  const asked: ClaudeAccountChange = {
    ...change,
    ...(change.name === undefined ? {} : { name: change.name.trim() }),
    ...(change.credential === undefined
      ? {}
      : { credential: { kind: change.credential.kind, path: change.credential.path.trim() } }),
  };
  const fault = claudeAccountChangeFault(id, asked, await claudeAccounts(archive));

  if (fault !== null) {
    throw new Error(fault);
  }

  await archive.claudeAccounts().updateClaudeAccount(id, {
    name: asked.name,
    enabled: asked.enabled,
    credentialKind: asked.credential?.kind,
    credentialPath: asked.credential?.path,
  });
  await letChoiceGoIfOff(archive);

  return await claudeAccount(archive, id) as ClaudeAccount;
}

// The order the Claude Code entry walks its accounts in. It is the accounts' own rows read in
// order, and that is the whole of why the sequence and the switches cannot disagree: there is no
// second statement of either to fall out of step with the list of accounts itself. The providers
// are a list the code states and the owner's order is a change to it, which is why that one is a stored
// value the bot removes once it matches the default; the accounts are the owner's own list from the start,
// so there is nothing to remove — putting the order back to the default is a reset.

/** One account turned on or off, its place and every other account's left where they are. */
export async function setClaudeAccountEnabled(archive: Archive, id: string, enabled: boolean): Promise<void> {
  await updateClaudeAccount(archive, id, { enabled });
}

// The account the owner chose to run on now. Choosing it makes it the login in use, and wherever the
// login in use cannot answer the walk starts on it and goes on through the order from the top, so
// choosing one moves where every automatic run goes without touching the order the owner dragged: it is
// one setting naming one account by id beside the rows, and nothing else says it.
// Only an account that is on can be chosen, and the choice goes with the account the moment that
// account is turned off or removed, so turning it back on later does not quietly bring it back.

const chosenAccountKey = "claude_account_chosen";

export const chosenAccountOffReason: string = controlAnswers.accounts.chosen_off;

// The login in use. A login in use is used to the end: every automatic run starts on it for as long
// as it answers, rather than moving back the moment an account ahead of it or the chosen one answers
// again, since the sessions and warm caches belong to the login that opened them and the owner spends one
// account's window before the next. One setting naming one account, so a restart starts every run
// where the road stood; the owner's choice moves it.
const inUseAccountKey = "claude_account_in_use";

/** The login in use, or nothing where none is held or the one held is no longer on. */
export async function claudeAccountInUse(archive: Archive): Promise<string | null> {
  const id = await archive.settings().getSetting(inUseAccountKey);

  return id !== null && (await enabledClaudeAccounts(archive)).some((one) => one.id === id) ? id : null;
}

/** That the road's runs go out on this login now, written only where it moved. */
export async function holdClaudeAccount(archive: Archive, id: string): Promise<void> {
  if (await archive.settings().getSetting(inUseAccountKey) !== id) {
    await archive.settings().setSetting(inUseAccountKey, id);
  }
}

/** Why an account cannot be chosen to run on now, or nothing. */
export function chooseClaudeAccountFault(id: string, held: readonly ClaudeAccount[]): string | null {
  const account = held.find((one) => one.id === id);

  if (account === undefined) {
    return fillMarks(controlAnswers.accounts.unknown, { id });
  }

  return account.enabled ? null : chosenAccountOffReason;
}

/** The account the walk starts on, or nothing where it walks the order from the top. */
export async function chosenClaudeAccount(archive: Archive): Promise<ClaudeAccount | null> {
  const id = await archive.settings().getSetting(chosenAccountKey);

  return id === null ? null : ((await enabledClaudeAccounts(archive)).find((one) => one.id === id) ?? null);
}

/** One account chosen to run on now, or the choice let go with nothing. */
export async function chooseClaudeAccount(archive: Archive, id: string | null): Promise<void> {
  // Letting the choice go hands the road back to the order, so the login in use goes with it and
  // the next run starts on the first open account of the order.
  if (id === null) {
    await archive.settings().deleteSetting(inUseAccountKey);
    await archive.settings().deleteSetting(chosenAccountKey);

    return;
  }

  const fault = chooseClaudeAccountFault(id, await claudeAccounts(archive));

  if (fault !== null) {
    throw new Error(fault);
  }

  // A choice is the owner saying where runs go now: it is a switch, so the login in use moves with it.
  await archive.settings().setSetting(inUseAccountKey, id);
  await archive.settings().setSetting(chosenAccountKey, id);
}

// A choice naming an account that is no longer on is let go rather than left to wake again.
async function letChoiceGoIfOff(archive: Archive): Promise<void> {
  if (await archive.settings().getSetting(chosenAccountKey) !== null && await chosenClaudeAccount(archive) === null) {
    await archive.settings().deleteSetting(chosenAccountKey);
  }
}

function accountOf(row: ClaudeAccountRow): ClaudeAccount {
  return {
    id: row.id,
    name: row.name,
    place: row.place,
    enabled: row.enabled,
    credential: { kind: row.credentialKind, path: row.credentialPath },
    addedAt: row.addedAt,
    userId: row.userId,
  };
}
