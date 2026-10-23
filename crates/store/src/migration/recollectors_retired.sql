-- No recollector stands (docs/agent-ownership.md, 「Who writes long-term memory」): every agent of the chat kind
-- writes its own long-term memory as it reads, so no companion names a model for another agent to write it
-- with, and no agent of the recollector kind, nor the summariser of the standing sheets, keeps a row, a work
-- key's road, a run, a clock, a log line or a setting.
ALTER TABLE "companion_settings" DROP COLUMN "recollector_model";

CREATE TEMPORARY TABLE "recollectors" AS
  SELECT "name" FROM "agents" WHERE "kind" = 'recollector' OR "role" = 'recollector'
  UNION SELECT unnest(ARRAY['digest', 'people-digest', 'people-standing', 'session-summary']);

DELETE FROM "clock_asks"
 WHERE "run" IN (
   SELECT "run" FROM "run_clocks"
    WHERE "agent" IN (SELECT "name" FROM "recollectors") OR "work_key" IN (SELECT "name" FROM "recollectors")
 );
DELETE FROM "run_clocks"
 WHERE "agent" IN (SELECT "name" FROM "recollectors") OR "work_key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "runs" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "agent_log" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "prefix_moves" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "room_session_turns" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "room_sessions" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "session_resets" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "agent_base_prompts" WHERE "agent" IN (SELECT "name" FROM "recollectors");
DELETE FROM "companion_settings" WHERE "agent" IN (SELECT "name" FROM "recollectors");

DELETE FROM "model_tier_assignments" WHERE "key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "model_tier_backfilled" WHERE "key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "model_tier_conflicts" WHERE "key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "model_tier_legacy" WHERE "key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "fallback_routes" WHERE "key" IN (SELECT "name" FROM "recollectors");
DELETE FROM "fallback_sessions" WHERE "key" IN (SELECT "name" FROM "recollectors");

DELETE FROM "settings"
 WHERE EXISTS (
   SELECT 1 FROM "recollectors"
    WHERE "settings"."key" LIKE 'agent_gate:' || "recollectors"."name" || ':%'
       OR "settings"."key" IN ('agent_sampling:' || "recollectors"."name", 'reset_point:' || "recollectors"."name")
 );

DELETE FROM "agents" WHERE "name" IN (SELECT "name" FROM "recollectors");

DROP TABLE "recollectors";
