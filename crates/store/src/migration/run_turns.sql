-- A run of the ledger keeps its title, and whether it is the run of a turn agency carries, which the pass over
-- runs left behind reads rather than any word of the run's name (docs/run-records.md, 「The run ledger」). The
-- chat turns' runs already kept are such runs.
ALTER TABLE "runs" ADD COLUMN IF NOT EXISTS "title" text;
ALTER TABLE "runs" ADD COLUMN IF NOT EXISTS "carried" boolean NOT NULL DEFAULT false;
UPDATE "runs" SET "carried" = true WHERE "unit" LIKE 'chat:%';
CREATE INDEX IF NOT EXISTS "runs_carried_open" ON "runs" ("id") WHERE "carried" AND "finished_at" IS NULL;
