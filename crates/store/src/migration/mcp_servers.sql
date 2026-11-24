-- The MCP servers agency put at home for an account (docs/person-nodes.md, 「MCP servers」): the account, the server's
-- name, the key its home copy stands under, and when the row was written.
CREATE TABLE IF NOT EXISTS "mcp_servers" (
  "account" text NOT NULL,
  "name" text NOT NULL,
  "key" text NOT NULL,
  "recorded_at" bigint NOT NULL,
  PRIMARY KEY ("account", "name")
);
