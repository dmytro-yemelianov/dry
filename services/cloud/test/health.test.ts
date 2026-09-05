import { exports } from "cloudflare:workers";
import { describe, expect, it } from "vitest";

const HEALTH_URL = "http://example.com/healthz";

describe("GET /healthz", () => {
  it("reports control-plane liveness without starting a verifier container", async () => {
    const response = await exports.default.fetch(HEALTH_URL);

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ ok: true, service: "dry-cloud-control-plane" });
  });

  it("rejects non-GET methods", async () => {
    const response = await exports.default.fetch(HEALTH_URL, { method: "POST" });

    expect(response.status).toBe(405);
    expect(response.headers.get("allow")).toBe("GET");
    expect(await response.json()).toEqual({ error: "method_not_allowed" });
  });
});
