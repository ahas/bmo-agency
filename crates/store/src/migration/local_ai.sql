ALTER TABLE "registered_models" ADD COLUMN "enabled" bigint NOT NULL DEFAULT 1;
ALTER TABLE "registered_model_addresses" ADD COLUMN "reach_port" bigint;
CREATE UNIQUE INDEX "registered_model_addresses_reach_port" ON "registered_model_addresses" ("reach_port");
