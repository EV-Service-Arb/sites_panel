import type { ChangedEvent, LiveTopic } from "./protocol";

/** What a screen hears: a change on a topic, or "re-read everything" (a `resync`, a reconnect, a poll). */
export type LiveSignal = { kind: "changed"; event: ChangedEvent } | { kind: "resync" };

/** The topics a reader cares about, or a test on the whole event (one brand, one record). */
export type LiveMatch = readonly LiveTopic[] | ((event: ChangedEvent) => boolean);

export function matchesLive(match: LiveMatch, signal: LiveSignal): boolean {
  if (signal.kind === "resync") return true;
  return typeof match === "function" ? match(signal.event) : match.includes(signal.event.topic);
}

interface LiveBus {
  subscribe(listener: (signal: LiveSignal) => void): () => void;
  emit(signal: LiveSignal): void;
}

function createLiveBus(): LiveBus {
  const listeners = new Set<(signal: LiveSignal) => void>();
  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    emit(signal) {
      // A copy: a listener may unsubscribe (a screen unmounting) while being told.
      for (const listener of [...listeners]) listener(signal);
    },
  };
}

/** One per page: the host's relayed messages feed it (host.ts), every mounted reader of both elements listens. */
export const liveBus: LiveBus = createLiveBus();
