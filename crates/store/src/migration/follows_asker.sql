-- A role's setting is its agents' default; an agent's own, where it holds one, stands over it.
ALTER TABLE "roles" ADD COLUMN "follows_asker" bigint NOT NULL DEFAULT 0;
ALTER TABLE "role_agents" ADD COLUMN "follows_asker" bigint;

-- The transliterator follows the supervisor whose narration asks it, its agents' own roads standing where none does.
UPDATE "roles" SET "follows_asker" = 1 WHERE "name" = 'transliterator';
