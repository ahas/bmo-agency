CREATE TABLE IF NOT EXISTS "person_nodes" (
  "owner_account" text PRIMARY KEY,
  "node" text NOT NULL,
  "set_at" bigint NOT NULL
);
