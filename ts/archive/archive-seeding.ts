// What agency's store is seeded with that only its deployment can say: its work keys and where each
// starts, its gates, the role each kind of agent holds, the row its chat agent stands under and its
// companions. The deployment registers them as its configuration loads, and the few members that read
// them ask here. Nothing here knows whose they are.

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
  // Every gate of the deployment's gate order: a chat's unifying keeps their sessions apart.
  gateKeys: readonly string[];
  // The row the deployment's one chat agent stands under once every room is unified into it: its
  // name is the registered agent key, and these are what the row is seeded with. Its window and the
  // words that call it are its own values, written where its row has none (docs/agent-ownership.md).
  chatAgent: { label: string; aliases: string; icon: string; herdrLabel?: string; windowSeconds: number; wakeWords: readonly string[] };
  // The role a row of each kind holds where nothing names one (docs/agent-roles.md), given to a row
  // the store makes for a kind with no seed of its own — a worker the owner adds, a support worker,
  // the chat agent.
  roleOfKind: Readonly<Record<string, string>>;
  // The agents that answer in a room by themselves (docs/companions.md).
  companions: readonly CompanionSeed[];
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
      "no archive seeds are registered: a process reads its configuration before it settles the archive or asks it for a seed, or imports the deployment's archive seeds itself",
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
