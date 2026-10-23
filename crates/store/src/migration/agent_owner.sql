-- The login account an agent belongs to (docs/store.md, 「Its tables」): the id of its user, or none for the
-- administrator's, as every agent registered before it reads.
ALTER TABLE "agents" ADD COLUMN "owner_account" bigint;
