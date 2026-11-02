ALTER TABLE "coworker_pool_requests" ADD COLUMN "title" text;
ALTER TABLE "coworker_pool_requests" ADD COLUMN "asked_by" text;
ALTER TABLE "coworker_pool_requests" ADD COLUMN "agent" text;

CREATE INDEX "coworker_pool_asked_by" ON "coworker_pool_requests" (source, asked_by, sequence);
