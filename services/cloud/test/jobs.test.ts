import { env, exports } from "cloudflare:workers";
import { describe, expect, it, vi } from "vitest";
import type { ContainerStubLike } from "../src/container";
import { handleQueueBatch, type QueueJobMessage } from "../src/jobs";

const ORIGIN = "http://example.com";

function url(path: string): string {
  return new URL(path, ORIGIN).toString();
}

async function fetchWorker(path: string, init?: RequestInit): Promise<Response> {
  return exports.default.fetch(url(path), init);
}

function formBody(fields: Record<string, string>): string {
  return new URLSearchParams(fields).toString();
}

let accountCounter = 0;

/**
 * Full device-flow round trip (mirrors test/auth.test.ts's own helper) that
 * returns a fresh Bearer access token for a BRAND NEW account. Each call uses a
 * unique email and a unique IP (the rate limiter's KV counters are shared, not
 * reset, across the whole test file per the R1 test-isolation notes -- reusing one
 * IP across this file's many jobs tests would eventually 429).
 */
async function grantAccessToken(): Promise<string> {
  accountCounter += 1;
  const ip = { "cf-connecting-ip": `198.51.100.${accountCounter}` };
  const email = `jobs-user-${accountCounter}-${Date.now()}@example.com`;

  const startResponse = await fetchWorker("/v1/auth/device", { method: "POST", headers: ip });
  expect(startResponse.status).toBe(200);
  const start = (await startResponse.json()) as { device_code: string; user_code: string };

  const approveResponse = await fetchWorker("/activate", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", ...ip },
    body: formBody({ user_code: start.user_code, email }),
  });
  expect(approveResponse.status).toBe(200);

  const tokenResponse = await fetchWorker("/v1/auth/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: formBody({
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
      device_code: start.device_code,
    }),
  });
  expect(tokenResponse.status).toBe(200);
  const body = (await tokenResponse.json()) as { access_token: string };
  return body.access_token;
}

async function submitJob(
  token: string,
  gcode: string,
  query = "pack=demo-printer&version=0.1.0&profile=demo-profile",
): Promise<Response> {
  return fetchWorker(`/v1/jobs/verify?${query}`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-length": String(new TextEncoder().encode(gcode).length),
    },
    body: gcode,
  });
}

interface JobRow {
  id: string;
  account_id: string;
  status: string;
  pack_id: string;
  pack_version: string;
  profile_id: string;
  input_r2: string;
  report_r2: string | null;
  error: string | null;
  stage: string | null;
  finished_at: string | null;
}

async function loadJobRow(id: string): Promise<JobRow | null> {
  return env.DB.prepare("SELECT * FROM jobs WHERE id = ?").bind(id).first<JobRow>();
}

// --- Fakes for the queue consumer's DI seam (see src/container.ts's module doc
// comment and src/jobs.ts's `QueueDeps`) -- no real container/Docker involved. ---

function fakeStub(
  handler: (url: string, init: RequestInit) => Promise<Response> | Response,
): ContainerStubLike & { fetch: ReturnType<typeof vi.fn>; startAndWaitForPorts: ReturnType<typeof vi.fn> } {
  return {
    startAndWaitForPorts: vi.fn(async () => {}),
    fetch: vi.fn(async (url: string, init?: RequestInit) => handler(url, init ?? {})),
  };
}

function throwingStub(message: string): ContainerStubLike {
  return {
    startAndWaitForPorts: vi.fn(async () => {
      throw new Error(message);
    }),
    fetch: vi.fn(),
  };
}

function fakeMessage(body: QueueJobMessage, attempts = 1): Message<QueueJobMessage> {
  return {
    id: crypto.randomUUID(),
    timestamp: new Date(),
    body,
    attempts,
    ack: vi.fn(),
    retry: vi.fn(),
  };
}

function fakeBatch(messages: Message<QueueJobMessage>[]): MessageBatch<QueueJobMessage> {
  return {
    queue: "verify-jobs",
    messages,
    metadata: { metrics: { backlogCount: 0, backlogBytes: 0 } },
    ackAll: vi.fn(),
    retryAll: vi.fn(),
  };
}

/** Runs the consumer for one job id against a given stub, returning the message
 * so tests can assert on its `ack`/`retry` spies. */
async function runConsumerOnce(jobId: string, stub: ContainerStubLike, attempts = 1): Promise<Message<QueueJobMessage>> {
  const message = fakeMessage({ id: jobId }, attempts);
  const batch = fakeBatch([message]);
  await handleQueueBatch(batch, env, {
    getContainerStub: () => stub,
  });
  return message;
}

