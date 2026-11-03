-- The rank the demands of the runs for a supervisor are asked at; none where the supervisor holds none.
ALTER TABLE "supervisors" ADD COLUMN "card_rank" integer;
