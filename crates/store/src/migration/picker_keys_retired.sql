-- The work keys of the picker gates no turn asks any more: their tier assignments, the owner's choices from
-- before the tiers kept for review, the backfill marks, the legacy choices and their roads go, so no screen
-- lists them.
DELETE FROM "model_tier_assignments" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "model_tier_conflicts" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "model_tier_backfilled" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "model_tier_legacy" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "fallback_routes" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
DELETE FROM "fallback_sessions" WHERE "key" IN ('document-seek', 'library-knowledge', 'people-search', 'room-memories', 'room-rules', 'situation-rules',
  'system-documents', 'voice-polish', 'voice-reading');
