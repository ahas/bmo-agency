// The agency's client (docs/openapi.json): the types openapi-typescript generates from the document, and
// openapi-fetch's typed calls over them, each route by its path.

import createClient, { type Client } from "openapi-fetch";

import type { components, paths } from "./schema.ts";

export type { components, operations, paths } from "./schema.ts";

type Schemas = components["schemas"];

export type Health = Schemas["Health"];
export type Event = Schemas["Event"];
export type EventPage = Schemas["EventPage"];
export type Agent = Schemas["Agent"];
export type Session = Schemas["Session"];
export type Turn = Schemas["Turn"];
export type Run = Schemas["Run"];
export type LogLine = Schemas["LogLine"];
export type Execution = Schemas["Execution"];
export type Pool = Schemas["Pool"];
export type SeatRequest = Schemas["SeatRequest"];
export type Reservation = Schemas["Reservation"];
export type Place = Schemas["Place"];
export type Provider = Schemas["Provider"];
export type Model = Schemas["Model"];
export type Tier = Schemas["Tier"];
export type Worker = Schemas["Worker"];
export type Assignment = Schemas["Assignment"];
export type JobItem = Schemas["JobItem"];
export type Job = Schemas["Job"];
export type SessionWork = Schemas["SessionWork"];
export type SessionLoads = Schemas["SessionLoads"];
export type SessionSummary = Schemas["SessionSummary"];
export type WireError = Schemas["Error"];

export type AgencyClient = Client<paths>;

/**
 * The agency at `baseUrl`, sending `token` as its bearer token. Each call is a route of the OpenAPI
 * document, typed by it: `client.GET("/sessions/{sessionId}", { params: { path: { sessionId } } })`.
 */
export function agencyClient(baseUrl: string, token: string, fetcher?: typeof globalThis.fetch): AgencyClient {
  return createClient<paths>({
    baseUrl,
    headers: { Authorization: `Bearer ${token}` },
    ...(fetcher ? { fetch: fetcher } : {}),
  });
}
