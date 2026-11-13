ALTER TABLE "role_agents" ADD COLUMN IF NOT EXISTS "owner_account" text;
ALTER TABLE "role_slot_requests" ADD COLUMN IF NOT EXISTS "account" text;
