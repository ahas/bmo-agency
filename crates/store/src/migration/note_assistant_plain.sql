-- The note assistant goes out on the plain line, whose system prompt is the note unit's instructions alone
-- (docs/agent-roles.md, 「The note-assistant role」); a row the owner moved to another line stays as he left it.
UPDATE "roles" SET "line" = 'plain' WHERE "name" = 'note-assistant' AND "line" = 'search';
