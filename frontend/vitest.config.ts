import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Logic-level tests in Node, like the other Service-Arb fronts: the rules the
// screens obey live in plain modules (`model/`, `lib/`), so no DOM is needed.
export default defineConfig({
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)), "@evinvest/sites-contract": fileURLToPath(new URL("../ts/sites-contract/src/index.ts", import.meta.url)) },
  },
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
    setupFiles: ["tests/setup.ts"],
  },
});
