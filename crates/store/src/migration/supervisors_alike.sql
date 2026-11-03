-- Every supervisor is of one kind (docs/supervisors.md, 「One runner」): no row says which answers the rooms
-- that name none, which is the receiver's setting. The supervisor whose brain a runner line no supervisor
-- delegated writes into is agency's own setting, given the supervisor the column marked.
INSERT INTO "settings" ("key", "value")
SELECT 'research_supervisor', "name" FROM "supervisors" WHERE "default_answerer" <> 0 ORDER BY "name" LIMIT 1
ON CONFLICT ("key") DO NOTHING;

ALTER TABLE "supervisors" DROP COLUMN "default_answerer";
