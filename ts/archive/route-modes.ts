import { agentProviderNames } from "./provider-names";

// What a work key's route row may name as its road, and the model its own-LLM columns start on. The
// archive stores and settles these rows, so what may stand in them stands with the archive; what a
// work key's profile is, is the agent base's (`work-profile.ts`).

// A stored mode is the automatic walk or one provider the key runs on alone
// (docs/cache-fallback.md, 「The per-key choice is automatic or one provider」). An explicit job
// request names a provider in the same terms. Laya is a gate's provider and no agent's: a gate
// whose profile offers it may stand on it, and nothing else may (docs/laya.md, 「A gate whose
// provider is Laya」).
export const layaRoad = "laya";

export const fallbackRouteModes = ["auto", ...agentProviderNames, layaRoad] as const;

export type FallbackRouteMode = (typeof fallbackRouteModes)[number];

export function isFallbackRouteMode(value: unknown): value is FallbackRouteMode {
  return typeof value === "string" && (fallbackRouteModes as readonly string[]).includes(value);
}
