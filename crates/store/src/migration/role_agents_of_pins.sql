INSERT INTO "role_agents" ("name", "role", "provider", "model", "enabled", "position", "added_at")
SELECT r."name" || '-' || lpad((seated.last + pinned.place)::text, 2, '0'),
       r."name",
       pinned."provider",
       pinned."model",
       1,
       seated.last + pinned.place,
       r."created_at"
FROM "roles" r
CROSS JOIN LATERAL (
  SELECT COALESCE(MAX(a."position"), 0) AS last FROM "role_agents" a WHERE a."role" = r."name"
) AS seated
JOIN (
  SELECT "pinned_provider" AS "provider", "pinned_model" AS "model",
         row_number() OVER (ORDER BY "pinned_provider", "pinned_model") AS place
  FROM (
    SELECT DISTINCT "pinned_provider", "pinned_model" FROM "repositories"
    WHERE COALESCE("pinned_provider", '') <> '' AND COALESCE("pinned_model", '') <> ''
  ) AS pins
) AS pinned ON true
WHERE r."name" = $1
  AND EXISTS (SELECT 1 FROM "role_agents" a WHERE a."role" = r."name" AND a."provider" IS NOT NULL)
  AND NOT EXISTS (
    SELECT 1 FROM "role_agents" a
    WHERE a."role" = r."name" AND a."provider" = pinned."provider" AND a."model" = pinned."model"
  )
