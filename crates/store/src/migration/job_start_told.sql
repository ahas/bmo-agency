-- When a job's orderer was told it stood `starting` too long (docs/agent-control.md, 「A job that stays starting is
-- told once more」): none until it is.
ALTER TABLE "jobs" ADD COLUMN "start_told_at" bigint;
