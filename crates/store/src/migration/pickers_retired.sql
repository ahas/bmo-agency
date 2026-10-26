-- The picker gates no turn asks any more, and the chat-kind row that stood for the gates' old shared place:
-- their agent rows and the per-agent switches of those gates go, so no screen lists them. Their judgement
-- history is the gate's, and their places on disk are left as they stand.
DELETE FROM "agents" WHERE "kind" = 'gate' AND "name" IN ('document-seek', 'library-knowledge', 'people-search',
  'room-rules', 'situation-rules', 'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "agents" WHERE "name" = 'gate' AND "kind" = 'chat' AND "role" IS NULL;
DELETE FROM "settings" WHERE "key" LIKE 'agent\_gate:%' ESCAPE '\' AND split_part("key", ':', 3) IN ('document-seek',
  'library-knowledge', 'people-search', 'room-rules', 'room-memories', 'situation-rules', 'system-documents',
  'voice-polish', 'voice-reading');
