-- Every supervisor's settings stand in a table named for the supervisors (docs/supervisors.md, 「Its settings」):
-- the table that held them, its key, its row number's constraint and the sequence numbering it take that name.
ALTER TABLE "companion_settings" RENAME TO "supervisor_settings";
ALTER TABLE "supervisor_settings" RENAME CONSTRAINT "companion_settings_pkey" TO "supervisor_settings_pkey";
ALTER TABLE "supervisor_settings" RENAME CONSTRAINT "companion_settings_rowid_key" TO "supervisor_settings_rowid_key";
ALTER SEQUENCE "companion_settings_rowid_seq" RENAME TO "supervisor_settings_rowid_seq";
