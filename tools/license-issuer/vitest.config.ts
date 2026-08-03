import { readFileSync } from "node:fs";
import path from "node:path";
import { cloudflareTest } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

// schema.sql is applied in test/issuer.test.ts's beforeAll via env.DB.exec()
// per statement. Split here, in the Node-side config context, so the
// worker-side test file only deals with plain strings (no filesystem
// access is available from inside workerd) -- mirrors services/cloud's
// vitest.config.ts pattern.
const schemaSql = readFileSync(path.join(import.meta.dirname, "schema.sql"), "utf8");
const schemaStatements = schemaSql
  .split(";")
  .map((statement) => statement.trim())
  .filter((statement) => statement.length > 0);

// The committed TEST Ed25519 keypair (crates/license/tests/fixtures) --
// explicitly non-secret, shared with the Rust cross-stack test (Task 2) so
// tests here can assert the emailed token verifies against the SAME public
// key crates/license/tests/cross_stack.rs checks.
const testKey = JSON.parse(
  readFileSync(
    path.join(import.meta.dirname, "../../crates/license/tests/fixtures/test-signing-key.json"),
    "utf8",
  ),
);

export default defineConfig({
  plugins: [
    cloudflareTest(async () => ({
      wrangler: {
        configPath: "./wrangler.jsonc",
        environment: "test",
      },
      miniflare: {
        bindings: {
          TEST_SCHEMA_STATEMENTS: schemaStatements,
          // Secrets: never committed as real production values, and only
          // ever wired up for this "test" environment.
          LS_WEBHOOK_SECRET: "test-webhook-secret",
          ADMIN_TOKEN: "test-admin-token",
          SIGNING_KEY_PKCS8_B64: testKey.signing_key_pkcs8_b64,
        },
      },
    })),
  ],
  test: {
    include: ["test/**/*.test.ts"],
  },
});
