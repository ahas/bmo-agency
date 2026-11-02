-- The delegator role's order emptied where its providers and models are still the ones the roles policy gave
-- it, in that order; an order the owner set on the role is left as it stands.
UPDATE "roles" SET "provider_order" = '[]'
WHERE "name" = 'delegator'
  AND (
    SELECT jsonb_agg(jsonb_build_array("entry"->>'provider', "entry"->>'model') ORDER BY "at")
    FROM jsonb_array_elements("roles"."provider_order"::jsonb) WITH ORDINALITY AS "entries"("entry", "at")
  ) = '[["claude-code", "claude-sonnet-5"], ["codex", "gpt-5.6-terra"], ["deepseek", "deepseek-flash"]]'::jsonb;
