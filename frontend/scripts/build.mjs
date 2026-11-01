// Builds dist/: `mfe-sites.js` (defines <mfe-sites-panel> and <mfe-sites-nav>) and `mfe-sites.css` under
// stable names, the screens as hashed chunks under dist/chunks/. sites serves dist/ at /mfe/*, the conductor
// forwards it as /sites/mfe/*.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { REMOTE_LAYER } from "@evinvest/mfe";
import { build, transform } from "esbuild";

import { scopeCss } from "./scope-css.mjs";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const DIST = path.join(ROOT, "dist");
const WORK = path.join(ROOT, ".build");
const TAGS = ["mfe-sites-panel", "mfe-sites-nav"];
// Tailwind v4's own floor; esbuild lowers CSS nesting for it.
const CSS_TARGET = ["chrome111", "safari16.4", "firefox128"];

rmSync(DIST, { recursive: true, force: true });
rmSync(WORK, { recursive: true, force: true });
mkdirSync(DIST, { recursive: true });
mkdirSync(WORK, { recursive: true });

const tailwindOut = path.join(WORK, "styles.css");
const run = spawnSync(path.join(ROOT, "node_modules/.bin/tailwindcss"), ["-i", path.join(ROOT, "src/app/styles.css"), "-o", tailwindOut], { cwd: ROOT, encoding: "utf8" });
if (run.status !== 0) throw new Error(`tailwindcss failed:\n${run.stderr}`);
const { code: css } = await transform(scopeCss(readFileSync(tailwindOut, "utf8"), TAGS, REMOTE_LAYER), { loader: "css", minify: true, target: CSS_TARGET, legalComments: "none" });
writeFileSync(path.join(DIST, "mfe-sites.css"), css);

const result = await build({
  entryPoints: { "mfe-sites": path.join(ROOT, "src/app/entry.tsx") },
  bundle: true,
  splitting: true,
  format: "esm",
  platform: "browser",
  target: ["es2022"],
  jsx: "automatic",
  tsconfig: path.join(ROOT, "tsconfig.json"),
  // React reads process.env.NODE_ENV; a host page has no `process`.
  define: { "process.env.NODE_ENV": '"production"' },
  entryNames: "[name]",
  chunkNames: "chunks/[name]-[hash]",
  outdir: DIST,
  minify: true,
  legalComments: "linked",
  metafile: true,
  logLevel: "warning",
});
for (const [file, { bytes }] of Object.entries(result.metafile.outputs)) {
  if (file.endsWith(".js")) console.log(`${path.relative(DIST, file)}  ${(bytes / 1024).toFixed(1)} KiB`);
}
console.log(`mfe-sites.css  ${(css.length / 1024).toFixed(1)} KiB`);
