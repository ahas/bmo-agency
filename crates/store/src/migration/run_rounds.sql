ALTER TABLE "runs" ADD COLUMN "round" text;
ALTER TABLE "runs" ADD COLUMN "message_id" bigint;

CREATE INDEX "runs_round" ON "runs" ("round");
CREATE INDEX "runs_unit" ON "runs" ("unit");
