import { describe, expect, it } from "vitest";
import { registryHost } from "../src/container";

describe("registryHost", () => {
  it("returns the bare hostname WITHOUT a port -- must match Rust's Url::host_str(), which also excludes the port", () => {
    // Regression test: found via itest/jobs-local.sh against a real local stub
    // registry on a non-default port. An earlier version used JS's `URL.host`
    // (host **+ port**), which silently mismatched the runner's
    // `ALLOWED_REGISTRY_HOST` comparison (containers/verify-runner/src/lib.rs's
    // `validate_registry_url` uses `Url::host_str()`, host-only) for any registry
    // not on its scheme's default port -- e.g. a local stub on
    // `http://127.0.0.1:8823` -- causing every job to fail profile-unavailable.
    expect(registryHost("http://127.0.0.1:8823")).toBe("127.0.0.1");
    expect(registryHost("https://api.dry.yemelianov.dev")).toBe("api.dry.yemelianov.dev");
    expect(registryHost("https://api.dry.yemelianov.dev:8443/some/path")).toBe("api.dry.yemelianov.dev");
  });

  it("fails closed (empty string) for unset or unparseable input", () => {
    expect(registryHost(undefined)).toBe("");
    expect(registryHost("")).toBe("");
    expect(registryHost("not a url")).toBe("");
  });
});
