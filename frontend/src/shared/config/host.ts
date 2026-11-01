/** What the host page tells the bundle through the elements' attributes; one page, one set for both elements. */
export interface Host {
  /** The sites API as the host forwards it (`/api/sites`); paths are joined onto it. */
  api: string;
  /** Where the host mounts the sites page (`/sites`); the router's routes are under it. */
  base: string;
  /** The cookie holding the host's CSRF token, sent back on every write. */
  csrfCookie: string;
}

let settled: Host | null = null;

/** A root-relative path without a trailing slash. */
function isPath(value: string): boolean {
  return value.startsWith("/") && !value.startsWith("//") && !value.endsWith("/");
}

export function settleHost(next: Host): void {
  if (!isPath(next.api) || !isPath(next.base)) throw new Error(`api and base are root-relative paths without a trailing slash: ${next.api}, ${next.base}`);
  if (next.csrfCookie === "") throw new Error("csrf-cookie is empty");
  if (settled !== null && (settled.api !== next.api || settled.base !== next.base || settled.csrfCookie !== next.csrfCookie)) {
    throw new Error(`the sites elements on one page disagree: ${JSON.stringify(settled)} vs ${JSON.stringify(next)}`);
  }
  settled = next;
}

export function host(): Host {
  if (settled === null) throw new Error("no sites element has connected yet");
  return settled;
}
