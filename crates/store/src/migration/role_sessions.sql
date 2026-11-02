-- Whether a role's runs keep their sessions (docs/agent-roles.md, 「A role's sessions」): `keep` unless the
-- policy seeds the role `fresh`, which it does for the searcher, the translator and the gate roles.
ALTER TABLE "agent_roles" ADD COLUMN "sessions" text NOT NULL DEFAULT 'keep';

UPDATE "agent_roles" SET "sessions" = 'fresh' WHERE "name" IN ('searcher', 'translator', 'gate');

-- The session a run apart whose role keeps sessions left, one row an agent's command (docs/sessions.md,
-- 「A run apart's kept session」).
CREATE TABLE "apart_sessions" (
  "key" text,
  "agent" text NOT NULL,
  "line" text NOT NULL,
  "session" text NOT NULL,
  "answered_at" bigint NOT NULL,
  PRIMARY KEY ("key")
);
