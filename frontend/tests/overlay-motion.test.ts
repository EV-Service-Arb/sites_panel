import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

// The panel once patched the kit's Sheet and Dialog motion in globals.css (the
// scrim flashed dark again on close; a click right after closing landed on the
// fading scrim). uikit 0.26 ships the fixes itself and the patch is gone: these
// hold the kit to them, so a release that loses one fails here, not on screen.
const read = (path: string) => readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");
const kit = (name: string) => read(`../node_modules/@evinvest/uikit/dist/generated/${name}.js`);

function kitConst(source: string, name: string): string {
  const m = new RegExp(`const ${name} = "([^"]*)"`).exec(source);
  if (!m?.[1]) throw new Error(`${name} not found in the kit`);
  return m[1];
}

function durationOf(classes: string, state?: "open" | "closed"): number {
  const prefix = state ? `data-\\[state=${state}\\]:` : "(?:^|\\s)";
  const m = new RegExp(`${prefix}duration-(\\d+)`).exec(classes);
  if (!m?.[1]) throw new Error(`no ${state ?? ""} duration in "${classes}"`);
  return Number(m[1]);
}

const OVERLAYS = [
  ["sheet", "SHEET_OVERLAY", "SHEET_CONTENT"],
  ["dialog", "DIALOG_OVERLAY", "DIALOG_CONTENT"],
  ["alert-dialog", "ALERT_DIALOG_OVERLAY", "ALERT_DIALOG_CONTENT"],
] as const;

describe("the kit's overlays", () => {
  it.each(OVERLAYS)("%s: the scrim lasts as long as its panel, both ways", (file, overlay, content) => {
    const scrim = kitConst(kit(file), overlay);
    const panel = kitConst(kit(file), content);
    for (const state of ["open", "closed"] as const) {
      const of = (c: string) => (c.includes(`data-[state=${state}]:duration-`) ? durationOf(c, state) : durationOf(c));
      expect(of(scrim)).toBe(of(panel));
    }
  });

  it.each(OVERLAYS)("%s: a closing scrim holds its last frame and lets clicks through", (file, overlay, content) => {
    const scrim = kitConst(kit(file), overlay);
    expect(scrim).toContain("data-[state=closed]:fill-mode-forwards");
    expect(scrim).toContain("data-[state=closed]:pointer-events-none");
    expect(kitConst(kit(file), content)).toContain("data-[state=closed]:fill-mode-forwards");
  });

  it("keep only a fade under reduced motion, in the kit's own tokens", () => {
    const tokens = read("../node_modules/@evinvest/uikit/styles/tokens.css");
    expect(tokens).toMatch(/prefers-reduced-motion: reduce\)\s*\{\s*\[data-slot="sheet-content"\],\s*\[data-slot="dialog-content"\],\s*\[data-slot="alert-dialog-content"\]/);
  });

  it("are no longer patched by the panel", () => {
    expect(read("../src/app/styles.css")).not.toMatch(/data-slot[$^*]?="[a-z-]*(overlay|content)"/);
  });
});
