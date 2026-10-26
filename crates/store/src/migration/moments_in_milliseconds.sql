-- Every run log line's moment and every ledger run's start and finish in epoch milliseconds: a moment kept in
-- epoch seconds is one below the smallest millisecond count of this century, so it is read as seconds and
-- given its milliseconds.
UPDATE "agent_log" SET "at" = "at" * 1000 WHERE "at" < 100000000000;
UPDATE "runs" SET "started_at" = "started_at" * 1000 WHERE "started_at" < 100000000000;
UPDATE "runs" SET "finished_at" = "finished_at" * 1000 WHERE "finished_at" < 100000000000;
