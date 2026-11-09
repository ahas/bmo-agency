ALTER TABLE "usage_balances" ADD COLUMN "requests_used" bigint;
ALTER TABLE "usage_balances" ADD COLUMN "requests_limit" bigint;
ALTER TABLE "usage_balances" ADD COLUMN "requests_left" bigint;
ALTER TABLE "usage_balances" ADD COLUMN "requests_scope" text;
