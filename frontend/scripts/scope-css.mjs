// Turns the Tailwind build into the stylesheet the elements put into a host page (concierge's
// web/scripts/scope-css.mjs, scoped to a list of tags).
//
// The elements render in the host's LIGHT DOM, under the host's preflight, and the host puts
// remote styles in a layer below all of its own (@evinvest/mfe `REMOTE_LAYER`), so the host's
// preflight would beat every utility of ours on our own nodes. Hence:
//   - utilities are compiled `important` and nested under `:where(<tags>)`: an important
//     declaration in the lowest layer beats every normal one above it, and the nesting keeps
//     it off the host's own nodes, which use the same class names;
//   - everything else (theme variables, the kit's tokens, keyframes) stays normal inside the
//     layer, so the host's own copy wins;
//   - `@property` registrations are hoisted to the top level, where they are valid.
// Utilities arrive in the marker layer `evmfe-scope` (src/app/styles.css); without it the
// template is broken and this throws.

const MARKER = "evmfe-scope";

/** Top-level statements of `css`: `{ prelude, body }` for blocks, `{ text }` for the rest. */
function topLevel(css) {
  const items = [];
  let start = 0;
  let depth = 0;
  let open = -1;
  for (let i = 0; i < css.length; i++) {
    const c = css[i];
    if (c === '"' || c === "'") {
      const end = css.indexOf(c, i + 1);
      i = end === -1 ? css.length : end;
    } else if (c === "/" && css[i + 1] === "*") {
      const end = css.indexOf("*/", i + 2);
      i = end === -1 ? css.length : end + 1;
    } else if (c === "{") {
      if (depth === 0) open = i;
      depth++;
    } else if (c === "}") {
      depth--;
      if (depth === 0) {
        items.push({ prelude: css.slice(start, open).trim(), body: css.slice(open + 1, i) });
        start = i + 1;
      }
    } else if (c === ";" && depth === 0) {
      items.push({ text: css.slice(start, i + 1).trim() });
      start = i + 1;
    }
  }
  if (depth !== 0) throw new Error("scope-css: unbalanced braces in the Tailwind output");
  const tail = css.slice(start).trim();
  if (tail) items.push({ text: tail });
  return items;
}

/**
 * @param {string} css   Tailwind output for the bundle.
 * @param {string[]} tags The elements' tags; utilities apply inside them only.
 * @param {string} layer The host-contract layer (`REMOTE_LAYER` of @evinvest/mfe).
 */
export function scopeCss(css, tags, layer) {
  const hoisted = [];
  const rest = [];
  let utilities = null;
  for (const item of topLevel(css)) {
    if ("text" in item) {
      rest.push(item.text);
    } else if (item.prelude === `@layer ${MARKER}`) {
      utilities = item.body;
    } else if (item.prelude.startsWith("@property")) {
      hoisted.push(`${item.prelude}{${item.body}}`);
    } else {
      rest.push(`${item.prelude}{${item.body}}`);
    }
  }
  if (utilities === null) throw new Error(`scope-css: no @layer ${MARKER} block — src/app/styles.css lost its utilities import`);
  return `${hoisted.join("\n")}\n@layer ${layer}{\n${rest.join("\n")}\n:where(${tags.join(",")}){\n${utilities}\n}\n}\n`;
}
