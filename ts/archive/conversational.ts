// The conversational agents as their rows declare them (docs/agents.md, 「One runner」): the scope of each
// one's session, the shape of its turns, and which of them answers the rooms that name nobody. Every reader
// that needs to know how an agent talks, or who answers a room that names nobody, reads it here, so nothing
// chooses a path by an agent's name or by a kind of its own.

import { deploymentNames } from "@agency/support/names";
import type { ArchiveSql } from "./archive-sql";

// Where a conversational agent's session stands: one across every room it answers, each turn carrying its
// room's block, or one in each room it answers.
export type SessionScope = "agent" | "room";

// What a conversational agent's turn is made of: the chat documents, the window's round prompt and the
// re-asks after an answer, or its settings' pieces, the companion blocks and no re-ask.
export type TurnShape = "window" | "pieces";

/** A conversational agent's settings, as its row holds them. */
export interface Conversation {
  scope: SessionScope;
  shape: TurnShape;
  // Whether it answers every room that names no answerer.
  defaultAnswerer: boolean;
}

// The columns of an agents row the settings are read from. The three settings are absent on a file the
// settling has not given them yet; the number is PostgreSQL's bigint or SQLite's integer as the driver
// hands it back.
export interface ConversationColumns {
  kind: string;
  session_scope?: string | null;
  turn_shape?: string | null;
  default_answerer?: number | bigint | string | null;
}

// The settings a companion added on the console or declared by its seed is given.
export const companionConversation: Conversation = { scope: "room", shape: "pieces", defaultAnswerer: false };

// The settings the chat agent's row is given where it holds none.
export const chatAgentConversation: Conversation = { scope: "agent", shape: "window", defaultAnswerer: true };

/**
 * The settings a row holds, or null where the row is no conversational agent's. A row an older writer left
 * with no settings is read by the kind it was written under, as the settling read every row standing when the
 * settings arrived; this is the one place that reads it so.
 */
export function conversationOf(row: ConversationColumns): Conversation | null {
  const scope = scopeNamed(row.session_scope);

  if (scope === null) return unsettled(row.kind);

  return {
    scope,
    shape: shapeNamed(row.turn_shape) ?? (scope === "agent" ? "window" : "pieces"),
    defaultAnswerer: Number(row.default_answerer ?? 0) !== 0,
  };
}

/** Whether a row is a companion's: a conversational agent other than the one answering the unnamed rooms. */
export function isCompanion(conversation: Conversation | null): boolean {
  return conversation !== null && !conversation.defaultAnswerer;
}

/**
 * The agent answering every room that names nobody, by its row. A row an older writer left without the
 * settings names none, and an archive whose rows name none answers with the deployment's own agent.
 */
export async function defaultAnswerer(sql: ArchiveSql): Promise<string> {
  // Every column, since a file the settling has not reached holds neither the settings nor the rest.
  const rows = await sql.all("SELECT * FROM agents ORDER BY name") as (ConversationColumns & { name: string; resting_since?: number | null })[];
  const named = rows.find((row) => scopeNamed(row.session_scope) !== null && (row.resting_since ?? null) === null && conversationOf(row)?.defaultAnswerer === true);

  return named?.name ?? deploymentNames().agentKey;
}

function scopeNamed(word: string | null | undefined): SessionScope | null {
  return word === "agent" || word === "room" ? word : null;
}

function shapeNamed(word: string | null | undefined): TurnShape | null {
  return word === "window" || word === "pieces" ? word : null;
}

// The settings of a row an older writer left without them, by the kind it was written under: what the
// settling gave every row of those kinds standing when the settings arrived.
function unsettled(kind: string): Conversation | null {
  switch (kind) {
    case "chat":
      return chatAgentConversation;
    case "companion":
      return companionConversation;
    default:
      return null;
  }
}
