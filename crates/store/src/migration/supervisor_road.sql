-- A supervisor stands on one road, set on its row as an agent's is set on the agent, and nothing stands in for it
-- (docs/provider-order.md, 「What a road is」): each supervisor keeps the first entry of the order it held.
ALTER TABLE "supervisors" ADD COLUMN "provider" text;
ALTER TABLE "supervisors" ADD COLUMN "model" text;
ALTER TABLE "supervisors" ADD COLUMN "effort" text;
ALTER TABLE "supervisors" ADD COLUMN "thinking" text;
ALTER TABLE "supervisors" ADD COLUMN "answer_tokens" bigint;

UPDATE "supervisors" AS "s"
   SET "provider" = "first"."entry" ->> 'provider',
       "model" = "first"."entry" ->> 'model',
       "effort" = NULLIF("first"."entry" ->> 'effort', ''),
       "thinking" = NULLIF("first"."entry" ->> 'thinking', ''),
       "answer_tokens" = ("first"."entry" ->> 'answerTokens')::bigint
  FROM (SELECT "name", (NULLIF(TRIM("provider_order"), '')::jsonb) -> 0 AS "entry" FROM "supervisors"
         WHERE "provider_order" ~ '^\s*\[') AS "first"
 WHERE "first"."name" = "s"."name" AND "first"."entry" IS NOT NULL;

ALTER TABLE "supervisors" DROP COLUMN "provider_order";
ALTER TABLE "supervisors" DROP COLUMN "quota_share";
ALTER TABLE "supervisors" DROP COLUMN "active_provider";
ALTER TABLE "supervisors" DROP COLUMN "active_model";

-- The marks of a stand-in answering and the roads a supervisor's walk kept go with the walk.
DELETE FROM "settings"
 WHERE "key" IN ('fallback_since', 'fallback_provider') OR "key" LIKE 'road\_kept:supervisor:%';
