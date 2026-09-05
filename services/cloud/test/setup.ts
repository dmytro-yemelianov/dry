import { env } from "cloudflare:workers";
import { beforeAll } from "vitest";

type TestEnv = Env & { TEST_SCHEMA_STATEMENTS: string[] };

const testEnv = env as unknown as TestEnv;

beforeAll(async () => {
  for (const statement of testEnv.TEST_SCHEMA_STATEMENTS) {
    // D1Database.exec treats each newline as a statement boundary, which
    // truncates multiline CREATE TABLE definitions. Each parsed entry is one
    // statement, so prepare/run preserves it exactly.
    await testEnv.DB.prepare(statement).run();
  }
});
