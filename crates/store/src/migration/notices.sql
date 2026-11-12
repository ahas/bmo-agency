-- Agency's outbox: one row a notice owed to an orderer or the owner, keyed by its kind, sent until the
-- receiver takes it (docs/agent-control.md, 「An item's change is told to its orderer」).
CREATE SEQUENCE "notice_versions";

CREATE TABLE "notices" (
  "key" text NOT NULL,
  "route" text NOT NULL,
  "body" text NOT NULL,
  "keep" text NOT NULL,
  "mark" text NOT NULL DEFAULT '',
  "instruction_id" bigint,
  "version" bigint NOT NULL DEFAULT nextval('notice_versions'),
  "owed_at" bigint NOT NULL,
  "told_at" bigint,
  "due_at" bigint,
  "tries" bigint NOT NULL DEFAULT 0,
  "failure" text,
  PRIMARY KEY ("key")
);

CREATE INDEX "notices_due" ON "notices" ("due_at");

-- A question told before the outbox stood keeps its instruction's id on a row of its own, so a settling reply
-- still withdraws it.
INSERT INTO "notices" ("key", "route", "body", "keep", "instruction_id", "owed_at", "told_at")
SELECT 'question-awaits-answer:' || substr("key", length('question_told:') + 1), 'instruction', '{}', 'kept',
       "value"::bigint, (extract(epoch FROM now()) * 1000)::bigint, (extract(epoch FROM now()) * 1000)::bigint
  FROM "settings"
 WHERE "key" LIKE 'question\_told:%' AND "value" ~ '^[0-9]+$';

-- The marks the outbox's rows stand for now.
DELETE FROM "settings"
 WHERE "key" LIKE 'question\_told:%' OR "key" LIKE 'queue\_item\_told:%' OR "key" LIKE 'deploy\_told:%';
