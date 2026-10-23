ALTER TABLE "runs" ADD COLUMN "input_tokens" bigint;
ALTER TABLE "runs" ADD COLUMN "output_tokens" bigint;
ALTER TABLE "runs" ADD COLUMN "cache_read_tokens" bigint;
ALTER TABLE "runs" ADD COLUMN "cache_write_tokens" bigint;
