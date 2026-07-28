import { env } from "cloudflare:workers";
import { beforeAll } from "vitest";

type TestEnv = Env & { TEST_SCHEMA_STATEMENTS: string[] };

const testEnv = env as unknown as TestEnv;

beforeAll(async () => {
  for (const statement of testEnv.TEST_SCHEMA_STATEMENTS) {
    await testEnv.DB.exec(statement);
  }
});
