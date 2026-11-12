-- A role's agents stand in no order of priority: a job's agent is named by its supervisor, and a run naming
-- none takes any free agent of its role.
ALTER TABLE "role_agents" DROP COLUMN IF EXISTS "position";

-- A kept session is its agent's: the agent whose run answered on it, none for a run whose agent could not be told.
ALTER TABLE "role_sessions" ADD COLUMN IF NOT EXISTS "agent" text;
