// The events agency's TypeScript side publishes, by the names the boundary gives them (docs/api.md,
// 「Events」), on the stream of the process that publishes them: a listener in that process reads each
// as it comes, and one that attaches late picks up from the id it last saw within the last stretch.
// Nothing is stored, and publishing never waits on a listener.

/** A login's end told (docs/cache-fallback.md, 「A login that is about to end」). */
export interface AccountLoginNotice {
  account: string;
  // Which login it is: the one on this machine, or the account's token for the work nodes.
  login: "here" | "node-token";
  stands: "gone" | "ending" | "missing";
  // How long the login has left, in milliseconds; null where nothing says.
  leftMs: number | null;
  // The words agency's policy composes of it, with the one command the owner runs.
  notice: string;
}

/** Every event this side publishes, by its name. */
export interface AgencyEvents {
  "account-login-notice": AccountLoginNotice;
}

export type AgencyEventName = keyof AgencyEvents;

/** One event as a listener reads it: its name, its body, its place in the stream and when it came. */
export type AgencyEvent = { [Name in AgencyEventName]: { name: Name; body: AgencyEvents[Name]; id: number; at: number } }[AgencyEventName];

export type AgencyEventListener = (event: AgencyEvent) => void;

// Long enough that a listener reconnecting after a dropped stream still finds where it was.
const bufferSize = 500;

const buffer: AgencyEvent[] = [];
const listeners = new Set<AgencyEventListener>();
let nextId = 1;

/** An event published to every listener; one that throws is logged and the rest are told all the same. */
export function emitAgencyEvent<Name extends AgencyEventName>(name: Name, body: AgencyEvents[Name]): AgencyEvent {
  const emitted = { name, body, id: nextId, at: Math.floor(Date.now() / 1000) } as AgencyEvent;

  nextId += 1;
  buffer.push(emitted);

  if (buffer.length > bufferSize) {
    buffer.splice(0, buffer.length - bufferSize);
  }

  for (const listener of listeners) {
    try {
      listener(emitted);
    } catch (error) {
      console.error("agency event listener failed:", error);
    }
  }

  return emitted;
}

/** The events published after the id given, within the last stretch kept. */
export function agencyEventsSince(id: number): AgencyEvent[] {
  return buffer.filter((event) => event.id > id);
}

/** A listener told of every event from now on, until the function it is handed back is called. */
export function subscribeAgencyEvents(listener: AgencyEventListener): () => void {
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
  };
}
