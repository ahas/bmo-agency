-- A job's lines stand under the agent its item names, the one that did the work, rather than the checkout it ran
-- in (docs/run-records.md, 「Agent log」): the lines already kept are moved to that name, and a job that named no
-- agent keeps its lines where they stand.
UPDATE "agent_log" AS "line"
   SET "agent" = "job"."item_agent"
  FROM "jobs" AS "job"
 WHERE "line"."job_id" = "job"."id"
   AND "job"."item_agent" IS NOT NULL
   AND "job"."item_agent" <> ''
   AND "line"."agent" <> "job"."item_agent";