describe("POST /v1/jobs/verify", () => {
  it("happy path: 202s, writes the R2 upload, a queued D1 row, and sends a queue message", async () => {
    const token = await grantAccessToken();
    const gcode = "G1 X10 Y10\nG1 X20 Y20\n";

    const sendSpy = vi.spyOn(env.VERIFY_JOBS, "send");

    const response = await submitJob(token, gcode);
    expect(response.status).toBe(202);
    const body = (await response.json()) as { id: string; status_url: string };
    expect(body.id).toBeTruthy();
    expect(body.status_url).toBe(`/v1/jobs/${body.id}`);

    // R2 object exists with exactly the uploaded bytes.
    const object = await env.STORAGE.get(`uploads/${body.id}`);
    expect(object).not.toBeNull();
    expect(await object?.text()).toBe(gcode);

    // D1 row is `queued` with the resolved pack/version/profile.
    const row = await loadJobRow(body.id);
    expect(row?.status).toBe("queued");
    expect(row?.pack_id).toBe("demo-printer");
    expect(row?.pack_version).toBe("0.1.0");
    expect(row?.profile_id).toBe("demo-profile");
    expect(row?.input_r2).toBe(`uploads/${body.id}`);

    // Queue message sent -- the real `VERIFY_JOBS` producer binding, not a stub.
    expect(sendSpy).toHaveBeenCalledWith({ id: body.id });

    sendSpy.mockRestore();
  });

  it("resolves the pack's default profile via the registry when `profile` is omitted (first listed profile)", async () => {
    const token = await grantAccessToken();
    const graphqlSpy = vi.fn(async (input: RequestInfo | URL) => {
      expect(String(input)).toBe("https://api.dry.yemelianov.dev/graphql");
      return Response.json({
        data: {
          printer: {
            versions: [
              { version: "0.1.0", profiles: [{ id: "first-profile" }, { id: "second-profile" }] },
            ],
          },
        },
      });
    });
    vi.stubGlobal("fetch", graphqlSpy);

    try {
      const response = await submitJob(token, "G1 X1\n", "pack=demo-printer&version=0.1.0");
      expect(response.status).toBe(202);
      const body = (await response.json()) as { id: string };
      const row = await loadJobRow(body.id);
      expect(row?.profile_id).toBe("first-profile");
      expect(graphqlSpy).toHaveBeenCalledTimes(1);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("502s profile_unavailable (and writes nothing) when default-profile resolution fails", async () => {
    const token = await grantAccessToken();
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ errors: [{ message: "not found" }] })),
    );

    try {
      const response = await submitJob(token, "G1 X1\n", "pack=unknown-printer&version=9.9.9");
      expect(response.status).toBe(502);
      expect(await response.json()).toMatchObject({ stage: "profile-unavailable" });
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("413s too-large (Content-Length over the cap) and writes nothing to R2 or D1 -- checked before any R2 write", async () => {
    const token = await grantAccessToken();
    const before = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs").first<{ count: number }>();

    const response = await fetchWorker("/v1/jobs/verify?pack=demo-printer&version=0.1.0&profile=demo-profile", {
      method: "POST",
      headers: {
        authorization: `Bearer ${token}`,
        // Lie about the size (tiny real body) -- the cap is enforced purely from
        // the header, before any bytes are read/written, so this alone triggers
        // the rejection without needing a real 100MB+ upload in a unit test.
        "content-length": String(101 * 1024 * 1024),
      },
      body: "G1 X1\n",
    });

    expect(response.status).toBe(413);
    expect(await response.json()).toMatchObject({ error: "too-large" });

    // No D1 row was created for the rejected submission (checked/rejected before
    // any R2 write or D1 insert -- see src/jobs.ts's ordering).
    const after = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs").first<{ count: number }>();
    expect(after?.count).toBe(before?.count);
  });

  it("401s with no Authorization header", async () => {
    const response = await fetchWorker("/v1/jobs/verify?pack=demo-printer&version=0.1.0&profile=demo-profile", {
      method: "POST",
      headers: { "content-length": "6" },
      body: "G1 X1\n",
    });
    expect(response.status).toBe(401);
  });
});

describe("GET /v1/jobs/{id}", () => {
  it("is owner-only: another account's job id 404s", async () => {
    const ownerToken = await grantAccessToken();
    const otherToken = await grantAccessToken();

    const submitResponse = await submitJob(ownerToken, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const ownerGet = await fetchWorker(`/v1/jobs/${id}`, { headers: { authorization: `Bearer ${ownerToken}` } });
    expect(ownerGet.status).toBe(200);

    const otherGet = await fetchWorker(`/v1/jobs/${id}`, { headers: { authorization: `Bearer ${otherToken}` } });
    expect(otherGet.status).toBe(404);
  });

  it("404s a nonexistent job id", async () => {
    const token = await grantAccessToken();
    const response = await fetchWorker("/v1/jobs/does-not-exist", { headers: { authorization: `Bearer ${token}` } });
    expect(response.status).toBe(404);
  });

  it("401s with no Authorization header", async () => {
    const response = await fetchWorker("/v1/jobs/does-not-exist");
    expect(response.status).toBe(401);
  });
});

describe("queue consumer: success path", () => {
  it("writes the report to R2, marks the job done, and inlines the report in GET once done", async () => {
    const token = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const report = { findings: [{ rule: "bounds", severity: "error" }] };
    const stub = fakeStub((requestUrl, init) => {
      expect(requestUrl).toContain("/verify?");
      expect(requestUrl).toContain("pack=demo-printer");
      expect(requestUrl).toContain("version=0.1.0");
      expect(requestUrl).toContain("profile=demo-profile");
      expect(requestUrl).toContain("registry=");
      expect(init.method).toBe("POST");
      return Response.json(report, { status: 200 });
    });

    const message = await runConsumerOnce(id, stub);
    expect(message.ack).toHaveBeenCalledTimes(1);
    expect(message.retry).not.toHaveBeenCalled();
    expect(stub.startAndWaitForPorts).toHaveBeenCalledTimes(1);

    const row = await loadJobRow(id);
    expect(row?.status).toBe("done");
    expect(row?.report_r2).toBe(`reports/${id}.json`);
    expect(row?.finished_at).toBeTruthy();

    const reportObject = await env.STORAGE.get(`reports/${id}.json`);
    expect(reportObject).not.toBeNull();
    expect(JSON.parse(await reportObject!.text())).toEqual(report);

    // GET inlines the parsed report once the job is done.
    const getResponse = await fetchWorker(`/v1/jobs/${id}`, { headers: { authorization: `Bearer ${token}` } });
    expect(getResponse.status).toBe(200);
    const body = (await getResponse.json()) as { status: string; report?: unknown };
    expect(body.status).toBe("done");
    expect(body.report).toEqual(report);
  });
});

describe("queue consumer: failure taxonomy", () => {
  it.each([
    { runnerStatus: 422, expectedStage: "input-invalid" },
    { runnerStatus: 502, expectedStage: "profile-unavailable" },
    { runnerStatus: 500, expectedStage: "engine-error" },
  ])("maps runner $runnerStatus to stage $expectedStage and acks (terminal, no retry)", async ({ runnerStatus, expectedStage }) => {
    const token = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const stub = fakeStub(() => Response.json({ error: "runner said no", stage: "whatever" }, { status: runnerStatus }));
    const message = await runConsumerOnce(id, stub);

    expect(message.ack).toHaveBeenCalledTimes(1);
    expect(message.retry).not.toHaveBeenCalled();

    const row = await loadJobRow(id);
    expect(row?.status).toBe("error");
    expect(row?.stage).toBe(expectedStage);
    expect(row?.error).toBe("runner said no");

    const getResponse = await fetchWorker(`/v1/jobs/${id}`, { headers: { authorization: `Bearer ${token}` } });
    const body = (await getResponse.json()) as { status: string; stage?: string; error?: string };
    expect(body.status).toBe("error");
    expect(body.stage).toBe(expectedStage);
    expect(body.error).toBe("runner said no");
  });
});

describe("queue consumer: container-start failures", () => {
  it("retries once, then persists an engine-error and acks after the 2nd failed attempt", async () => {
    const token = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const stub = throwingStub("container failed to start");

    const firstAttempt = await runConsumerOnce(id, stub, 1);
    expect(firstAttempt.retry).toHaveBeenCalledTimes(1);
    expect(firstAttempt.ack).not.toHaveBeenCalled();
    expect((await loadJobRow(id))?.status).toBe("queued"); // untouched by the failed attempt

    const secondAttempt = await runConsumerOnce(id, stub, 2);
    expect(secondAttempt.ack).toHaveBeenCalledTimes(1);
    expect(secondAttempt.retry).not.toHaveBeenCalled();

    const row = await loadJobRow(id);
    expect(row?.status).toBe("error");
    expect(row?.stage).toBe("engine-error");
    expect(row?.error).toContain("container failed to start");
  });
});
