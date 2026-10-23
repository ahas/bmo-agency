CREATE TABLE "usage_balances" (
  "mode" text NOT NULL,
  "account" text NOT NULL,
  "amount" double precision NOT NULL,
  "currency" text NOT NULL,
  "seen_at" bigint NOT NULL,
  PRIMARY KEY ("mode", "account")
);
