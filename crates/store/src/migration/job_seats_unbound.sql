-- A job's seat is held for the job from its first turn to its end, bound to none of its turns
-- (docs/role-slots.md, 「The line」): a job's active lease bound to a run that is no role session is unbound, so
-- the end of the turn it names gives it back no longer.
UPDATE "role_slot_requests"
   SET "child_id" = NULL, "child_seen" = 0
 WHERE "state" = 'active'
   AND "source" = 'job'
   AND "owner" LIKE 'job-%'
   AND "child_id" IS NOT NULL
   AND "child_id" IS DISTINCT FROM "session";
