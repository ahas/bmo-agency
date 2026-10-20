import declaredCeilings from "@agency/policy/answer-ceilings.toml";
import declaredTiers from "@agency/policy/tiers.toml";
import { agentProviderNames } from "./provider-names";
import type { AgentProviderName } from "./provider-names";
import type { ArchiveSql } from "./archive-sql";

// A tier names a level of work — how capable a model it takes — rather than a model, and each
// provider maps every tier to one of its own models (docs/model-tiers.md). Which tiers there are is
// the policy's ladder; the store keeps the mappings and the assignments and knows no tier by name.
export type ModelTier = string;

// The providers a mapping may name, which is every provider this build carries: the column is
// called `provider` in these tables, and what may stand in it is the providers' own list.
export const tierProviders = agentProviderNames;
export type TierProvider = AgentProviderName;
export interface TierChoice { model: string; effort: string | null }
// The registry's mapping also names the ceiling of the answer's tokens; null is the work's own
// default, and every other provider's is null (docs/own-llm.md, 「Settings per tier」).
export interface TierMapping extends TierChoice { provider: TierProvider; tier: ModelTier; answerTokens?: number | null }
export interface TierConflict { key: string; provider: TierProvider; previous: TierChoice; accepted: boolean }
export interface ModelCapability { provider: TierProvider; model: string; efforts: readonly (string | null)[] }

// The tiers, in the order the tiers are shown and compared, as the policy's ladder holds them.
export const tierLadder: readonly ModelTier[] = declaredTiers.ladder as string[];

function keyName(value: string) {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9:._-]{0,200}$/.test(value)) throw new Error("Invalid work key");
}

// The answer ceilings a request to a registration is worth offering, the shared policy's
// (`docs/policy.md`).
export const tierAnswerTokenChoices: readonly number[] = declaredCeilings.choices as number[];

export function isTierAnswerTokens(value: unknown): value is number {
  return typeof value === "number" && (tierAnswerTokenChoices as readonly number[]).includes(value);
}

// What a mapping has to be whoever its provider is: a tier that exists, a provider this build
// carries, a model identifier of the shape a command line will take. What only the named provider
// can answer — whether it takes that effort word, whether it caps the answer's tokens at all — is
// asked of the provider where a mapping is written and again where one is launched, because this
// store stands below the providers and reaches none of them.
function checkTierMapping(ladder: readonly ModelTier[], value: TierMapping): void {
  if (!ladder.includes(value.tier)) throw new Error(`Unknown model tier: ${value.tier}`);
  if (!(tierProviders as readonly string[]).includes(value.provider)) throw new Error("Unknown tier provider");
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._:/-]{0,200}$/.test(value.model)) throw new Error("Invalid model identifier");
  if ((value.answerTokens ?? null) !== null && !isTierAnswerTokens(value.answerTokens)) throw new Error("Unsupported answer token ceiling");
}

/**
 * DB configuration only. This store does not select routes or launch a provider.
 *
 * Its tables are the store crate's (docs/store.md), and their seeds are given as the store is settled.
 */
export class ModelTierStore {
  constructor(private sql: ArchiveSql, readonly ladder: readonly ModelTier[] = tierLadder) {}

  // A tier the ladder names.
  private tier(value: string): void {
    if (!this.ladder.includes(value)) throw new Error(`Unknown model tier: ${value}`);
  }

  private validate(value: TierMapping) {
    checkTierMapping(this.ladder, value);
  }

  async mappings(): Promise<TierMapping[]> {
    return await this.sql.all(`SELECT provider,tier,model,effort,answer_tokens AS "answerTokens" FROM model_tier_mappings ORDER BY provider,tier`) as TierMapping[];
  }
  async assignments(): Promise<{ key: string; tier: ModelTier }[]> {
    return await this.sql.all("SELECT key,tier FROM model_tier_assignments ORDER BY key") as { key: string; tier: ModelTier }[];
  }
  async legacy(key: string): Promise<unknown | null> {
    const row = await this.sql.get("SELECT body FROM model_tier_legacy WHERE key=?", key) as { body: string } | null;
    return row ? JSON.parse(row.body) : null;
  }
  async conflicts(): Promise<TierConflict[]> {
    return (await this.sql.all("SELECT key,provider,previous,accepted FROM model_tier_conflicts ORDER BY key,provider") as { key: string; provider: TierProvider; previous: string; accepted: number }[])
      .map(r => ({ ...r, previous: JSON.parse(r.previous), accepted: r.accepted === 1 }));
  }
  async resolve(key: string, provider: TierProvider): Promise<TierMapping> {
    const assignment = (await this.assignments()).find(r => r.key === key);
    if (!assignment) throw new Error(`Missing tier assignment: ${key}`);
    if ((await this.conflicts()).some(r => r.key === key && r.provider === provider && !r.accepted)) throw new Error(`Unresolved tier migration: ${key}/${provider}`);
    return this.resolveTier(assignment.tier, provider);
  }
  async resolveTier(value: ModelTier, provider: TierProvider): Promise<TierMapping> {
    this.tier(value);
    const mapping = (await this.mappings()).find(r => r.provider === provider && r.tier === value);
    if (!mapping) throw new Error(`Missing tier mapping: ${provider}/${value}`);
    this.validate(mapping);
    return mapping;
  }
  async setMapping(mapping: TierMapping, capability: ModelCapability | null) {
    return await this.sql.wrote(async () => {
      this.validate(mapping);
      if (!capability || capability.provider !== mapping.provider || capability.model !== mapping.model || !capability.efforts.includes(mapping.effort)) throw new Error("Model capability evidence required for this model and effort");
      await this.sql.run("INSERT INTO model_tier_mappings (provider,tier,model,effort,answer_tokens) VALUES (?,?,?,?,?) ON CONFLICT(provider,tier) DO UPDATE SET model=excluded.model,effort=excluded.effort,answer_tokens=excluded.answer_tokens", mapping.provider, mapping.tier, mapping.model, mapping.effort, mapping.answerTokens ?? null);
    });
  }
  async assign(key: string, value: ModelTier) {
    keyName(key); this.tier(value);
    await this.sql.transaction(async () => {
      await this.sql.run("INSERT INTO model_tier_assignments VALUES (?,?) ON CONFLICT(key) DO UPDATE SET tier=excluded.tier", key, value);
      await this.sql.run("UPDATE model_tier_conflicts SET accepted=1 WHERE key=?", key);
    });
  }
  async removeMapping(provider: TierProvider, value: ModelTier) {
    await this.sql.run("DELETE FROM model_tier_mappings WHERE provider=? AND tier=?", provider, value);
  }
  async removeAssignment(key: string) { await this.sql.run("DELETE FROM model_tier_assignments WHERE key=?", key); }
  // The tiers whose mapping on a provider names one model, in the ladder's order.
  async tiersNaming(provider: TierProvider, model: string): Promise<ModelTier[]> {
    const named = new Set((await this.sql.all("SELECT tier FROM model_tier_mappings WHERE provider=? AND model=?", provider, model) as { tier: string }[]).map(r => r.tier));
    return this.ladder.filter(t => named.has(t));
  }
}
