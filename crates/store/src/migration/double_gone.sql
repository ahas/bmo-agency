-- Every slot asked for a supervisor's double, and every slot of the double's role, still standing in the line
-- or let in, finished: a waiting one cancelled, one reserved or let in released, given back by the owner.
UPDATE "role_slot_requests"
  SET "state" = CASE WHEN "state" = 'waiting' THEN 'cancelled' ELSE 'released' END,
    "ended_at" = (extract(epoch FROM now()) * 1000)::bigint,
    "outcome" = 'The supervisor''s double was removed from agency, so this slot was given back.',
    "ended_by" = 'owner'
  WHERE ("source" = 'delegate' OR "role" = 'doppelganger')
    AND "state" IN ('waiting', 'reserved', 'active');

-- The double's role and everything keyed on its name but the slots that ran under it, which stay as the
-- record of what ran.
DELETE FROM "role_slot_reservations" WHERE "role" = 'doppelganger';
DELETE FROM "role_sessions" WHERE "role" = 'doppelganger';
DELETE FROM "role_agents" WHERE "role" = 'doppelganger';
DELETE FROM "role_provider_prompts" WHERE "role" = 'doppelganger';
DELETE FROM "role_provider_prompt_seeded" WHERE "role" = 'doppelganger';
DELETE FROM "roles" WHERE "name" = 'doppelganger';
DELETE FROM "role_seeded" WHERE "name" = 'doppelganger';

-- No ask is vouched short.
ALTER TABLE "role_slot_requests" DROP COLUMN IF EXISTS "short";
