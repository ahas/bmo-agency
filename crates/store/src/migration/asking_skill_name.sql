UPDATE "roles" SET "skills" = replace("skills", '"bmo-worker:bmo-asking-the-owner"', '"bmo-worker:bmo-asking"')
WHERE "skills" LIKE '%"bmo-worker:bmo-asking-the-owner"%';
UPDATE "role_agents" SET "skills" = replace("skills", '"bmo-worker:bmo-asking-the-owner"', '"bmo-worker:bmo-asking"')
WHERE "skills" LIKE '%"bmo-worker:bmo-asking-the-owner"%';
