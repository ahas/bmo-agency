-- A road names the advisor its program may consult, none where it consults none (docs/provider-order.md, 「What a
-- road is」): every road stands with none.
ALTER TABLE "supervisors" ADD COLUMN "advisor" text;
ALTER TABLE "role_agents" ADD COLUMN "advisor" text;
