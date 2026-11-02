UPDATE "assignments" SET "state" = 'released' WHERE "state" = 'expired';
ALTER TABLE "assignments" DROP COLUMN "expires_at";
