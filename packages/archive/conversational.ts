// The supervisors as their rows declare them (docs/supervisors.md, 「One runner」): the scope of each one's
// session, the shape of its turns, and which of them answers the rooms that name nobody. Every reader that
// needs to know how a supervisor talks, or who answers a room that names nobody, reads it here, so nothing
// chooses a path by a supervisor's name.

import { deploymentNames } from "@agency/support/names";
import type { ArchiveSql } from "./archive-sql";

// Where a supervisor's session stands: one across every room it answers, each turn carrying its room's
// block, or one in each room it answers.
export type SessionScope = "agent" | "room";

// What a supervisor's turn is made of: the chat documents, the window's round prompt and the re-asks after
// an answer, or its settings' pieces, the companion blocks and no re-ask.
export type TurnShape = "window" | "pieces";

/** A supervisor's settings, as its row holds them. */
export interface Conversation {
  scope: SessionScope;
  shape: TurnShape;
  // Whether it answers every room that names no answerer.
  defaultAnswerer: boolean;
}

// The columns of a supervisors row the settings are read from; the number is PostgreSQL's bigint as the
// driver hands it back.
export interface ConversationColumns {
  session_scope: string | null;
  turn_shape: string | null;
  default_answerer: number | bigint | string | null;
}

/** The settings a row holds, or null where its scope is none the runner carries. */
export function conversationOf(row: ConversationColumns): Conversation | null {
  const scope = scopeNamed(row.session_scope);

  if (scope === null) return null;

  return {
    scope,
    shape: shapeNamed(row.turn_shape) ?? (scope === "agent" ? "window" : "pieces"),
    defaultAnswerer: Number(row.default_answerer ?? 0) !== 0,
  };
}

/** Whether a supervisor is a companion: one other than the one answering the unnamed rooms. */
export function isCompanion(conversation: Conversation | null): boolean {
  return conversation !== null && !conversation.defaultAnswerer;
}

/** The supervisor answering every room that names nobody, by its row, and the deployment's own where none says so. */
export async function defaultAnswerer(sql: ArchiveSql): Promise<string> {
  const rows = await sql.all("SELECT name, session_scope, turn_shape, default_answerer FROM supervisors ORDER BY name") as (ConversationColumns & { name: string })[];
  const named = rows.find((row) => conversationOf(row)?.defaultAnswerer === true);

  return named?.name ?? deploymentNames().agentKey;
}

function scopeNamed(word: string | null | undefined): SessionScope | null {
  return word === "agent" || word === "room" ? word : null;
}

function shapeNamed(word: string | null | undefined): TurnShape | null {
  return word === "window" || word === "pieces" ? word : null;
}
