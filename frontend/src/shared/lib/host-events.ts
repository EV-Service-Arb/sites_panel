// The window events between the elements and the host page (README.md).

export const GMBS_EVENT = "sa:gmbs";

export function tellHost(type: "sa:title-count", detail: { count: number }): void;
export function tellHost(type: "sa:unauthenticated" | "sa:gmbs-request"): void;
export function tellHost(type: string, detail: unknown = null): void {
  window.dispatchEvent(new CustomEvent(type, { detail }));
}
