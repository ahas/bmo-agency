ALTER TABLE "role_agents" ADD COLUMN "sessions" text NOT NULL DEFAULT 'keep';
ALTER TABLE "role_agents" ADD COLUMN "callable" bigint NOT NULL DEFAULT 0;
