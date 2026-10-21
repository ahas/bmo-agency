-- The DeepSeek models move from the registry to the deepseek provider (docs/cache-fallback.md, 「The
-- DeepSeek road」): the pins that named them, the tiers they were mapped on, the roads that chose the
-- provider, and the worker sessions kept under it. The registrations themselves leave the registry.

UPDATE "agents" SET "pinned_provider" = 'deepseek'
 WHERE "pinned_provider" = 'registered' AND "pinned_model" IN ('deepseek-flash', 'deepseek-v4-pro');

-- The deepseek provider's own mappings are written by the seed that declares it; a mapping left on
-- registered here would name a registration this migration removes.
DELETE FROM "model_tier_mappings"
 WHERE "provider" = 'registered' AND "model" IN ('deepseek-flash', 'deepseek-v4-pro');

-- The chat's road moves to the provider. A gate's road does not: a gate is the gate service's to run,
-- and it is offered no line on this provider, so its road walks to one that carries it.
UPDATE "fallback_routes" SET "route_mode" = 'deepseek'
 WHERE "route_mode" = 'registered' AND "key" NOT IN ('voice-polish', 'voice-reading');

UPDATE "fallback_routes" SET "route_mode" = 'auto'
 WHERE "route_mode" = 'registered';

-- A worker's kept session record names the provider it answered on.
UPDATE "settings"
   SET "value" = replace("value", '"mode":"registered"', '"mode":"deepseek"')
 WHERE "key" LIKE 'agent_session:%' AND "value" LIKE '%"model":"deepseek-%';

-- The co-worker pool's starting limits on the new provider are the seed's to write, beside its tiers'
-- mappings, so a store the import fills is not taken for one already holding rows.

DELETE FROM "registered_model_addresses"
 WHERE "registration" IN ('deepseek-flash', 'deepseek-v4-pro');

DELETE FROM "registered_models"
 WHERE "name" IN ('deepseek-flash', 'deepseek-v4-pro');
