-- Each role under its new name, where no role holds that name already: a label still the one the roles
-- policy gave it takes the new name's, and the double's prompt still opening as the policy wrote it opens
-- on the double.
CREATE TEMPORARY TABLE "role_names" ("old" text, "new" text, "old_label" text, "new_label" text)
  ON COMMIT DROP;
INSERT INTO "role_names" VALUES
  ('delegator', 'doppelganger', 'Delegator', 'Doppelganger'),
  ('searcher', 'voyager', 'Searcher', 'Voyager'),
  ('browser', 'seeker', 'Browser', 'Seeker'),
  ('video', 'producer', 'Video producer', 'Producer');
DELETE FROM "role_names" WHERE "new" IN (SELECT "name" FROM "roles");

UPDATE "roles" SET "name" = "role_names"."new",
    "label" = CASE WHEN "roles"."label" = "role_names"."old_label" THEN "role_names"."new_label"
      ELSE "roles"."label" END
  FROM "role_names" WHERE "roles"."name" = "role_names"."old";
UPDATE "roles"
  SET "prompt" = 'You are the double of the supervisor that handed you this task.'
    || substr("prompt", length('You are the delegator: the double of the supervisor that handed you this task.')
      + 1)
  WHERE "name" = 'doppelganger'
    AND "prompt" LIKE 'You are the delegator: the double of the supervisor that handed you this task.%';

UPDATE "role_seeded" SET "name" = "role_names"."new" FROM "role_names"
  WHERE "role_seeded"."name" = "role_names"."old"
    AND NOT EXISTS (SELECT 1 FROM "role_seeded" "given" WHERE "given"."name" = "role_names"."new");

-- Everything keyed on a role's name.
UPDATE "role_slot_requests" SET "role" = "role_names"."new" FROM "role_names"
  WHERE "role_slot_requests"."role" = "role_names"."old";
UPDATE "role_slot_reservations" SET "role" = "role_names"."new" FROM "role_names"
  WHERE "role_slot_reservations"."role" = "role_names"."old";
UPDATE "role_sessions" SET "role" = "role_names"."new" FROM "role_names"
  WHERE "role_sessions"."role" = "role_names"."old";
UPDATE "assignments" SET "role" = "role_names"."new" FROM "role_names"
  WHERE "assignments"."role" = "role_names"."old";
UPDATE "jobs" SET "role" = "role_names"."new" FROM "role_names" WHERE "jobs"."role" = "role_names"."old";
UPDATE "branch_agents" SET "role" = "role_names"."new" FROM "role_names"
  WHERE "branch_agents"."role" = "role_names"."old";
UPDATE "agent_log" SET "agent" = "role_names"."new" FROM "role_names"
  WHERE "agent_log"."agent" = "role_names"."old";
UPDATE "runs" SET "agent" = "role_names"."new" FROM "role_names" WHERE "runs"."agent" = "role_names"."old";
UPDATE "run_clocks" SET "agent" = "role_names"."new" FROM "role_names"
  WHERE "run_clocks"."agent" = "role_names"."old";
UPDATE "session_resets" SET "agent" = "role_names"."new" FROM "role_names"
  WHERE "session_resets"."agent" = "role_names"."old";

-- The retired roles and the slots, reservations and sessions they held, none of which stands open; a role a
-- slot still stands under is left for the owner to remove.
DELETE FROM "role_slot_requests"
  WHERE "role" IN ('digest', 'helper') AND "state" NOT IN ('waiting', 'reserved', 'active');
DELETE FROM "role_slot_reservations" WHERE "role" IN ('digest', 'helper') AND "state" <> 'held';
DELETE FROM "role_sessions" WHERE "role" IN ('digest', 'helper');
DELETE FROM "roles" WHERE "name" IN ('digest', 'helper')
  AND NOT EXISTS (SELECT 1 FROM "role_slot_requests" WHERE "role_slot_requests"."role" = "roles"."name")
  AND NOT EXISTS (
    SELECT 1 FROM "role_slot_reservations" WHERE "role_slot_reservations"."role" = "roles"."name"
  );
DELETE FROM "role_seeded" WHERE "name" IN ('digest', 'helper')
  AND NOT EXISTS (SELECT 1 FROM "roles" WHERE "roles"."name" = "role_seeded"."name");
