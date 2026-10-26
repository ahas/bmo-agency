-- The translator is a runner holding the translator role (docs/agent-roles.md), and its work key is
-- `translate`, which starts where the translator's key stands: the owner's road and tier carried over.
UPDATE "agents" SET "kind" = 'runner', "role" = 'translator' WHERE "name" = 'translator';

INSERT INTO "fallback_routes" ("key", "account", "claude_model", "codex_model", "codex_effort", "openrouter_model",
    "set_at", "route_mode", "forced", "registration")
  SELECT 'translate', "account", "claude_model", "codex_model", "codex_effort", "openrouter_model", "set_at",
    "route_mode", "forced", "registration"
  FROM "fallback_routes" WHERE "key" = 'translator'
  ON CONFLICT ("key") DO NOTHING;

INSERT INTO "model_tier_assignments" ("key", "tier")
  SELECT 'translate', "tier" FROM "model_tier_assignments" WHERE "key" = 'translator'
  ON CONFLICT ("key") DO NOTHING;
