CREATE TABLE "role_agents" (
  "name" text PRIMARY KEY,
  "role" text NOT NULL,
  "provider" text,
  "model" text,
  "effort" text,
  "thinking" text,
  "answer_tokens" bigint,
  "enabled" bigint NOT NULL DEFAULT 1,
  "position" bigint NOT NULL DEFAULT 0,
  "keeps_cache" bigint NOT NULL DEFAULT 0,
  "fold_ratio" double precision,
  "prompt" text,
  "bundles" text,
  "skills" text,
  "added_at" bigint NOT NULL DEFAULT 0
);
CREATE INDEX "role_agents_role" ON "role_agents" ("role", "position");
ALTER TABLE "role_slot_requests" ADD COLUMN "seat" text;
