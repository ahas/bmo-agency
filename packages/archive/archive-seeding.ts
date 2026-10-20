// What agency's store is seeded with as its rows are written: its work keys and where each starts, its
// gates, and the role each kind of agent holds. Agency's own processes register them from its policy as
// their composition loads (`packages/agent/registrations.ts`), and the few members that read them ask here.

// A work key's profile, as far as the store reads one.
export interface SeededWorkProfile {
  key: string;
  // The road the key takes where nobody chose one.
  defaultRouteMode: string;
  // The Claude model the key starts on, which a route row that names the same one does not repeat.
  defaultClaudeModel: string;
}

export interface ArchiveSeeds {
  // Every work key's profile: a key's default road is read off them.
  workProfiles: readonly SeededWorkProfile[];
  // Every gate of the gate order: a gate's own thinking and effort are its tier's.
  gateKeys: readonly string[];
  // The role a row of each kind holds where its writer names none (docs/agent-roles.md), given to a row
  // the store makes for a kind — a worker the owner adds, a support worker, a companion naming no role.
  roleOfKind: Readonly<Record<string, string>>;
}

// One companion as its deployment declares it: its agent name, what the owner calls it, the mark a
// screen puts beside it, the provider and the model its turns go out on, the worker its TODO items go
// to, its picture, a path under the asset root, the gates its turns start with switched off, and the
// prompt folder its row names (docs/companions.md, 「Its settings」).
export interface CompanionSeed {
  name: string;
  label: string;
  icon: string;
  provider: string;
  model: string;
  itemWorker: string;
  picture: string;
  gatesOff?: readonly string[];
  // Whether its row starts thinking; absent leaves it to its model's default.
  thinking?: boolean;
  // The role it holds from its first start (docs/agent-roles.md); absent, its kind's.
  role?: string;
  // The folder under `prompts/agents/` its system prompt and persona are read from; absent, none
  // (docs/store.md, 「Its tables」).
  prompts?: string;
}

export class ArchiveUnseeded extends Error {
  constructor() {
    super(
      "no archive seeds are registered: a process that writes agency's store imports agency's registrations (`@agency/agent/registrations`) first",
    );
  }
}

let registered: ArchiveSeeds | null = null;

export function registerArchiveSeeds(seeds: ArchiveSeeds): void {
  registered = seeds;
}

export function archiveSeeds(): ArchiveSeeds {
  if (registered === null) {
    throw new ArchiveUnseeded();
  }

  return registered;
}

// The road a work key takes where nobody chose one: its profile's, or the automatic walk for a key
// no profile declares.
export function seededDefaultRoute(key: string): string {
  return archiveSeeds().workProfiles.find((one) => one.key === key)?.defaultRouteMode ?? "auto";
}
