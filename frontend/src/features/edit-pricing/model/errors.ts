import type { DraftCode, DraftProblem } from "./serialize";

/** What one field is told: a code the editor words itself, or the server's own words. */
export type Shown = { code: DraftCode; vars: Readonly<Record<string, string | number>> } | { text: string };

export interface FieldErrors {
  byField: ReadonlyMap<string, readonly Shown[]>;
  /** For what no field holds (the model's format, a path the draft no longer has): above the form. */
  general: readonly Shown[];
  /** The first field at fault, in the form's order: where "show me" goes. */
  first: string | null;
}

/** Said only once a save was tried: a new row is not wrong for being empty yet. */
const QUIET: ReadonlySet<DraftCode> = new Set(["required", "labelMissing"]);

/** A refusal by the server, filed under the field its path names (null: none). */
export interface ServerProblem {
  field: string | null;
  path: string;
  message: string;
}

export function errorsOf(problems: readonly DraftProblem[], revealAll: boolean, server: ServerProblem | null): FieldErrors {
  const byField = new Map<string, Shown[]>();
  const general: Shown[] = [];
  let first: string | null = null;
  const file = (field: string | null, shown: Shown) => {
    if (field === null) {
      general.push(shown);
      return;
    }
    first ??= field;
    byField.set(field, [...(byField.get(field) ?? []), shown]);
  };
  if (server) file(server.field, server.field === null && server.path !== "" ? { text: `${server.path}: ${server.message}` } : { text: server.message });
  for (const p of problems) if (revealAll || !QUIET.has(p.code)) file(p.field, { code: p.code, vars: p.vars });
  return { byField, general, first };
}

export const NO_FIELD_ERRORS: FieldErrors = { byField: new Map(), general: [], first: null };
