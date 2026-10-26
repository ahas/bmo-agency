UPDATE "coworker_pool_roles" SET "skills" =
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace("skills",
    '"bmo-worker:asking-the-owner"', '"bmo-worker:bmo-asking-the-owner"'),
    '"bmo-worker:beyond-the-checkout"', '"bmo-worker:bmo-beyond-the-checkout"'),
    '"bmo-worker:clean-code"', '"bmo-worker:ahas-code-clean"'),
    '"bmo-worker:closing-a-job"', '"bmo-worker:bmo-closing-a-job"'),
    '"bmo-worker:design-boundaries"', '"bmo-worker:ahas-code-boundaries"'),
    '"bmo-worker:design-patterns"', '"bmo-worker:ahas-code-design-patterns"'),
    '"bmo-worker:docs-conventions"', '"bmo-worker:bmo-docs-conventions"'),
    '"bmo-worker:eli5"', '"bmo-worker:bmo-docs-eli5"'),
    '"bmo-worker:find-skills"', '"bmo-worker:ahas-tools-find-skills"'),
    '"bmo-worker:marking-a-wait"', '"bmo-worker:bmo-marking-a-wait"'),
    '"bmo-worker:moving-korean-out-of-code"', '"bmo-worker:bmo-code-korean-out"'),
    '"bmo-worker:my-git-docs"', '"bmo-worker:ahas-docs-git"'),
    '"bmo-worker:my-nuxt-style"', '"bmo-worker:ahas-ux-nuxt-style"'),
    '"bmo-worker:nuxt-ui"', '"bmo-worker:ahas-ux-nuxt-ui"'),
    '"bmo-worker:playwright-cli"', '"bmo-worker:ahas-tools-playwright-cli"'),
    '"bmo-worker:refactoring"', '"bmo-worker:ahas-code-refactoring"'),
    '"bmo-worker:researching-and-benching"', '"bmo-worker:bmo-tools-researching-and-benching"'),
    '"bmo-worker:test-writing"', '"bmo-worker:ahas-code-test-writing"'),
    '"bmo-worker:ui-design-rules"', '"bmo-worker:ahas-ux-design-rules"'),
    '"bmo-worker:warm-prefix"', '"bmo-worker:bmo-warm-prefix"')
WHERE "skills" LIKE '%bmo-worker:%';
