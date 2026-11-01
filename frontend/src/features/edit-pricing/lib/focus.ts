import { domIdOf } from "../model/fields";

/**
 * Brings a field into view and focus — or, when the field has no control of
 * its own on screen, the nearest group that holds it (`input:k3:labels:de` →
 * `input:k3:labels` → `input:k3`).
 */
export function focusField(field: string): void {
  for (let at: string | null = field; at !== null; at = at.includes(":") ? at.slice(0, at.lastIndexOf(":")) : null) {
    const el = document.getElementById(domIdOf(at));
    if (!el) continue;
    el.scrollIntoView({ block: "center", behavior: "smooth" });
    el.focus({ preventScroll: true });
    return;
  }
}
