-- A role's agents stand in no order of priority: a job's agent is named by its supervisor, and a run naming
-- none takes any free agent of its role.
ALTER TABLE "role_agents" DROP COLUMN IF EXISTS "position";
