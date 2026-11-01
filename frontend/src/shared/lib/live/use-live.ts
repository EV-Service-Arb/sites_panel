import { useEffect, useRef } from "react";

import { type LiveMatch, type LiveSignal, liveBus, matchesLive } from "./bus";

/**
 * Calls `onSignal` with the signals that matched during the last `debounceMs`:
 * a burst (a batch import sends one `changed` per event) is one re-read, not fifty. A `resync`
 * always matches.
 */
export function useLiveSignal(match: LiveMatch, onSignal: (signals: LiveSignal[]) => void, debounceMs = 400): void {
  // The newest of both, without re-subscribing on every render (a topic array
  // literal is new each time).
  const latest = useRef({ match, onSignal });
  useEffect(() => {
    latest.current = { match, onSignal };
  });

  useEffect(() => {
    let batch: LiveSignal[] = [];
    let timer: ReturnType<typeof setTimeout> | null = null;
    const flush = () => {
      timer = null;
      const signals = batch;
      batch = [];
      latest.current.onSignal(signals);
    };
    const off = liveBus.subscribe((signal) => {
      if (!matchesLive(latest.current.match, signal)) return;
      batch.push(signal);
      timer ??= setTimeout(flush, debounceMs);
    });
    return () => {
      off();
      if (timer !== null) clearTimeout(timer);
    };
  }, [debounceMs]);
}
