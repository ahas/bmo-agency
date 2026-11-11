-- The co-worker roles and everything keyed on their names: no run calls them, so no slot, reservation or
-- session of theirs is waited on.
DELETE FROM "role_slot_requests" WHERE "role" IN ('verifier', 'scout', 'screener', 'builder');
DELETE FROM "role_slot_reservations" WHERE "role" IN ('verifier', 'scout', 'screener', 'builder');
DELETE FROM "role_sessions" WHERE "role" IN ('verifier', 'scout', 'screener', 'builder');
DELETE FROM "role_agents" WHERE "role" IN ('verifier', 'scout', 'screener', 'builder');
DELETE FROM "roles" WHERE "name" IN ('verifier', 'scout', 'screener', 'builder');
DELETE FROM "role_seeded" WHERE "name" IN ('verifier', 'scout', 'screener', 'builder');

-- No role or agent is one another run calls.
ALTER TABLE "roles" DROP COLUMN IF EXISTS "callable";
ALTER TABLE "role_agents" DROP COLUMN IF EXISTS "callable";

-- A native call's slot records.
DROP TABLE IF EXISTS "role_slot_native";
DROP TABLE IF EXISTS "role_slot_native_reservations";
DROP TABLE IF EXISTS "role_slot_native_stops";

-- The text a role's run is given beside its prompt by the provider its program stands on, and the record
-- of each one given by the policy, so one the owner removes stays removed.
CREATE TABLE IF NOT EXISTS "role_provider_prompts" (
  "role" text NOT NULL,
  "provider" text NOT NULL,
  "prompt" text NOT NULL,
  "updated_at" bigint NOT NULL,
  PRIMARY KEY ("role", "provider")
);
CREATE TABLE IF NOT EXISTS "role_provider_prompt_seeded" (
  "role" text NOT NULL,
  "provider" text NOT NULL,
  PRIMARY KEY ("role", "provider")
);
