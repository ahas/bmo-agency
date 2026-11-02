-- The life of a turn put into a program's pane (docs/agent-control.md, 「Telling that a turn's program is
-- gone」): one row a turn, keyed by its execution, written as its wait looks and closed with its ending.
CREATE TABLE "turn_liveness" (
  "execution" text,
  "run" text NOT NULL,
  "line" text NOT NULL,
  "session" text NOT NULL,
  "phase" text NOT NULL,
  "pane" text,
  "program_pid" bigint,
  "program_birth" text,
  "records" text NOT NULL DEFAULT '[]',
  "herdr" text,
  "herdr_at" bigint,
  "record_size" bigint NOT NULL DEFAULT 0,
  "record_grew_at" bigint,
  "status_line_at" bigint,
  "hook_at" bigint,
  "opened_at" bigint NOT NULL,
  "written_at" bigint NOT NULL,
  "ending" text,
  "ended_at" bigint,
  "detail" text,
  PRIMARY KEY ("execution")
);

CREATE INDEX "turn_liveness_open" ON "turn_liveness" ("ended_at");
