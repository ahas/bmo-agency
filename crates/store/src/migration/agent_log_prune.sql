-- The run log's prune reads the lines of one agent that no job wrote, and the lines past the age, so the
-- indexes it reads are partial on that predicate: the writer's per-agent pages in id order, and the age
-- sweep in time order.
CREATE INDEX IF NOT EXISTS "agent_log_prune_agent" ON "agent_log" ("agent", "id") WHERE "job_id" IS NULL;
CREATE INDEX IF NOT EXISTS "agent_log_prune_at" ON "agent_log" ("at") WHERE "job_id" IS NULL;
