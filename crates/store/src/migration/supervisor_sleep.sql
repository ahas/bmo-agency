-- A supervisor's sleep (docs/sleep.md, 「Its setting」): whether it sleeps and at what hour, none where the policy's
-- stands, the day it last slept for and the moment it last tried. A supervisor with no row sleeps at the policy's
-- hour.
CREATE TABLE IF NOT EXISTS "supervisor_sleep" (
  "agent" text PRIMARY KEY,
  "on" boolean NOT NULL DEFAULT true,
  "hour" integer,
  "slept_on" text,
  "tried_at" bigint
);
