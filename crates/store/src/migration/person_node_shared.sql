-- Several accounts' records name one node only where the administrator declared it shared for each
-- (docs/person-nodes.md, 「Several accounts on one node」): a record carries whether it is, and none is until said.
ALTER TABLE "person_nodes" ADD COLUMN IF NOT EXISTS "shared" boolean NOT NULL DEFAULT false;
