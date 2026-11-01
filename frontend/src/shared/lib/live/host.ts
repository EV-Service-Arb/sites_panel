import { useSyncExternalStore } from "react";

import { LIVE_EVENT, LIVE_STATUS_EVENT, tellHost } from "@/shared/lib/host-events";
import { oneOf, parse } from "@/shared/lib/parse";

import { liveBus } from "./bus";
import { LIVE_STATUSES, type LiveStatus, parseLiveMessage } from "./protocol";

/** While the host's socket cannot be had, the screens are re-read on this period instead. */
const POLL_MS = 60_000;

/** Null until the host has told its status: an element mounts after the socket's last change. */
let status: LiveStatus | null = null;
let poll: number | null = null;
const listeners = new Set<() => void>();
let installed = false;

/** What the old in-page client did on its own: a reconnect re-reads everything (events may have been missed), and so does every minute offline. */
function statusChanged(next: LiveStatus): void {
  const was = status;
  status = next;
  if (next === "live" && was !== null && was !== "connecting" && was !== "live") liveBus.emit({ kind: "resync" });
  if (next === "offline" && poll === null) poll = window.setInterval(() => liveBus.emit({ kind: "resync" }), POLL_MS);
  if (next !== "offline" && poll !== null) {
    window.clearInterval(poll);
    poll = null;
  }
  for (const l of listeners) l();
}

/** Called by each element as it connects: the window listeners once per page, the status asked for each time. */
export function followHostLive(): void {
  if (!installed) {
    installed = true;
    window.addEventListener(LIVE_EVENT, (e) => {
      const msg = parseLiveMessage((e as CustomEvent<unknown>).detail);
      if (msg?.type === "changed") liveBus.emit({ kind: "changed", event: { topic: msg.topic, brand_id: msg.brand_id, id: msg.id, at: msg.at } });
      else if (msg?.type === "resync") liveBus.emit({ kind: "resync" });
    });
    window.addEventListener(LIVE_STATUS_EVENT, (e) => {
      const detail = (e as CustomEvent<unknown>).detail;
      statusChanged(parse(oneOf(LIVE_STATUSES), typeof detail === "object" && detail !== null && "status" in detail ? detail.status : undefined));
    });
  }
  tellHost("sa:live-status-request");
}

export function useLiveStatus(): LiveStatus | null {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => status,
  );
}
