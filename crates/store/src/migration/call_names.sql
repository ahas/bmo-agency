-- A supervisor's call names stand in one column, "aliases" (docs/supervisors.md, 「Its call names」); no other
-- column holds a word that calls it.
ALTER TABLE "supervisors" DROP COLUMN IF EXISTS "wake_words";
