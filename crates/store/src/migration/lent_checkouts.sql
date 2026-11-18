-- A repository row that is an umbrella (docs/worker-slots.md, 「An umbrella repository」), the repositories a
-- job's item named to be lent at its launch, and when a loan's give-back began (「Loans across a job's
-- review」).
ALTER TABLE "repositories" ADD COLUMN "umbrella" bigint NOT NULL DEFAULT 0;
ALTER TABLE "jobs" ADD COLUMN "named_repositories" text;
ALTER TABLE "assignments" ADD COLUMN "returning_at" bigint;
