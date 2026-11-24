-- Whether a reset of a session starts it clean (docs/session-resets.md, 「What a reset is」): the session
-- consolidates its brain and writes no handoff note, and the next session opens on none.
ALTER TABLE "session_resets" ADD COLUMN "clean" boolean NOT NULL DEFAULT false;
