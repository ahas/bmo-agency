ALTER TABLE "jobs" ADD COLUMN "input_tokens" bigint;
ALTER TABLE "jobs" ADD COLUMN "output_tokens" bigint;
ALTER TABLE "jobs" ADD COLUMN "cache_read_tokens" bigint;
ALTER TABLE "jobs" ADD COLUMN "cache_write_tokens" bigint;
