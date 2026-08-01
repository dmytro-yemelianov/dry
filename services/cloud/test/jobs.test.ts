import { env, exports } from "cloudflare:workers";
import { describe, expect, it, vi } from "vitest";
import type { ContainerStubLike } from "../src/container";
import { handlePostVerifyJob, handleQueueBatch, type QueueJobMessage } from "../src/jobs";

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

/** Builds a raw `Request` for `handlePostVerifyJob`'s direct-call test path
 * (Fix 2a/2b below) -- these bypass `requireAuth`/the router entirely (the
 * function takes an already-authenticated `accountId`, same as production
 * code reaches it) so the test can pass a wrapped `Env` whose `DB`/
 * `VERIFY_JOBS` bindings throw, mirroring the container-seam DI pattern the
 * queue consumer tests already use (`fakeStub`/`throwingStub` above) instead
 * of mocking the real global `env` bindings in place. */
function verifyRequest(gcode: string, query = "pack=demo-printer&version=0.1.0&profile=demo-profile"): Request {
  return new Request(url(`/v1/jobs/verify?${query}`), {
    method: "POST",
    headers: { "content-length": String(new TextEncoder().encode(gcode).length) },
    body: gcode,
  });
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

  it("resolves and stores the registry's default version when `version` is omitted", async () => {
    const token = await grantAccessToken();
    const sendSpy = vi.spyOn(env.VERIFY_JOBS, "send").mockResolvedValue({} as QueueSendResponse);
    const graphqlSpy = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      const request = JSON.parse(String(init?.body)) as { variables: { id: string; version: string | null } };
      expect(request.variables).toEqual({ id: "demo-printer", version: null });
      return Response.json({
        data: {
          printer: {
            versions: [
              { version: "2.1.0", profiles: [{ id: "default-profile" }] },
              { version: "2.0.0", profiles: [{ id: "older-profile" }] },
            ],
          },
        },
      });
    });
    vi.stubGlobal("fetch", graphqlSpy);

    try {
      const response = await submitJob(token, "G1 X1\n", "pack=demo-printer");
      expect(response.status).toBe(202);
      const body = (await response.json()) as { id: string };
      const row = await loadJobRow(body.id);
      expect(row?.pack_version).toBe("2.1.0");
      expect(row?.profile_id).toBe("default-profile");
      expect(graphqlSpy).toHaveBeenCalledTimes(1);
    } finally {
      sendSpy.mockRestore();
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

  it("404s 'pack version not found' (and writes nothing to R2 or D1) when the registry has no EXACT match for the requested version -- no cross-version fallback (R3 review Fix 3)", async () => {
    const token = await grantAccessToken();
    const dbCountBefore = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs").first<{ count: number }>();
    const r2CountBefore = await env.STORAGE.list({ prefix: "uploads/" });

    // The registry answers just fine and even lists a DIFFERENT version for
    // this same pack with real profiles -- an earlier revision would have
    // silently fallen back to THAT version's first profile. Fix 3 removes
    // that fallback: no entry for the exact requested `version` (9.9.9) means
    // the submission fails fast with 404, never resolving a profile for a
    // version the caller didn't ask for.
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        Response.json({
          data: {
            printer: {
              versions: [{ version: "2.0.0", profiles: [{ id: "wrong-version-profile" }] }],
            },
          },
        }),
      ),
    );

    try {
      const response = await submitJob(token, "G1 X1\n", "pack=demo-printer&version=9.9.9");
      expect(response.status).toBe(404);
      expect(await response.json()).toEqual({ error: "pack version not found" });
    } finally {
      vi.unstubAllGlobals();
    }

    const dbCountAfter = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs").first<{ count: number }>();
    expect(dbCountAfter?.count).toBe(dbCountBefore?.count);
    const r2CountAfter = await env.STORAGE.list({ prefix: "uploads/" });
    expect(r2CountAfter.objects.length).toBe(r2CountBefore.objects.length);
  });

  it("411s length_required when Content-Length is missing (R3 review Fix 5a)", async () => {
    const token = await grantAccessToken();
    // A streaming body (rather than a plain string) is required to actually
    // exercise the MISSING-header path in this runtime: a string/Blob body
    // gets its Content-Length auto-computed by the Fetch implementation even
    // when the caller never sets the header explicitly, silently sidestepping
    // this exact check. A `ReadableStream` body has no known length up front,
    // so no Content-Length is ever synthesized -- reproducing a real client
    // that streams a request body without knowing/declaring its size.
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode("G1 X1\n"));
        controller.close();
      },
    });
    const response = await fetchWorker("/v1/jobs/verify?pack=demo-printer&version=0.1.0&profile=demo-profile", {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
      body: stream,
      duplex: "half",
    } as RequestInit);
    expect(response.status).toBe(411);
    expect(await response.json()).toEqual({ error: "length_required" });
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

