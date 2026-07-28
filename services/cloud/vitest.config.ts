import fs from "node:fs";
import path from "node:path";
import { cloudflareTest } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

// schema.sql is applied in test/setup.ts via env.DB.exec() per statement (one
// CREATE TABLE/INDEX per line — see the file). Split here, in the Node-side
// config context, so the worker-side setup file only deals with plain
// strings (no filesystem access is available from inside workerd).
const schemaSql = fs.readFileSync(
  path.join(import.meta.dirname, "schema.sql"),
  "utf8",
);
const schemaStatements = schemaSql
  .split("\n")
  .map((line) => line.trim())
  .filter((line) => line.length > 0 && !line.startsWith("--"));

export default defineConfig({
  plugins: [
    cloudflareTest(async () => ({
      wrangler: {
        configPath: "./wrangler.jsonc",
        // Tests exercise the Turnstile dev-bypass path, so they run against
        // the "dev" named environment (dummy keys + TURNSTILE_DEV_BYPASS) —
        // never the default/production environment, which intentionally has
        // no bypass at all (see wrangler.jsonc and src/activate.ts).
        environment: "dev",
      },
      miniflare: {
        bindings: {
          TEST_SCHEMA_STATEMENTS: schemaStatements,
        },
      },
    })),
  ],
  test: {
    include: ["test/**/*.test.ts"],
    setupFiles: ["./test/setup.ts"],
  },
});
