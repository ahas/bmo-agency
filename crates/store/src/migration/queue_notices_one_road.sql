-- Every notice of the queue is a quiet instruction to the orderer, and an idle agent's notice stands in for the
-- launch notice and the idle queue's alarm (docs/agent-control.md, 「An idle agent is told to the orderers of the
-- items it could take」): a row still owed by the receiver's owner road goes, a launch notice told is withdrawn
-- from the receiver and one not yet told goes, and the alarm's rows go.
DELETE FROM "notices" WHERE "route" = 'owner';

UPDATE "notices"
   SET "route" = 'withdrawal',
       "body" = json_build_object('id', "instruction_id")::text,
       "keep" = 'once',
       "mark" = '',
       "due_at" = (extract(epoch FROM now()) * 1000)::bigint,
       "tries" = 0,
       "failure" = NULL
 WHERE "key" LIKE 'item-awaits-launch:%' AND "route" = 'instruction' AND "instruction_id" IS NOT NULL;

DELETE FROM "notices" WHERE "key" LIKE 'item-awaits-launch:%' AND "route" = 'instruction';

DELETE FROM "settings"
 WHERE "key" IN ('queue_idle_alarm', 'queue_idle_told', 'queue_idle_minutes')
    OR "key" LIKE 'queue_item_waiting:%';
