import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

// Only brackets opening on a digit or `#` are caught: the hardcoded sizes and colours. `calc()`, `var()` and
// `env()` brackets stay legal, as do `grid-cols-(--name)` and bare viewport units (`max-h-[90dvh]`).
const ARBITRARY = "-\\[(?!\\d+(?:\\.\\d+)?(?:d|s|l)?v[hw]\\])(?:#|\\d)";

export default tseslint.config(
  { ignores: ["dist/**", ".build/**", "node_modules/**"] },
  js.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    extends: [...tseslint.configs.recommended],
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Validators refuse control characters by naming them in a class; tests match NBSP and the narrow NBSP literally.
      "no-control-regex": "off",
      "no-irregular-whitespace": ["error", { skipRegExps: true }],
      "no-restricted-syntax": [
        "error",
        { selector: `Literal[value=/${ARBITRARY}/]`, message: "Arbitrary Tailwind value — use a scale step, or a CSS custom property for a genuine one-off." },
        { selector: `TemplateElement[value.raw=/${ARBITRARY}/]`, message: "Arbitrary Tailwind value — use a scale step, or a CSS custom property for a genuine one-off." },
      ],
    },
  },
  {
    files: ["scripts/**/*.{mjs,ts}", "eslint.config.mjs"],
    languageOptions: { globals: { process: "readonly", console: "readonly", URL: "readonly", Buffer: "readonly", setTimeout: "readonly", setInterval: "readonly" } },
  },
);
