// `npm audit`, with "there is a vulnerability" and "I could not check" kept apart.
//
// `npm audit --audit-level=high` exits non-zero for two unrelated reasons: it found something at or
// above the threshold, or it never reached the advisory service. The workflow treated both as a
// failing gate, so an npm outage turned `main` red — which is what happened on 2026-09-04, when
// `/-/npm/v1/security/audits/quick` answered 503 and the docs-site job failed with
// `npm error audit endpoint returned an error` and no vulnerability anywhere in the output.
//
// A gate that reports "vulnerable" when it means "could not check" is the same defect this repo
// keeps finding in its own claims, pointed at CI. The two outcomes are now distinct:
//
//   vulnerabilities at or above `high`  -> exit 1, the gate fails, as before
//   advisory service unreachable        -> exit 0 with a ::warning::, and the log says plainly that
//                                          the gate did not run rather than that it passed
//   clean                               -> exit 0
//
// The severity decision is made here from `metadata.vulnerabilities` rather than delegated to npm's
// exit code, so the vulnerability signal never has to be inferred from the same number that also
// means "network".

import { execFileSync } from 'node:child_process';

const FAIL_AT = ['high', 'critical'];
const ATTEMPTS = 3;
const BACKOFF_MS = 15_000;
// npm's own retry/backoff against a dead endpoint ran for ~7 minutes in the 2026-09-04 outage, so
// three unbounded attempts would cost more job time than the bug did. Bound each attempt twice: npm
// gives up on a socket at 45s, and the process is killed at 90s if it ignores that.
const FETCH_TIMEOUT_MS = 45_000;
const ATTEMPT_TIMEOUT_MS = 90_000;

const UNREACHABLE = [
  'audit endpoint returned an error',
  'ENOTFOUND',
  'ETIMEDOUT',
  'ECONNRESET',
  'network timeout',
  'Service Unavailable',
];

function audit() {
  const args = ['audit', '--json', `--fetch-timeout=${FETCH_TIMEOUT_MS}`];
  try {
    return {
      stdout: execFileSync('npm', args, { encoding: 'utf8', timeout: ATTEMPT_TIMEOUT_MS }),
      stderr: '',
      timedOut: false,
    };
  } catch (err) {
    // Non-zero here is expected whenever anything was found; the caller decides what it means. A
    // kill from the `timeout` option arrives the same way but with empty output, so it has to be
    // reported separately — otherwise it reads as "npm failed in a way this gate does not
    // recognise" and fails the build for what is still just an unreachable service.
    return {
      stdout: err.stdout ?? '',
      stderr: err.stderr ?? '',
      timedOut: Boolean(err.killed) || err.signal != null || err.code === 'ETIMEDOUT',
    };
  }
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

for (let attempt = 1; attempt <= ATTEMPTS; attempt += 1) {
  const { stdout, stderr, timedOut } = audit();

  let report = null;
  try {
    report = JSON.parse(stdout);
  } catch {
    report = null;
  }

  const counts = report?.metadata?.vulnerabilities;
  if (counts) {
    const offending = FAIL_AT.filter((level) => (counts[level] ?? 0) > 0);
    const summary = Object.entries(counts)
      .map(([level, n]) => `${level}=${n}`)
      .join(' ');

    if (offending.length === 0) {
      console.log(`npm audit: clean at or above high (${summary})`);
      process.exit(0);
    }

    console.log(`npm audit: ${summary}`);
    for (const [name, v] of Object.entries(report.vulnerabilities ?? {})) {
      if (FAIL_AT.includes(v.severity)) {
        const via = (v.via ?? [])
          .map((entry) => (typeof entry === 'string' ? entry : entry.url ?? entry.title))
          .filter(Boolean)
          .join(', ');
        console.log(`  ${v.severity.padEnd(8)} ${name}${via ? ` — ${via}` : ''}`);
      }
    }
    console.log(`::error::npm audit found ${offending.join(' and ')} severity advisories`);
    process.exit(1);
  }

  // No parseable report. Either the service is unreachable, or npm failed in a way worth surfacing.
  const noise = `${stdout}\n${stderr}`;
  const unreachable = timedOut || UNREACHABLE.some((needle) => noise.includes(needle));
  if (!unreachable) {
    console.log(noise.trim());
    console.log('::error::npm audit failed in a way this gate does not recognise');
    process.exit(1);
  }

  if (attempt < ATTEMPTS) {
    console.log(`npm audit: advisory service unreachable (attempt ${attempt}/${ATTEMPTS}), retrying`);
    sleep(BACKOFF_MS);
  }
}

console.log(
  '::warning::npm audit SKIPPED — the advisory service was unreachable after ' +
    `${ATTEMPTS} attempts. This gate did not run; it did not pass. Re-run the job once ` +
    'https://status.npmjs.org recovers.',
);
process.exit(0);
