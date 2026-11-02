-- What the two axes retire, once its rows are carried (docs/agents.md, 「Two axes」): the agent rows, the
-- tiers and their mappings, the work keys' roads, the per-tier fold ratios, the co-worker pool's capacities,
-- definitions and warming, the sessions kept per agent, the gate role and every global order and warming
-- setting.
-- The chat agents are supervisors and the gates are retired: neither is a role. A role's cap is a whole
-- number, one where none was set.
DELETE FROM "roles" WHERE "name" IN ('chat', 'gate');
UPDATE "roles" SET "max_concurrent" = 1 WHERE "max_concurrent" IS NULL;
ALTER TABLE "roles" ALTER COLUMN "max_concurrent" SET NOT NULL;

DROP TABLE "agents";
DROP TABLE "model_tier_assignments";
DROP TABLE "model_tier_backfilled";
DROP TABLE "model_tier_conflicts";
DROP TABLE "model_tier_legacy";
DROP TABLE "model_tier_mappings";
DROP TABLE "model_tier_meta";
DROP TABLE "model_tier_seeded";
DROP TABLE "fold_ratios";
DROP TABLE "fold_ratio_seeded";
DROP TABLE "fallback_routes";
DROP TABLE "fallback_sessions";
DROP TABLE "coworker_pool_limits";
DROP TABLE "coworker_pool_meta";
DROP TABLE "coworker_pool_pings";
DROP TABLE "coworker_pool_reservation_meta";
DROP TABLE "coworker_pool_roles";
DROP TABLE "coworker_pool_tier_limits";
DROP TABLE "coworker_pool_tier_meta";
DROP TABLE "apart_sessions";
DROP TABLE "worker_sessions";
DROP TABLE "worker_words";

DELETE FROM "settings" WHERE "key" = 'provider_order'
  OR "key" LIKE 'road_kept:%'
  OR "key" LIKE 'agent_session:job-worker:%'
  OR "key" LIKE 'agent_session_shelf:job-worker:%'
  OR "key" LIKE 'worker_generation:%';
