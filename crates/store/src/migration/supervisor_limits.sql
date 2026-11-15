-- A supervisor's brain switch and tool limit (docs/supervisors.md, 「Its settings」): none held means its brain is
-- on and it holds every tool, as every supervisor did before either could be set.
ALTER TABLE "supervisor_settings" ADD COLUMN IF NOT EXISTS "brain_on" bigint;
ALTER TABLE "supervisor_settings" ADD COLUMN IF NOT EXISTS "tools_allowed" text;
