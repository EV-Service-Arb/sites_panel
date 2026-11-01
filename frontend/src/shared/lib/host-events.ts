// The window events between the elements and the host page (README.md).

export const LIVE_EVENT = "sa:live";
export const LIVE_STATUS_EVENT = "sa:live-status";

export function tellHost(type: "sa:title-count", detail: { count: number }): void;
export function tellHost(type: "sa:unauthenticated" | "sa:live-status-request"): void;
export function tellHost(type: string, detail: unknown = null): void {
  window.dispatchEvent(new CustomEvent(type, { detail }));
}
