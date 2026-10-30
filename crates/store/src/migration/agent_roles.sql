CREATE TABLE "agent_roles" (
  "name" text,
  "label" text NOT NULL,
  "bundles" text NOT NULL DEFAULT '[]',
  "max_concurrent" bigint,
  "prompt" text,
  "position" bigint NOT NULL,
  "created_at" bigint NOT NULL,
  "updated_at" bigint NOT NULL,
  PRIMARY KEY ("name")
);

CREATE TABLE "agent_role_seeded" (
  "name" text,
  PRIMARY KEY ("name")
);
