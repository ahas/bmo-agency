// What every test file stands on beside its home: agency's own composition of its agent base, and the
// seeds a store's agents are written from, as a deployment registers its own (`archive-seeding.ts`).
// The seeds name a chat agent, a gate and a companion of the tests' own, nobody's real ones.

import "@agency/agent/registrations";

import { registerArchiveSeeds } from "@agency/archive/archive-seeding";
import { policyWork } from "@agency/agent/work-keys";

const work = policyWork();

registerArchiveSeeds({
  workProfiles: work.profiles,
  gateKeys: work.gateKeys,
  chatAgent: { label: "Chat agent", aliases: "chat agent", icon: "c", windowSeconds: 600, wakeWords: ["agent"] },
  roleOfKind: { chat: "chat", worker: "worker" },
  companions: [],
});
