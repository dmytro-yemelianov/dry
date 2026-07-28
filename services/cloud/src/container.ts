// The verify-runner container binding (`containers/verify-runner/`, Task R2) —
// wired up here as a Cloudflare Container-backed Durable Object.
//
// `VerifyContainer` itself is deliberately thin: `@cloudflare/containers`'s
// `Container` base class does all the start/stop/port-wait lifecycle work. The
// only thing this subclass adds is deriving `ALLOWED_REGISTRY_HOST` (the
// runner's SSRF allowlist, see containers/verify-runner/src/lib.rs) from the
// SAME `REGISTRY_URL` var the Worker itself uses (wrangler.jsonc's top-level
// `vars.REGISTRY_URL`, documented there as a load-bearing coupling for R7).
//
// `containerFetch` is a separate, narrow seam (not a method on this class) so
// `src/jobs.ts`'s queue consumer can inject a fake for unit tests: a real
// `Container` DO requires an actual container instance (Docker) to answer
// `startAndWaitForPorts()`/`fetch()`, which `vitest-pool-workers` cannot run —
// see test/jobs.test.ts's fake `ContainerStubLike` objects.

import { Container } from "@cloudflare/containers";

export class VerifyContainer extends Container<Env> {
  defaultPort = 8080;
  // Short-lived on purpose: verify jobs are one-shot request/response calls,
  // not long-lived sessions, so there is no reason to keep a container warm
  // for minutes between jobs. Keeps cost down; 2m is enough to absorb a burst
  // of jobs for the same job id (there is only ever one, since jobs are
  // one-per-container via `getByName(job_id)` — see src/jobs.ts) without
  // repeatedly paying the ~2-3s cold-start cost within a short window.
  sleepAfter = "2m";

  // `ConstructorParameters<typeof Container<Env>>` (rather than hand-typing the
  // `ctx` param) sidesteps a real structural mismatch between the ambient
  // `cloudflare:workers` `DurableObjectState` default type param and the one
  // `@cloudflare/containers`'s own `Container` constructor declares.
  constructor(...args: ConstructorParameters<typeof Container<Env>>) {
    super(...args);
    const [, env] = args;
    this.envVars = {
      ALLOWED_REGISTRY_HOST: registryHost(env.REGISTRY_URL),
    };
  }
}

/** Extracts the bare hostname (NO port) from a registry base URL, for the runner's
 * `ALLOWED_REGISTRY_HOST` env var. Deliberately `URL.hostname`, not `URL.host`:
 * the runner (containers/verify-runner/src/lib.rs's `validate_registry_url`)
 * compares against Rust's `Url::host_str()`, which also excludes the port -- using
 * JS's `.host` (host **+ port**) here would silently mismatch and reject every
 * registry that isn't on its default port (e.g. a local stub on
 * `http://127.0.0.1:8823`, exactly the case itest/jobs-local.sh exercises).
 * Empty string (refuses every fetch, fail-closed per the runner's own contract) if
 * `REGISTRY_URL` is unset or unparseable — never silently permissive. */
export function registryHost(registryUrl: string | undefined): string {
  if (!registryUrl) return "";
  try {
    return new URL(registryUrl).hostname;
  } catch {
    return "";
  }
}

/** Minimal surface `containerFetch` needs from a container stub — deliberately NOT
 * the full `DurableObjectStub<VerifyContainer>` RPC type, so tests can pass a plain
 * fake object (see test/jobs.test.ts) instead of a real Durable Object stub. */
export interface ContainerStubLike {
  startAndWaitForPorts(): Promise<void>;
  fetch(url: string, init?: RequestInit): Promise<Response>;
}

/** Real production accessor: `env.VERIFY_CONTAINER.getByName(jobId)` gives every job
 * its own container instance (isolation — see the Global Constraints' byte-identity
 * requirement and the R3 brief's "getByName(job_id) for isolation"). */
export function getContainerStub(env: Env, jobId: string): ContainerStubLike {
  return env.VERIFY_CONTAINER.getByName(jobId);
}

/**
 * Seam for calling the verify-runner container for one job. Isolated to its own
 * function (rather than inlined in the queue consumer) so `src/jobs.ts` can inject
 * a fake `ContainerStubLike` in unit tests without needing a real container
 * instance (Docker) — see the R3 task brief's "inject a containerFetch(stub) seam".
 *
 * Starts the container (idempotent/no-op if already running) and waits for its
 * HTTP port before issuing the request, per `@cloudflare/containers`'s own
 * guidance (`fetch()` alone does not wait for the port to be ready).
 */
export async function containerFetch(stub: ContainerStubLike, url: string, init: RequestInit): Promise<Response> {
  await stub.startAndWaitForPorts();
  return stub.fetch(url, init);
}
