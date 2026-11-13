-- Whether the notice of a job's end is owed yet, and the words its due review check is told with, so agency's
-- sender owes either where a stop cut it off before its row was written (docs/agent-control.md, 「An item's change
-- is told to its orderer」). A job ended before this column stood was told by the path of its end.
ALTER TABLE "jobs" ADD COLUMN "end_owed" boolean NOT NULL DEFAULT false;
UPDATE "jobs" SET "end_owed" = true WHERE "ended_at" IS NOT NULL;
ALTER TABLE "jobs" ADD COLUMN "review_told" text;
