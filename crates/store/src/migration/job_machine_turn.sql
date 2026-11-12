-- A job standing in the machine's turn leaves its agent free for another job (docs/role-agents.md, 「When an
-- agent is busy」): each job carries the mark of the last reading of the testbed's lanes and its item's close.
ALTER TABLE "jobs" ADD COLUMN IF NOT EXISTS "machine_turn" BOOLEAN NOT NULL DEFAULT FALSE;
