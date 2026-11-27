-- The values of the settings the agency declares, as the service in front of it resolved them for a person or a
-- unit and wrote them whole: a row a subject (`person:<id>` or `unit:<id>`), a setting's key and its value. The agency
-- keeps no delegation chain of its own; where a subject holds no row, a setting says nothing and its default stands.

CREATE TABLE "subject_settings" (
  "subject" text NOT NULL,
  "key" text NOT NULL,
  "value" jsonb NOT NULL,
  PRIMARY KEY ("subject", "key")
);
