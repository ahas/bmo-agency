-- A supervisor's accounts on chat services (docs/supervisors.md, 「Its accounts on chat services」): one supervisor
-- to many, one a service, each the account's number there and the name of the saved secret its sign-in is kept in,
-- gone with the supervisor.
CREATE TABLE "supervisor_accounts" (
  "supervisor" text NOT NULL REFERENCES "supervisors" ("name") ON DELETE CASCADE,
  "service" text NOT NULL,
  "number" text NOT NULL,
  "secret" text NOT NULL,
  PRIMARY KEY ("supervisor", "service")
);
