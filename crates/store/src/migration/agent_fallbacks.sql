-- An agent's fallback roads in order: a JSON list of roads, each `provider`, `model`, `effort`, `thinking`,
-- `answerTokens` and `advisor`, asked only where the road before it cannot answer for a clear reason
-- (docs/role-agents.md, 「An agent's fallback roads」). An empty list holds none.
ALTER TABLE "role_agents" ADD COLUMN "fallbacks" text NOT NULL DEFAULT '[]';
