-- The units each account reaches, as the service in front of the agency writes them whole: whether the account's
-- people may read a unit's resources, and whether they head it and so may change them. The agency keeps no
-- organisation of its own; it reads this table and nothing else for a unit's reach (docs/api.md, 「Who a caller
-- acts for」).

CREATE TABLE "unit_reach" (
  "account" bigint NOT NULL,
  "unit" bigint NOT NULL,
  "write" boolean NOT NULL DEFAULT false,
  PRIMARY KEY ("account", "unit")
);
