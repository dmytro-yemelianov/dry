import fs from "node:fs";
import path from "node:path";
import { cloudflareTest } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

// schema.sql is applied in test/setup.ts via env.DB.prepare().run() per statement.
// Parse semicolon-terminated statements here, in the Node-side config
// context, so multiline DDL remains intact and the worker-side setup file
// only deals with plain strings (no filesystem access is available from
// inside workerd). The schema contains no triggers or string literals with
// semicolons; if that changes, replace this small parser with a migration
// runner rather than weakening the tests.
const schemaSql = fs.readFileSync(
  path.join(import.meta.dirname, "schema.sql"),
  "utf8",
);
const schemaStatements = schemaSql
  .split("\n")
  .map((line) => line.replace(/--.*$/, ""))
  .join("\n")
  .split(";")
  .map((statement) => statement.trim())
  .filter(Boolean);

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
