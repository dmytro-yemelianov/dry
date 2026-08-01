#!/usr/bin/env node
// Minimal local stub of the public printer registry (dry-printer-registry /
// api.dry.yemelianov.dev), for itest/jobs-local.sh. Serves exactly the two
// routes the real system touches for one job:
//
//   GET  /v1/profiles/:pack/:version/:profile  -- REST artifact route
//        (docs/19-printer-registry-api.md), fetched by the verify-runner
//        container (containers/verify-runner/src/lib.rs's `fetch_profile`).
//   POST /graphql                              -- GraphQL, fetched by the
//        Worker's default-profile resolution (src/jobs.ts's
//        `resolveDefaultProfileId`) when a job omits `profile=`.
//
// Serves ONE real profile fixture (conformance/profile-matrix/marlin-pla-i3,
// the same fixture containers/verify-runner's own tests use) under
// pack=version=profile="marlin-pla-i3"/"0.1.0" -- matching the convention
// containers/verify-runner/tests/handler.rs already established.
//
// Usage: node stub-registry.mjs <path-to-profile.json> [port]

import { createServer } from "node:http";
import { readFileSync } from "node:fs";

const [, , profilePath, portArg] = process.argv;
if (!profilePath) {
  console.error("usage: node stub-registry.mjs <path-to-profile.json> [port]");
  process.exit(1);
}
const port = Number.parseInt(portArg ?? "8823", 10);

const PACK = "marlin-pla-i3";
const VERSION = "0.1.0";
const PROFILE_ID = "marlin-pla-i3";

const profileJson = readFileSync(profilePath, "utf8");

const server = createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://stub-registry");

  if (req.method === "GET" && url.pathname === `/v1/profiles/${PACK}/${VERSION}/${PROFILE_ID}`) {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(profileJson);
    return;
  }

  if (req.method === "POST" && url.pathname === "/graphql") {
    let body = "";
    req.setEncoding("utf8");
    req.on("data", (chunk) => {
      body += chunk;
    });
    req.on("end", () => {
      let requestedVersion = VERSION;
      try {
        const parsed = JSON.parse(body);
        if (typeof parsed?.variables?.version === "string") requestedVersion = parsed.variables.version;
      } catch {
        // malformed body -- fall through with the default version
      }
      res.writeHead(200, { "content-type": "application/json" });
      res.end(
        JSON.stringify({
          data: {
            printer: {
              versions: [{ version: requestedVersion, profiles: [{ id: PROFILE_ID }] }],
            },
          },
        }),
      );
    });
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: "not_found", path: url.pathname }));
});

server.listen(port, "127.0.0.1", () => {
  console.log(`stub-registry: listening on http://127.0.0.1:${port} (pack=${PACK} version=${VERSION} profile=${PROFILE_ID})`);
});

process.on("SIGTERM", () => server.close(() => process.exit(0)));
process.on("SIGINT", () => server.close(() => process.exit(0)));
