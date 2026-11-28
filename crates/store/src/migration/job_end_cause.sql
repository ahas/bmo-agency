-- Why a job's turn last ended short, as a code (docs/api.md, 「assignments and jobs」): the engine's error kind, or
-- `loop` for a job stopped for its loop; none where its turns ended as they should.
ALTER TABLE "jobs" ADD COLUMN "end_cause" text;
