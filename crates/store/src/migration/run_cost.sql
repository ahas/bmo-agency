ALTER TABLE "runs" ADD COLUMN "cost_micros" bigint;
ALTER TABLE "runs" ADD COLUMN "supply_source" text;
ALTER TABLE "jobs" ADD COLUMN "cost_micros" bigint;
ALTER TABLE "jobs" ADD COLUMN "supply_source" text;
