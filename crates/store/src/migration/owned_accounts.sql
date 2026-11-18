ALTER TABLE "claude_accounts" ADD COLUMN "node" text;
ALTER TABLE "claude_accounts" ADD COLUMN "lendable" boolean NOT NULL DEFAULT false;
