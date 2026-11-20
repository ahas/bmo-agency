-- The deploy queue's hourly flush retires each repository's due clock (docs/agent-control.md, 「The deploy queue is
-- flushed every hour」): the waiting commits' moments and the owner's due minutes go. A due notice still owed is
-- let go: one the receiver kept a copy of turns into that copy's withdrawal, due at once, and any other goes.
DELETE FROM "settings" WHERE "key" IN ('deploy_due_since', 'deploy_due_minutes');

UPDATE "notices"
   SET "route" = 'withdrawal',
       "body" = json_build_object('id', "instruction_id")::text,
       "keep" = 'once',
       "mark" = '',
       "version" = nextval('notice_versions'),
       "owed_at" = (extract(epoch FROM clock_timestamp()) * 1000)::bigint,
       "told_at" = NULL,
       "due_at" = (extract(epoch FROM clock_timestamp()) * 1000)::bigint,
       "tries" = 0,
       "failure" = NULL,
       "instruction_id" = NULL
 WHERE "key" LIKE 'repository-deploy-due:%' AND "route" = 'instruction' AND "instruction_id" IS NOT NULL;

DELETE FROM "notices" WHERE "key" LIKE 'repository-deploy-due:%' AND "route" = 'instruction';
