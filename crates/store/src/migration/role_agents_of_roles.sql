INSERT INTO "role_agents"
  ("name", "role", "provider", "model", "effort", "thinking", "answer_tokens", "enabled", "position", "added_at")
SELECT r."name" || '-' || lpad(n::text, 2, '0'),
       r."name",
       COALESCE(r."active_provider", picked.entry ->> 'provider'),
       COALESCE(r."active_model", picked.entry ->> 'model'),
       picked.entry ->> 'effort',
       picked.entry ->> 'thinking',
       (picked.entry ->> 'answerTokens')::bigint,
       1,
       n,
       r."created_at"
FROM "roles" r
CROSS JOIN LATERAL generate_series(1, GREATEST(r."max_concurrent", 0)) AS n
LEFT JOIN LATERAL (
  SELECT entry FROM jsonb_array_elements(r."provider_order"::jsonb) AS entry
  WHERE r."active_provider" IS NULL OR entry ->> 'provider' = r."active_provider"
  LIMIT 1
) AS picked ON true
WHERE NOT EXISTS (SELECT 1 FROM "role_agents" a WHERE a."role" = r."name")
