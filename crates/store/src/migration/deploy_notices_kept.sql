-- A commit waiting for its repository's batch deployment is told once for each commit and kept until the item no
-- longer waits (docs/agent-control.md, 「An item's change is told to its orderer」): a row still standing is kept,
-- due no more once told.
UPDATE "notices"
   SET "keep" = 'kept',
       "due_at" = CASE WHEN "told_at" IS NULL THEN "due_at" ELSE NULL END
 WHERE "key" LIKE 'item-awaits-deploy:%' AND "route" = 'instruction' AND "keep" = 'standing';
