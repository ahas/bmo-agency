ALTER TABLE "registered_models" ADD COLUMN "card_service" text;
ALTER TABLE "registered_models" ADD COLUMN "role_files" bigint NOT NULL DEFAULT 0;
CREATE TABLE "card_demands" (
  "demand_id" bigint PRIMARY KEY,
  "service" text NOT NULL,
  "registration" text NOT NULL,
  "holder" text NOT NULL,
  "lease" text,
  "within" bigint,
  "asked_at" bigint NOT NULL
);
