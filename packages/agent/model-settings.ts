import queueWords from "@agency/policy/queue-words.toml";
import { carriesWorkLine, provider as providerNamed, walkedProvider } from "./agent-providers";
import { enabledProviders, providerEnabled } from "./provider-order";
import { isWorkKey, layaRoad, workProfile } from "./work-profile";

import type { Archive } from "@agency/archive/archive";
import type { FallbackRouteMode } from "./work-profile";
import type { TierMapping, TierProvider } from "@agency/archive/model-tiers";
import type { HarnessRun } from "./harness";
import { isReasoningLevel, tierReasoning } from "./reasoning";
import type { AgentReasoning } from "./reasoning";
import { providerSettingsArchive } from "./provider-order";
import { findInOrder } from "@agency/support/in-order";

export class TierConfigurationError extends Error {}

export async function configuredRun(run: HarnessRun, key: string, provider: TierProvider = "claude-code", store = archive ?? providerSettingsArchive()): Promise<HarnessRun> {
  if (!store) return run; // Standalone command fixtures have no installed settings archive.
  return mappedRun(run, await resolveExecutionModel(store, key, provider, run.tier));
}

// A run with a tier mapping laid on it: the mapping's model and tier, and the effort word its provider
// makes of the mapping's word and the agent's own choice the run carries (docs/model-tiers.md, 「An
// agent's own thinking and effort」). Every place a mapping is laid on a run lays it this way, so the
// record, the session and the prefix of the run carry the word it went out with.
export async function mappedRun(run: HarnessRun, mapping: Pick<TierMapping, "provider" | "model" | "effort" | "tier">): Promise<HarnessRun> {
  return { ...run, model: mapping.model, effort: await agentEffort(mapping, run.reasoning) ?? "", tier: mapping.tier };
}

// The effort word a run of a mapping goes out with where an agent's choice reaches it: the provider's
// own answer, and the mapping's word where no choice does.
export async function agentEffort(mapping: Pick<TierMapping, "provider" | "model" | "effort">, choice: AgentReasoning | null | undefined): Promise<string | null> {
  return choice == null ? mapping.effort : await providerNamed(mapping.provider).reasoning.effort(mapping.effort, choice, mapping.model);
}

// What one agent's row names of its own thinking and effort. A support worker runs on its base's, as
// it runs on its base's pin; an agent with no row, or a process with no archive, names nothing.
export async function agentReasoning(name: string, store = archive ?? providerSettingsArchive()): Promise<AgentReasoning> {
  const row = await store?.agents().getAgent(name) ?? null;
  const owner = row?.supportOf == null ? row : await store?.agents().getAgent(row.supportOf) ?? null;

  if (owner === null) return tierReasoning;

  return { thinking: owner.thinking, effort: isReasoningLevel(owner.effort) ? owner.effort : null };
}

// What a run builder sets for an agent's run: the agent's choice, carried for the mapping to be laid
// with, and whether the run thinks — the agent's switch where it names one, the line's otherwise.
export async function agentRunReasoning(name: string, lineThinking: boolean, store = archive ?? providerSettingsArchive()): Promise<Pick<HarnessRun, "reasoning" | "thinking">> {
  const reasoning = await agentReasoning(name, store);

  return { reasoning, thinking: reasoning.thinking ?? lineThinking };
}

/** Resolve once per physical attempt, before session selection or command construction. */
export async function resolveExecutionModel(store: Archive, key: string, provider: TierProvider, requestedTier?: string): Promise<TierMapping> {
  if (!await providerEnabled(store, provider)) throw new TierConfigurationError(`${provider}: ${queueWords.pool.off}`);
  try {
    const tiers = store.modelTiers();
    if ((await tiers.conflicts()).some(c => c.key === key && c.provider === provider && !c.accepted)) throw new Error(`Unresolved tier migration: ${key}/${provider}`);
    const result = requestedTier ? await tiers.resolveTier(requestedTier, provider) : await tiers.resolve(key, provider);
    // Whether this run may go out with the effort its mapping named is the provider's own word: a
    // program that cannot transmit one refuses a mapping that names one, and a program that needs
    // one refuses a mapping that names none.
    const fault = providerNamed(provider).effortFault(result.effort);
    if (fault !== null) throw new Error(fault);
    return result;
  } catch (error) { throw new TierConfigurationError(error instanceof Error ? error.message : String(error)); }
}

let archive: Archive | null = null;

export function installModelSettings(store: Archive | null): void {
  archive = store;
}

// The road a work key is on: the owner's row where the owner wrote one, the key's profile otherwise.
export async function selectedRouteMode(store: Archive, key: string): Promise<FallbackRouteMode> {
  return (await store.fallbacks().fallbackRoute(key))?.routeMode ?? (isWorkKey(key) ? workProfile(key).defaultRouteMode : "auto");
}

// The one provider a key is set to run on, or null where the key walks automatically. A key set
// to a provider the owner has turned off walks automatically too, so it reads as null here, and so
// does a gate on Laya, which runs on no agent provider at all and asks none of this.
export async function pinnedProvider(store: Archive, key: string): Promise<TierProvider | null> {
  const mode = await selectedRouteMode(store, key);

  return mode === "auto" || mode === layaRoad || !await providerEnabled(store, mode) ? null : mode;
}

// What a worker is pinned to, or null where it follows the worker key's road: a provider, and one
// model of it where the provider takes one named — a registration. A support worker runs its base's
// jobs and so is on its base's pin. A pin to a provider the owner has turned off reads as null, as a
// key set to one does.
export async function pinnedWorkerChoice(store: Archive, worker: string): Promise<{ provider: TierProvider; model: string | null } | null> {
  const row = await store.agents().getAgent(worker);
  const owner = row?.supportOf == null ? row : await store.agents().getAgent(row.supportOf);
  const pinned = owner?.pinnedProvider ?? null;

  if (pinned === null || !await providerEnabled(store, pinned)) return null;

  const model = providerNamed(pinned).namedModelFault === undefined ? null : owner?.pinnedModel ?? null;

  return { provider: pinned, model };
}

// The provider a key's first road is on: the one it is set to, or the first enabled one the
// automatic walk asks where it walks automatically — or, where every enabled provider is one the walk
// passes by, the first of them that carries the key's line, or else the first of them, since the
// owner left nothing else on.
export async function roadProvider(store: Archive, key: string): Promise<TierProvider> {
  const enabled = await enabledProviders(store);
  const line = workProfile(key).line;

  return await pinnedProvider(store, key)
    ?? await findInOrder(enabled, (name) => walkedProvider(name, { key, line, archive: store }))
    ?? enabled.find((name) => carriesWorkLine(name, line))
    ?? enabled[0]!;
}

export { configuredProviderEnabled } from "./provider-order";
