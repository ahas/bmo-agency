-- The run log is read by run: a page of one run's lines after an id, in id order. The index that stands on
-- (agent, run, id) serves a read that names the agent; a read by the run alone needs the run first.
CREATE INDEX IF NOT EXISTS "agent_log_run_id" ON "agent_log" ("run", "id");
