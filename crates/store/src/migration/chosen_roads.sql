-- What chooses where a run goes (docs/provider-order.md, 「A provider the owner forces」; docs/repositories.md,
-- 「A model an item names」): the provider and model the owner forced on a role or a supervisor, the model a
-- job's item names, and which of the item, the forced provider, the pin and the order chose a job's road.
ALTER TABLE "roles" ADD COLUMN "active_provider" text;
ALTER TABLE "roles" ADD COLUMN "active_model" text;
ALTER TABLE "supervisors" ADD COLUMN "active_provider" text;
ALTER TABLE "supervisors" ADD COLUMN "active_model" text;
ALTER TABLE "jobs" ADD COLUMN "item_provider" text;
ALTER TABLE "jobs" ADD COLUMN "item_model" text;
ALTER TABLE "jobs" ADD COLUMN "chosen_by" text;
