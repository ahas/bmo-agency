-- The events the stream carries, kept so a reader resumes across the service's restarts (docs/api.md,
-- 「events」): one row an event, by its number.
CREATE TABLE "stream_events" (
  "id" bigint,
  "kind" text NOT NULL,
  "at" bigint NOT NULL,
  "data" text NOT NULL,
  PRIMARY KEY ("id")
);
