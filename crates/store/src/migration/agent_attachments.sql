-- The skills and MCP servers an account attaches to its agents (docs/person-nodes.md, 「Attaching skills and MCP
-- servers」): the account, the holder's kind (`supervisor` or `agent`) and name, the item's kind (`skill` or `mcp`)
-- and name, and when it was attached.
CREATE TABLE IF NOT EXISTS "agent_attachments" (
  "account" text NOT NULL,
  "holder_kind" text NOT NULL,
  "holder" text NOT NULL,
  "item_kind" text NOT NULL,
  "item" text NOT NULL,
  "attached_at" bigint NOT NULL,
  PRIMARY KEY ("account", "holder_kind", "holder", "item_kind", "item")
);
CREATE INDEX IF NOT EXISTS "agent_attachments_holder" ON "agent_attachments" ("holder_kind", "holder");