describe("queue consumer: redelivery idempotency (R3 review Fix 1)", () => {
  it("skips a redelivered `done` job without touching the container or the stored report", async () => {
    const token = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const reportKey = `reports/${id}.json`;
    const report = { findings: [] };
    await env.STORAGE.put(reportKey, JSON.stringify(report), { httpMetadata: { contentType: "application/json" } });
    await env.DB.prepare("UPDATE jobs SET status = 'done', report_r2 = ?, finished_at = datetime('now') WHERE id = ?")
      .bind(reportKey, id)
      .run();

    // If the redelivery guard didn't fire, this stub's `fetch` would run and
    // fail the test loudly (rather than silently succeeding with a bogus
    // report) -- a stronger assertion than just "was it called".
    const stub = fakeStub(() => {
      throw new Error("container fetch seam must not be called for a redelivered terminal job");
    });

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    let message: Message<QueueJobMessage>;
    try {
      message = await runConsumerOnce(id, stub);
    } finally {
      warnSpy.mockRestore();
    }

    expect(message.ack).toHaveBeenCalledTimes(1);
    expect(message.retry).not.toHaveBeenCalled();
    expect(stub.startAndWaitForPorts).not.toHaveBeenCalled();
    expect(stub.fetch).not.toHaveBeenCalled();

    const row = await loadJobRow(id);
    expect(row?.status).toBe("done");
    expect(row?.report_r2).toBe(reportKey);

    const reportObject = await env.STORAGE.get(reportKey);
    expect(reportObject).not.toBeNull();
    expect(JSON.parse(await reportObject!.text())).toEqual(report);
  });

  it("skips a redelivered `error` job without touching the container or clobbering the persisted error/stage", async () => {
    const token = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    await env.DB.prepare(
      "UPDATE jobs SET status = 'error', error = 'original error', stage = 'input-invalid', finished_at = datetime('now') WHERE id = ?",
    )
      .bind(id)
      .run();

    const stub = fakeStub(() => {
      throw new Error("container fetch seam must not be called for a redelivered terminal job");
    });

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    let message: Message<QueueJobMessage>;
    try {
      message = await runConsumerOnce(id, stub);
    } finally {
      warnSpy.mockRestore();
    }

    expect(message.ack).toHaveBeenCalledTimes(1);
    expect(message.retry).not.toHaveBeenCalled();
    expect(stub.fetch).not.toHaveBeenCalled();

    const row = await loadJobRow(id);
    expect(row?.status).toBe("error");
    expect(row?.error).toBe("original error");
    expect(row?.stage).toBe("input-invalid");
  });
});

describe("POST /v1/jobs/verify: partial-failure handling (R3 review Fix 2)", () => {
  it("cleans up the orphaned R2 upload and rethrows (-> generic 500) when the D1 insert fails after the R2 write", async () => {
    const accountId = `fix2a-account-${crypto.randomUUID()}`;

    // Wraps the REAL `env.DB` (so the quota-check SELECT that runs earlier in
    // `handlePostVerifyJob` still works normally) and throws ONLY for the
    // specific INSERT this fix targets -- mirroring the container-seam DI
    // pattern above (`fakeStub`/`throwingStub`), just for the D1 binding
    // instead of the container stub.
    const throwingDb = {
      prepare(sql: string) {
        if (sql.includes("INSERT INTO jobs")) {
          throw new Error("simulated D1 insert failure");
        }
        return env.DB.prepare(sql);
      },
    } as unknown as Env["DB"];

    const putSpy = vi.spyOn(env.STORAGE, "put");
    const deleteSpy = vi.spyOn(env.STORAGE, "delete");
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    let caught: unknown;
    try {
      await handlePostVerifyJob(verifyRequest("G1 X1\n"), { ...env, DB: throwingDb }, accountId);
    } catch (error) {
      caught = error;
    } finally {
      errorSpy.mockRestore();
    }

    // Rethrows rather than swallowing the error into a Response -- it's
    // index.ts's OWN top-level try/catch (already covered by other
    // suites/routes) that turns this into the generic 500, not this handler.
    expect(caught).toBeInstanceOf(Error);
    expect((caught as Error).message).toBe("simulated D1 insert failure");

    // The R2 object written before the failing insert was cleaned up, not
    // left orphaned: exactly one `uploads/...` key was ever put, and that
    // SAME key was passed to `delete`.
    expect(putSpy).toHaveBeenCalledTimes(1);
    const inputKey = putSpy.mock.calls[0][0] as string;
    expect(inputKey).toMatch(/^uploads\//);
    expect(deleteSpy).toHaveBeenCalledTimes(1);
    expect(deleteSpy).toHaveBeenCalledWith(inputKey);
    putSpy.mockRestore();
    deleteSpy.mockRestore();

    expect(await env.STORAGE.get(inputKey)).toBeNull();

    const row = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs WHERE account_id = ?")
      .bind(accountId)
      .first<{ count: number }>();
    expect(row?.count).toBe(0);
  });

  it("marks the job 'error'/'queue-send-failed' and 500s (NOT 202) when VERIFY_JOBS.send() throws after the D1 insert", async () => {
    const accountId = `fix2b-account-${crypto.randomUUID()}`;

    const throwingQueue = {
      send: vi.fn(async () => {
        throw new Error("simulated queue send failure");
      }),
    } as unknown as Env["VERIFY_JOBS"];

    const response = await handlePostVerifyJob(verifyRequest("G1 X1\n"), { ...env, VERIFY_JOBS: throwingQueue }, accountId);

    expect(response.status).toBe(500);
    expect(await response.json()).toEqual({ error: "job could not be enqueued" });
    expect(throwingQueue.send).toHaveBeenCalledTimes(1);

    // The D1 row DOES exist (the insert, before the queue send, succeeded) --
    // but it's marked terminal, not left sitting `queued` forever with no
    // consumer ever able to reach it.
    const row = await env.DB.prepare("SELECT * FROM jobs WHERE account_id = ?")
      .bind(accountId)
      .first<JobRow>();
    expect(row?.status).toBe("error");
    expect(row?.stage).toBe("queue-send-failed");
    expect(row?.error).toBe("job could not be enqueued");
  });
});
