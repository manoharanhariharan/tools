import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

const uiRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  // Pin the root to ui/ so `src/**` resolves where the tests actually live,
  // regardless of the directory npm was invoked from.
  root: uiRoot,
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    reporters: ["default"],
  },
});
