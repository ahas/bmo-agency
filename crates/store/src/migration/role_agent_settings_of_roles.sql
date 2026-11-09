UPDATE "role_agents" AS a
SET "sessions" = r."sessions",
    "callable" = r."callable",
    "fold_ratio" = COALESCE(a."fold_ratio", r."fold_ratio")
FROM "roles" AS r
WHERE r."name" = a."role"
