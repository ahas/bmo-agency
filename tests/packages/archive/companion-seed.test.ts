import "@agency/test-support/setup";

import { describe, expect, test } from "bun:test";

import { scratchArchive } from "@agency/test-support/store";

import type { CompanionSeed } from "@agency/archive/archive-seeding";

// A companion as its deployment's seed declares it (docs/store.md, 「An agent's `prompts`」): the prompt
// folder the seed names is written with the row, and a row standing already keeps its own.

describe("standCompanion", () => {
  test("writes the prompt folder its seed names, and keeps the one a standing row holds", async () => {
    const archive = await scratchArchive();
    const agents = archive.agents();
    const seed: Omit<CompanionSeed, "name"> = { label: "Seeded", icon: "s", provider: "registered", model: "m", itemWorker: "worker", picture: "seeded.png" };
    const prompts = async (name: string) => ((await archive.sql.get("SELECT prompts FROM agents WHERE name = ?", name)) as { prompts: string | null }).prompts;

    await agents.standCompanion({ ...seed, name: "seeded-named", prompts: "seeded" });
    await agents.standCompanion({ ...seed, name: "seeded-bare" });
    await agents.standCompanion({ ...seed, name: "seeded-named", prompts: "another" });

    expect([await prompts("seeded-named"), await prompts("seeded-bare")]).toEqual(["seeded", null]);
  });
});
