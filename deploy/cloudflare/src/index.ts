/**
 * Cloudflare Containers entrypoint for `containers/verify-runner`.
 *
 * The Worker is deliberately thin. It is a router and an admission gate, not a second
 * implementation: everything about *verification* lives in the container, which runs the same
 * `dry-core` call sequence the CLI does and whose byte-identity against the real `dry` binary is
 * pinned by `verify_report_is_byte_identical_to_the_real_cli`. A Worker that reimplemented any of
 * that would be the second divergent sketch ADR 0003 exists to prevent.
 */
import { Container, getRandom } from '@cloudflare/containers';

/** Body cap enforced here, before a container is started. Kept in step with `MAX_BODY_BYTES`. */
const MAX_BODY_BYTES = 104_857_600; // 100 MB

/**
 * How many instances `getRandom` spreads across. Must match `max_instances` in `wrangler.jsonc`:
 * a larger number here addresses instances that cannot exist, and a smaller one leaves paid
 * capacity idle.
 */
const INSTANCES = 5;

interface Env {
  VERIFY_RUNNER: DurableObjectNamespace<VerifyRunner>;
  ALLOWED_REGISTRY_HOST: string;
  MAX_BODY_BYTES: string;
  RUST_LOG: string;
}

export class VerifyRunner extends Container<Env> {
  defaultPort = 8080;
  requiredPorts = [8080];

  /** The container's own Docker healthcheck path; 2xx when the service is up. */
  pingEndpoint = '/healthz';

  /**
   * Verification is bursty — a user reviews a handful of programs, then stops. Ten minutes keeps a
   * warm instance across an editing session while still releasing it; a cold start is 2-3s, which
   * is tolerable for a request that already spends seconds importing g-code.
   */
  sleepAfter = '10m';

  /**
   * Required. The runner fetches the resolved profile from the printer registry over HTTPS before it
   * can verify anything; without outbound access every request would fail `502 profile-unavailable`
   * and the cause would not be obvious from the response.
   */
  enableInternet = true;
}

/** 100 MB is not a suggestion here: see the sizing note in `wrangler.jsonc`. */
function tooLarge(request: Request): boolean {
  const declared = request.headers.get('content-length');
  if (declared === null) return false; // chunked; the container's own limiter still applies
  const n = Number(declared);
  return Number.isFinite(n) && n > MAX_BODY_BYTES;
}

function problem(status: number, error: string, stage: string): Response {
  return new Response(JSON.stringify({ error, stage }) + '\n', {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);

    // Only the two public routes are proxied. `/metrics` is deliberately NOT among them: it reports
    // request counts, per-stage refusal counts and segment totals, which is operational detail about
    // other people's jobs and should not be world-readable. Scraping it needs a private route or an
    // authenticated one, and that is a D2 (dashboard) decision rather than something to leak by
    // default here.
    if (pathname === '/healthz') {
      const container = await getRandom(env.VERIFY_RUNNER, INSTANCES);
      await container.startAndWaitForPorts();
      return container.fetch(request);
    }

    if (pathname !== '/verify') {
      return problem(404, `no route for ${pathname}`, 'not-found');
    }
    if (request.method !== 'POST') {
      return problem(405, 'verify accepts POST', 'method-not-allowed');
    }

    // Refused before a container is started, so an oversized body cannot cost an instance. The
    // container enforces the same cap itself (`RequestBodyLimitLayer`); this is the cheap half of
    // the same rule, not a substitute for it.
    if (tooLarge(request)) {
      return problem(413, `body exceeds ${MAX_BODY_BYTES} bytes`, 'input-invalid');
    }

    // Verification is stateless — no session, no accumulated state, nothing to keep affinity with —
    // so requests spread across instances. `getContainer(binding, name)` would pin every caller to
    // one container for no benefit. There is no autoscaler: `max_instances` in `wrangler.jsonc` and
    // this call *are* the load balancing.
    const container = await getRandom(env.VERIFY_RUNNER, INSTANCES);

    // Not `start()`: that returns when the process has started, not when it is listening, and the
    // next line would then race it into "connection refused".
    await container.startAndWaitForPorts({
      ports: [8080],
      startOptions: {
        envVars: {
          ALLOWED_REGISTRY_HOST: env.ALLOWED_REGISTRY_HOST,
          MAX_BODY_BYTES: env.MAX_BODY_BYTES,
          RUST_LOG: env.RUST_LOG,
        },
      },
    });

    // The request — including its body — is streamed through rather than buffered in the Worker. A
    // Worker isolate has 128 MB, which is the measurement that ruled out running the engine here at
    // all (ADR 0003); holding the body would reintroduce that ceiling in the proxy.
    return container.fetch(request);
  },
};
