-- An organisation's own supply (docs/supply.md, 「Organisation supply」): a gateway key and a Claude account may
-- belong to a unit in place of a person, and a reservation of a run on it names the unit; a repository may belong
-- to a unit (docs/repositories.md, 「An organisation's repository」).
ALTER TABLE "personal_supply" ALTER COLUMN "user_id" DROP NOT NULL;
ALTER TABLE "personal_supply" ADD COLUMN "unit_id" bigint;
ALTER TABLE "personal_supply" ADD CONSTRAINT "personal_supply_one_owner"
  CHECK (("user_id" IS NULL) <> ("unit_id" IS NULL));
CREATE UNIQUE INDEX "personal_supply_unit_key" ON "personal_supply" (unit_id, road, secret)
  WHERE unit_id IS NOT NULL;

ALTER TABLE "claude_accounts" ADD COLUMN "unit_id" bigint;

ALTER TABLE "supply_reservations" ADD COLUMN "unit_id" bigint;

ALTER TABLE "repositories" ADD COLUMN "owner_unit" bigint;
