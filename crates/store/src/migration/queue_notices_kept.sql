-- An item awaiting a launch is told once for each stretch it waits, and an ended job's item left leased once for
-- each lease (docs/agent-control.md, 「An item's change is told to its orderer」): a row of either notice still
-- standing is kept, due no more once told.
UPDATE "notices"
   SET "keep" = 'kept',
       "due_at" = CASE WHEN "told_at" IS NULL THEN "due_at" ELSE NULL END
 WHERE ("key" LIKE 'item-awaits-launch:%' OR "key" LIKE 'job-awaits-close:%')
   AND "route" = 'instruction' AND "keep" = 'standing';

-- A close notice is owed once for each lease, its row marked with the lease: a row told before is marked with the
-- lease its ended job still keeps, so it is not told again for it.
UPDATE "notices"
   SET "mark" = "jobs"."lease_id"::text
  FROM "jobs"
 WHERE "notices"."key" = 'job-awaits-close:' || "jobs"."item_id"
   AND "notices"."mark" = '' AND "jobs"."state" = 'ended' AND "jobs"."lease_kept";
