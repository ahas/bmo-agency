import type { Archive } from "@agency/archive/archive";
import { agentProviderNames } from "./harness";
import type { AgentProviderName } from "./harness";

const orderKey = "provider_order";
// Every provider this build carries, in the order the provider list names them.
export const defaultProviderOrder: readonly AgentProviderName[] = agentProviderNames;
export interface ProviderOrderEntry { name: AgentProviderName; enabled: boolean }
export type ProviderOrderInput = readonly AgentProviderName[] | readonly ProviderOrderEntry[];
export const allProvidersOffReason = "Not every provider can be turned off: keep at least one on.";
// Whether a value names providers of this list, each at most once, all as names or all as entries
// with an on/off.
function namesProviders(value: unknown, providers: readonly string[]): value is (string | { name: string; enabled: boolean })[] {
  if (!Array.isArray(value)) return false;
  const names = value.map(entry => typeof entry === "string" ? entry : entry?.name);
  const uniform = value.every(entry => typeof entry === "string") || value.every(entry => entry && typeof entry === "object" && typeof entry.enabled === "boolean");
  return uniform && new Set(names).size === names.length && names.every(name => providers.includes(name));
}
// An order to save: every provider this build carries, once each.
export function isProviderOrder(value: unknown): value is AgentProviderName[] | ProviderOrderEntry[] {
  return namesProviders(value, defaultProviderOrder) && value.length === defaultProviderOrder.length;
}
// A saved order as it reads under these providers: its own order and on/off kept, and every provider
// it does not name — one the build gained after it was saved — appended at the end, on. A value
// naming a provider outside the list, naming one twice, of any other shape, or leaving none on reads
// as null.
export function readProviderOrder<N extends string>(value: unknown, providers: readonly N[]): { name: N; enabled: boolean }[] | null {
  if (!namesProviders(value, providers)) return null;
  const entries = value.map(entry => typeof entry === "string" ? { name: entry as N, enabled: true } : { name: entry.name as N, enabled: entry.enabled });
  const missing = providers.filter(name => !entries.some(entry => entry.name === name)).map(name => ({ name, enabled: true }));
  const read = [...entries, ...missing];
  return read.some(entry => entry.enabled) ? read : null;
}
// The order a stored setting stands for, as every reader of it reads it: a provider the build gained
// after the order was saved is appended on, and storage that is missing or invalid is the default.
export function storedProviderOrder(stored: string | null): ProviderOrderEntry[] {
  try {
    const read = readProviderOrder(JSON.parse(stored ?? "null"), defaultProviderOrder);
    if (read !== null) return read;
  } catch { /* Invalid storage reads as the default. */ }
  return defaultProviderOrder.map(name => ({ name, enabled: true }));
}
export async function providerOrder(archive: Archive): Promise<ProviderOrderEntry[]> {
  return storedProviderOrder(await archive.settings().getSetting(orderKey));
}
export async function enabledProviders(archive: Archive): Promise<AgentProviderName[]> {
  return (await providerOrder(archive)).filter(entry => entry.enabled).map(entry => entry.name);
}
export async function providerEnabled(archive: Archive, name: AgentProviderName): Promise<boolean> {
  return (await providerOrder(archive)).some(entry => entry.name === name && entry.enabled);
}
export function isDefaultProviderOrder(order: ProviderOrderInput): boolean {
  return order.length === defaultProviderOrder.length && order.every((entry, index) => typeof entry === "string" ? entry === defaultProviderOrder[index] : entry.enabled && entry.name === defaultProviderOrder[index]);
}
export async function setProviderOrder(archive: Archive, order: ProviderOrderInput): Promise<void> {
  if (!isProviderOrder(order)) throw new Error("The order must name every provider once.");
  const held = await providerOrder(archive);
  const entries = order.map(entry => typeof entry === "string" ? { name: entry, enabled: held.find(one => one.name === entry)!.enabled } : { ...entry });
  if (!entries.some(entry => entry.enabled)) throw new Error(allProvidersOffReason);
  if (isDefaultProviderOrder(entries)) await archive.settings().deleteSetting(orderKey);
  else await archive.settings().setSetting(orderKey, JSON.stringify(entries));
}
export async function setProviderEnabled(archive: Archive, name: AgentProviderName, enabled: boolean): Promise<void> {
  if (!defaultProviderOrder.includes(name)) throw new Error("Unknown provider.");
  await setProviderOrder(archive, (await providerOrder(archive)).map(entry => entry.name === name ? { ...entry, enabled } : entry));
}

// The archive for launch-boundary checks in processes that installed the harness registry.
let launchStore: Archive | null = null;
export function installProviderSettings(archive: Archive | null): void { launchStore = archive; }
export async function configuredProviderEnabled(name: AgentProviderName): Promise<boolean> {
  return launchStore === null || await providerEnabled(launchStore, name);
}

export function providerSettingsArchive(): Archive | null { return launchStore; }
