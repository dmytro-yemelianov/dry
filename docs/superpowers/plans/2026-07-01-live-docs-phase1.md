# Live Docs — Phase 1 (toolchain + LiveExample + engine split + Guide) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an interactive VitePress docs site whose editable TypeScript snippets run the *real* `@dry/sdk` `Design` API against the wasm engine and re-render canvas + g-code + IR + metrics + verify in realtime, with a 6-page guided tour.

**Architecture:** Split the wasm loader out of `@dry/sdk`'s `engine.ts` so the platform-agnostic `Design` API can run in a browser (Approach A from the spec). A VitePress app under `docs/site/` imports the agnostic SDK source directly, initialises the existing web-target wasm (`web/pkg/`) once, and a `<LiveExample>` Vue component edits/transpiles/evaluates snippets live. A Playwright smoke asserts every example's browser output matches the engine run in Node — so docs can't drift from the engine.

**Tech Stack:** TypeScript, VitePress 1 (Vue 3 + Vite), CodeMirror 6, sucrase, the existing wasm-bindgen `--target web` engine, Vitest + @vue/test-utils + jsdom (unit), Playwright (smoke).

> **Implementation status:** complete. This document records the original task sequence; the checked-in
> source and tests are canonical where the implementation evolved (notably Worker-isolated snippet
> execution, teardown guards, parser-aware module transforms, and stale-output clearing).

## Global Constraints

- **One engine, no fork.** The browser must run the genuine `@dry/sdk` `Design` API; no re-implemented browser `Design`, no parallel resolve logic. (Spec decision 4.)
- **SDK public API unchanged.** The `engine.ts` split is internal and non-breaking; `import { Design } from '@dry/sdk'` on Node keeps auto-initialising. The existing `sdk/ts` Node tests and `web/smoke.cjs` must stay green.
- **No `node:` import in the browser graph.** The docs site imports only the agnostic SDK modules (`design.ts`, `ops.ts`, `generators/*`, agnostic `engine.ts`, `engine.web.ts`) — never `index.ts` (which pulls in node-only `engine.node.ts`).
- **wasm-bindgen pinned `=0.2.123`** (the repo's triple-maintained pin); reuse `web/build.sh web` output — **no new wasm target**.
- **Node ≥ 20.** Run all `docs/site` commands with `npm --prefix docs/site …`.
- **Branch:** `feat/live-docs` (already created; the spec is committed there).
- **The web wasm glue** (`web/pkg/dry_wasm.js`) exports, by name: `resolve_gcode`, `resolve_ir`, `resolve_metrics`, `metrics_ir`, `resolve_binary`, `resolve_optimized_ir`, `resolve_balanced_ir`, `resolve_verify`, `tpms_ops_json`, plus a **default** async init (`__wbg_init`). It fetches `dry_wasm_bg.wasm` next to its own URL.

---

## File map

| File | Responsibility |
|---|---|
| `sdk/ts/src/engine.ts` (modify) | Binding-agnostic resolve wrappers + `setWasmBinding`/`bind` slot; exports `DryWasm` |
| `sdk/ts/src/engine.node.ts` (create) | Node loader: `createRequire` the nodejs-target glue → `setWasmBinding` |
| `sdk/ts/src/engine.web.ts` (create) | Browser loader: `initDryWeb(url)` loads web glue, `await default()`, `setWasmBinding` |
| `sdk/ts/src/index.ts` (modify) | `import './engine.node';` side-effect first (preserve Node auto-init) |
| `sdk/ts/test/engine-init.test.ts` (create) | Asserts the agnostic engine throws before a binding is set |
| `docs/site/package.json` (create) | Deps + scripts (dev/build/preview/typecheck/test/smoke) |
| `docs/site/.vitepress/config.ts` (create) | Site config, nav, sidebar, `@sdk` alias, fs.allow |
| `docs/site/.vitepress/theme/index.ts` (create/modify) | Extend default theme; register `<LiveExample>` (registration added in Task 6) |
| `docs/site/.vitepress/theme/dry-engine.ts` (create) | `initDryEngine()` singleton + `getDry()` injected object |
| `docs/site/.vitepress/theme/run-snippet.ts` (create) | `compileSnippet(src)` → `(dry)=>result` (sucrase strip + eval) — shared by browser & smoke |
| `docs/site/.vitepress/theme/render-ir.ts` (create) | `computeViewBox` + `drawIr` (IR → 2D canvas) |
| `docs/site/.vitepress/theme/LiveExample.vue` (create) | Editor (CodeMirror) + live demo (canvas + tabs) |
| `docs/site/examples/*.ts` (create) | The real, type-checked example snippets |
| `docs/site/{index.md,guide/*.md}` (create) | Landing + 6 guide pages |
| `docs/site/build.sh` (create) | Build web wasm → copy to `public/pkg/` → `vitepress build` |
| `docs/site/tsconfig.examples.json` (create) | Type-check `examples/*.ts` against the SDK source |
| `docs/site/vitest.config.ts` (create) | Unit-test config (jsdom + `@sdk` alias) |
| `docs/site/smoke/examples.spec.ts` (create) | Playwright anti-drift smoke |
| `docs/site/playwright.config.ts` (create) | Playwright config (build + preview server) |
| `.github/workflows/ci.yml` (modify) | Add a `docs-site` job |

---

### Task 1: Split the SDK wasm loader (browser-ready, non-breaking)

**Files:**
- Modify: `sdk/ts/src/engine.ts`
- Create: `sdk/ts/src/engine.node.ts`
- Create: `sdk/ts/src/engine.web.ts`
- Modify: `sdk/ts/src/index.ts:11` (add side-effect import)
- Test: `sdk/ts/test/engine-init.test.ts`

**Interfaces:**
- Produces: `setWasmBinding(b: DryWasm): void`, `DryWasm` (exported interface), unchanged `resolveGcode/resolveMetrics/resolveMetricsIr/resolveIr/resolveBinary/resolveOptimizedIr/resolveBalancedIr/resolveVerify/tpmsOps` signatures, `initDryWeb(wasmUrl: string): Promise<void>`.
- Consumes: the web glue's named exports + default init (Global Constraints).

- [ ] **Step 1: Write the failing test** — `sdk/ts/test/engine-init.test.ts`

```ts
// Run in its own process (node --test uses process isolation by default on Node 20+),
// importing ONLY the agnostic engine — so no binding is ever set and bind() must throw.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveGcode } from '../src/engine';
import { RESOLVE_PARAMS } from '../src/ops';

test('agnostic engine throws before a wasm binding is set', () => {
  assert.throws(
    () => resolveGcode([{ op: 'move', x: 0, y: 0, z: 0 }], RESOLVE_PARAMS),
    /not initialised/i
  );
});
```

- [ ] **Step 2: Run it — expect FAIL**

Run: `cd sdk/ts && npx tsc -p tsconfig.json && node --test dist/test/engine-init.test.js`
Expected: FAIL — today `engine.ts` auto-loads the nodejs wasm at import, so `resolveGcode` succeeds and does not throw.

- [ ] **Step 3: Make `engine.ts` binding-agnostic** — replace the node import + module-load binding (top of the file, currently `sdk/ts/src/engine.ts:4-6` and `:54-57`).

Replace:

```ts
import * as path from 'node:path';
import { createRequire } from 'node:module';
import type { Metrics, Op, Report, ResolveParams, Toolpath } from './ops';
```

with:

```ts
import type { Metrics, Op, Report, ResolveParams, Toolpath } from './ops';
```

Then change `interface DryWasm {` to `export interface DryWasm {` (line ~21). Replace the binding load (lines ~54-57):

```ts
// compiled to dist/src/engine.js, so the wasm dir is two levels up (dist/src -> dist -> ts/wasm... two
// `..` reach the package root). Resolved relative to this file so it works regardless of cwd.
const requireWasm = createRequire(__filename);
const wasm: DryWasm = requireWasm(path.join(__dirname, '..', '..', 'wasm', 'dry_wasm.js'));
```

with:

```ts
// The wasm binding is injected by a platform loader (engine.node.ts on Node, engine.web.ts in the
// browser). Keeping engine.ts binding-agnostic is what lets the same Design API run client-side.
let wasm: DryWasm | undefined;

/** Install the resolved wasm binding. Called once by a platform loader before any resolve call. */
export function setWasmBinding(binding: DryWasm): void {
  wasm = binding;
}

function bind(): DryWasm {
  if (!wasm) {
    throw new Error(
      'Dry wasm engine not initialised: import the Node entry (@dry/sdk) or call initDryWeb() first'
    );
  }
  return wasm;
}
```

- [ ] **Step 4: Route every wrapper through `bind()`** — in `sdk/ts/src/engine.ts`, replace each `wasm.` call with `bind().`. There are nine call sites (`wasm.resolve_gcode`, `wasm.tpms_ops_json`, `wasm.resolve_metrics`, `wasm.metrics_ir`, `wasm.resolve_ir`, `wasm.resolve_binary`, `wasm.resolve_optimized_ir`, `wasm.resolve_balanced_ir`, `wasm.resolve_verify`). Example — `resolveGcode`:

```ts
export function resolveGcode(
  ops: Op[],
  params: ResolveParams,
  relativeE = true,
  travelG1E0 = false,
  fiveAxis = false,
  rotaryAxes = 'ab'
): string[] {
  return bind().resolve_gcode(
    JSON.stringify(ops),
    JSON.stringify(params),
    relativeE,
    travelG1E0,
    fiveAxis,
    rotaryAxes
  );
}
```

Apply the identical `wasm.` → `bind().` swap to the other eight wrappers, leaving their bodies otherwise unchanged.

- [ ] **Step 5: Create the Node loader** — `sdk/ts/src/engine.node.ts`

```ts
// Node loader for the Dry wasm engine. Loads the nodejs-target glue (built into ../../wasm by build.sh)
// and installs it as the engine binding. index.ts imports this for its side effect, so the published
// Node package keeps auto-initialising on import — behaviour-identical to the pre-split engine.ts.
import * as path from 'node:path';
import { createRequire } from 'node:module';
import { setWasmBinding, type DryWasm } from './engine';

const requireWasm = createRequire(__filename);
// compiled to dist/src/engine.node.js → two `..` reach the package root, then /wasm.
const wasm = requireWasm(path.join(__dirname, '..', '..', 'wasm', 'dry_wasm.js')) as DryWasm;
setWasmBinding(wasm);
```

- [ ] **Step 6: Create the browser loader** — `sdk/ts/src/engine.web.ts`

```ts
// Browser loader for the Dry wasm engine. Dynamically loads the web-target glue from a runtime URL
// (the docs site copies web/pkg/ into its public assets), runs the async wasm init, and installs the
// binding. NOTE: takes the URL as a parameter and uses NO `import.meta` so it still type-checks under
// the SDK's commonjs tsc build (it is never *run* on Node — only the docs Vite build executes it).
import { setWasmBinding, type DryWasm } from './engine';

let initPromise: Promise<void> | undefined;

/** Load + initialise the web-target wasm exactly once and install it as the engine binding. */
export function initDryWeb(wasmUrl: string): Promise<void> {
  if (!initPromise) {
    const attempt = (async () => {
      const glue: Record<string, unknown> = await import(/* @vite-ignore */ wasmUrl);
      // wasm-bindgen --target web: default export is the async init; it fetches dry_wasm_bg.wasm
      // relative to the glue's own URL.
      await (glue.default as () => Promise<unknown>)();
      const fn = (k: string) => glue[k] as DryWasm[keyof DryWasm];
      setWasmBinding({
        resolve_gcode: fn('resolve_gcode'),
        tpms_ops_json: fn('tpms_ops_json'),
        resolve_metrics: fn('resolve_metrics'),
        metrics_ir: fn('metrics_ir'),
        resolve_ir: fn('resolve_ir'),
        resolve_binary: fn('resolve_binary'),
        resolve_optimized_ir: fn('resolve_optimized_ir'),
        resolve_balanced_ir: fn('resolve_balanced_ir'),
        resolve_verify: fn('resolve_verify'),
      } as DryWasm);
    })();
    initPromise = attempt.catch((error: unknown) => {
      initPromise = undefined;
      throw error;
    });
  }
  return initPromise;
}
```

- [ ] **Step 7: Preserve Node auto-init** — `sdk/ts/src/index.ts`, add as the **first** line (before the existing `export { Design } from './design';`):

```ts
import './engine.node'; // side effect: install the Node wasm binding on import (Node entry only)
```

- [ ] **Step 8: Run the new test — expect PASS, then the full SDK suite — expect PASS**

Run: `cd sdk/ts && bash build.sh && node --test dist/test/engine-init.test.js && npm test`
Expected: `engine-init` PASSES (agnostic engine throws); the existing `conformance`/`parity`/`kinematics`/`tpms-delegation`/`verify-input` suites all PASS (the Node path still auto-inits via `index.ts`).

- [ ] **Step 9: Confirm the byte-identity smoke still passes**

Run: `cd /Users/dmytro/Documents/github/dry && bash web/build.sh && node web/smoke.cjs`
Expected: the existing wasm byte-identity smoke prints its pass line (9/9) — the split is behaviour-preserving.

- [ ] **Step 10: Commit**

```bash
git add sdk/ts/src/engine.ts sdk/ts/src/engine.node.ts sdk/ts/src/engine.web.ts sdk/ts/src/index.ts sdk/ts/test/engine-init.test.ts
git commit -m "refactor(ts): split wasm loader so the Design API runs in a browser (non-breaking)"
```

---

### Task 2: VitePress scaffold that builds

**Files:**
- Create: `docs/site/package.json`
- Create: `docs/site/.vitepress/config.ts`
- Create: `docs/site/.vitepress/theme/index.ts`
- Create: `docs/site/index.md`
- Create: `docs/site/build.sh`
- Create: `docs/site/.gitignore`

**Interfaces:**
- Produces: a buildable site; the `@sdk` Vite alias → `<repo>/sdk/ts/src`; `public/pkg/` served at `/pkg/`.

- [ ] **Step 1: Create `docs/site/package.json`**

```json
{
  "name": "@dry/docs-site",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vitepress dev",
    "build": "vitepress build",
    "preview": "vitepress preview --port 4173",
    "wasm": "bash build.sh wasm-only",
    "typecheck": "tsc -p tsconfig.examples.json --noEmit",
    "test": "vitest run",
    "smoke": "playwright test"
  },
  "devDependencies": {
    "vitepress": "^1.6.3",
    "vue": "^3.5.13",
    "sucrase": "^3.35.0",
    "codemirror": "^6.0.1",
    "@codemirror/lang-javascript": "^6.2.2",
    "@codemirror/state": "^6.5.0",
    "@codemirror/view": "^6.36.1",
    "vitest": "^2.1.8",
    "@vue/test-utils": "^2.4.6",
    "jsdom": "^25.0.1",
    "@playwright/test": "^1.49.1",
    "typescript": "^5.6.0",
    "@types/node": "^22.0.0"
  }
}
```

- [ ] **Step 2: Create `docs/site/.gitignore`**

```
node_modules/
.vitepress/cache/
.vitepress/dist/
public/pkg/
test-results/
playwright-report/
```

- [ ] **Step 3: Create `docs/site/.vitepress/config.ts`**

```ts
import { defineConfig } from 'vitepress';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url)); // docs/site/.vitepress
const repoRoot = path.resolve(here, '../../..'); // -> repo root
const sdkSrc = path.resolve(repoRoot, 'sdk/ts/src');

export default defineConfig({
  title: 'Dry',
  description: 'Interactive docs for the Dry toolpath compiler — editable code, live execution.',
  themeConfig: {
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'Gallery', link: 'https://github.com/dmytro-yemelianov/dry' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'Overview', link: '/guide/' },
            { text: '1. Author a path', link: '/guide/author' },
            { text: '2. Lower to the Dry IR', link: '/guide/lower' },
            { text: '3. Simulate', link: '/guide/simulate' },
            { text: '4. Verify', link: '/guide/verify' },
            { text: '5. Optimize', link: '/guide/optimize' },
            { text: '6. Generative', link: '/guide/generative' },
          ],
        },
      ],
    },
  },
  vite: {
    resolve: { alias: { '@sdk': sdkSrc } },
    server: { fs: { allow: [repoRoot] } },
  },
});
```

- [ ] **Step 4: Create `docs/site/.vitepress/theme/index.ts`** (registration of `<LiveExample>` is added in Task 6)

```ts
import DefaultTheme from 'vitepress/theme';
import type { Theme } from 'vitepress';

export default {
  extends: DefaultTheme,
  enhanceApp() {
    // <LiveExample> is registered here in Task 6.
  },
} satisfies Theme;
```

- [ ] **Step 5: Create `docs/site/index.md`** (landing — prose only for now; hero LiveExample lands in Task 7)

```md
---
layout: home
hero:
  name: Dry
  text: Toolpath compiler — live docs
  tagline: Edit the code, watch the engine run. The same Rust/wasm engine the CLI and SDKs use.
  actions:
    - theme: brand
      text: Start the tour
      link: /guide/
---
```

- [ ] **Step 6: Create `docs/site/build.sh`**

```bash
#!/usr/bin/env bash
# Build the live-docs site: (1) build the web-target wasm engine, (2) copy it into the site's public
# assets so the browser can load it at /pkg/, (3) build the VitePress site. Pass "wasm-only" to stop
# after the copy (used by `npm run wasm` before dev/test/smoke).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"

bash "$ROOT/web/build.sh" web "$ROOT/web/pkg"
mkdir -p "$HERE/public/pkg"
cp "$ROOT/web/pkg/dry_wasm.js" "$ROOT/web/pkg/dry_wasm_bg.wasm" "$HERE/public/pkg/"
echo "copied web wasm -> $HERE/public/pkg/"

[ "${1:-}" = "wasm-only" ] && exit 0
npm --prefix "$HERE" run build
echo "built docs site -> $HERE/.vitepress/dist"
```

- [ ] **Step 7: Install, copy wasm, and build — expect success**

Run:
```bash
cd docs/site && npm install && chmod +x build.sh && bash build.sh wasm-only && npm run build
```
Expected: `vitepress build` completes; `.vitepress/dist/index.html` exists; `public/pkg/dry_wasm.js` and `dry_wasm_bg.wasm` exist.

- [ ] **Step 8: Commit**

```bash
git add docs/site/package.json docs/site/.gitignore docs/site/.vitepress docs/site/index.md docs/site/build.sh
git commit -m "feat(docs-site): VitePress scaffold + wasm copy build step"
```

---

### Task 3: Browser engine singleton + injected `dry` object

**Files:**
- Create: `docs/site/.vitepress/theme/dry-engine.ts`
- Create: `docs/site/tsconfig.examples.json`
- Create: `docs/site/vitest.config.ts`
- Test: `docs/site/.vitepress/theme/dry-engine.test.ts`

**Interfaces:**
- Consumes: `@sdk/engine` (`setWasmBinding`, `initDryWeb` via `@sdk/engine.web`), `@sdk/design` (`Design`), `@sdk/ops` (`PRINTERS`), `@sdk/generators/tpms` (`tpms`), `@sdk/generators/starPolygonLattice` (`starPolygonLattice`).
- Produces: `getDry(): Dry` (the object injected into snippets), `initDryEngine(): Promise<void>` (idempotent), `type Dry`.

- [ ] **Step 1: Create `docs/site/vitest.config.ts`**

```ts
import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '../..');

export default defineConfig({
  resolve: { alias: { '@sdk': path.resolve(repoRoot, 'sdk/ts/src') } },
  test: { environment: 'jsdom', include: ['.vitepress/theme/**/*.test.ts', 'smoke/**/*.unit.test.ts'] },
});
```

- [ ] **Step 2: Create `docs/site/tsconfig.examples.json`**

```json
{
  "compilerOptions": {
    "target": "ES2021",
    "module": "esnext",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": ["node"],
    "paths": { "@dry/sdk": ["../../sdk/ts/src/index.ts"], "@sdk/*": ["../../sdk/ts/src/*"] }
  },
  "include": ["examples", ".vitepress/theme"]
}
```

- [ ] **Step 3: Write the failing test** — `docs/site/.vitepress/theme/dry-engine.test.ts`

```ts
import { test, expect } from 'vitest';
import { setWasmBinding, type DryWasm } from '@sdk/engine';
import { getDry } from './dry-engine';

// A fake binding: resolve_ir returns a one-segment IR so we can prove the real Design API drives it.
const fake = {
  resolve_ir: () =>
    JSON.stringify({ version: 1, segments: [{ start: [0, 0, 0], end: [10, 0, 0], travel: false, kind: 'line' }] }),
  resolve_gcode: () => ['G1 X10 Y0'],
} as unknown as DryWasm;

test('getDry() exposes the real Design API over the injected binding', () => {
  setWasmBinding(fake);
  const dry = getDry();
  expect(typeof dry.Design).toBe('function');
  expect(typeof dry.tpms).toBe('function');
  const ir = new dry.Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2).ir();
  expect(ir.segments).toHaveLength(1);
  expect(dry.PRINTERS.generic.dia).toBe(1.75);
});
```

- [ ] **Step 4: Run it — expect FAIL**

Run: `cd docs/site && npx vitest run .vitepress/theme/dry-engine.test.ts`
Expected: FAIL — `./dry-engine` does not exist.

- [ ] **Step 5: Create `docs/site/.vitepress/theme/dry-engine.ts`**

```ts
// The browser-side engine singleton. Initialises the web-target wasm once, then hands snippets a `dry`
// object that is literally the published @dry/sdk surface (Design + resolve fns + generators), so an
// edited example runs the genuine API — byte-identical to the CLI/Python/Node.
import { Design } from '@sdk/design';
import { PRINTERS } from '@sdk/ops';
import {
  resolveGcode, resolveMetrics, resolveMetricsIr, resolveIr,
  resolveBinary, resolveOptimizedIr, resolveBalancedIr, resolveVerify,
} from '@sdk/engine';
import { initDryWeb } from '@sdk/engine.web';
import { tpms } from '@sdk/generators/tpms';
import { starPolygonLattice } from '@sdk/generators/starPolygonLattice';

export interface Dry {
  Design: typeof Design;
  PRINTERS: typeof PRINTERS;
  resolveGcode: typeof resolveGcode;
  resolveMetrics: typeof resolveMetrics;
  resolveMetricsIr: typeof resolveMetricsIr;
  resolveIr: typeof resolveIr;
  resolveBinary: typeof resolveBinary;
  resolveOptimizedIr: typeof resolveOptimizedIr;
  resolveBalancedIr: typeof resolveBalancedIr;
  resolveVerify: typeof resolveVerify;
  tpms: typeof tpms;
  starPolygonLattice: typeof starPolygonLattice;
}

const dry: Dry = {
  Design, PRINTERS,
  resolveGcode, resolveMetrics, resolveMetricsIr, resolveIr,
  resolveBinary, resolveOptimizedIr, resolveBalancedIr, resolveVerify,
  tpms, starPolygonLattice,
};

export function getDry(): Dry {
  return dry;
}

let ready: Promise<void> | undefined;

/** Initialise the wasm engine exactly once. Safe to call from every component mount. */
export function initDryEngine(): Promise<void> {
  if (!ready) {
    const base = (import.meta.env?.BASE_URL ?? '/') as string;
    ready = initDryWeb(`${base}pkg/dry_wasm.js`);
  }
  return ready;
}
```

- [ ] **Step 6: Run the test — expect PASS, and typecheck the surface**

Run: `cd docs/site && npx vitest run .vitepress/theme/dry-engine.test.ts && npm run typecheck`
Expected: PASS; `tsc` reports no errors.

- [ ] **Step 7: Commit**

```bash
git add docs/site/.vitepress/theme/dry-engine.ts docs/site/.vitepress/theme/dry-engine.test.ts docs/site/tsconfig.examples.json docs/site/vitest.config.ts
git commit -m "feat(docs-site): browser engine singleton injecting the real @dry/sdk surface"
```

---

### Task 4: `compileSnippet` — strip TS imports, eval against the injected `dry`

**Files:**
- Create: `docs/site/.vitepress/theme/run-snippet.ts`
- Test: `docs/site/.vitepress/theme/run-snippet.test.ts`

**Interfaces:**
- Consumes: `sucrase`, `type Dry` from `./dry-engine`.
- Produces: `compileSnippet(src: string): (dry: Dry) => unknown` (throws on transpile error); `runSnippet(src: string, dry: Dry): { ok: true; value: unknown } | { ok: false; error: string }`.

- [ ] **Step 1: Write the failing test** — `docs/site/.vitepress/theme/run-snippet.test.ts`

```ts
import { test, expect } from 'vitest';
import { runSnippet } from './run-snippet';
import type { Dry } from './dry-engine';

const fakeDry = {
  Design: class {
    ops: unknown[] = [];
    geometry() { return this; }
    extruder() { return this; }
    point() { return this; }
    gcode() { return ['G1 X10 Y0']; }
  },
  tpms: () => ({ tag: 'tpms-design' }),
} as unknown as Dry;

test('idiomatic snippet with an @dry/sdk import runs and returns its last expression', () => {
  const src = `import { Design } from '@dry/sdk';\nnew Design().geometry(0.6, 0.2).extruder(true).point(0,0,0.2)`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('a throwing snippet is captured, not propagated', () => {
  const r = runSnippet(`throw new Error('boom')`, fakeDry);
  expect(r.ok).toBe(false);
  if (!r.ok) expect(r.error).toMatch(/boom/);
});

test('destructured generator import resolves from the injected dry', () => {
  const r = runSnippet(`import { tpms } from '@dry/sdk';\ntpms({ surface: 'gyroid' })`, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect(r.value).toEqual({ tag: 'tpms-design' });
});
```

- [ ] **Step 2: Run it — expect FAIL**

Run: `cd docs/site && npx vitest run .vitepress/theme/run-snippet.test.ts`
Expected: FAIL — `./run-snippet` does not exist.

- [ ] **Step 3: Create `docs/site/.vitepress/theme/run-snippet.ts`**

```ts
// Turn an editable TypeScript snippet into a function of the injected `dry` object. We strip types and
// import/export with sucrase, then prepend a destructure of `dry` so an idiomatic
// `import { Design } from '@dry/sdk'` resolves to the real SDK at eval time. The snippet's final
// expression is returned (sucrase's 'imports' transform leaves the trailing expression as the value).
import { transform } from 'sucrase';
import type { Dry } from './dry-engine';

const KEYS = [
  'Design', 'PRINTERS', 'resolveGcode', 'resolveMetrics', 'resolveMetricsIr', 'resolveIr',
  'resolveBinary', 'resolveOptimizedIr', 'resolveBalancedIr', 'resolveVerify', 'tpms', 'starPolygonLattice',
];

export function compileSnippet(src: string): (dry: Dry) => unknown {
  const js = transform(src, { transforms: ['typescript', 'imports'] }).code;
  const preamble = `const { ${KEYS.join(', ')} } = __dry;\n`;
  const moduleScope = `const exports = {};\nconst require = (specifier) => {\n` +
    `  if (specifier !== '@dry/sdk') throw new Error('unsupported live-docs import: ' + specifier);\n` +
    `  return __dry;\n};\n`;
  // Capture the snippet's last top-level expression by assigning it; we wrap user code so a trailing
  // expression statement becomes the return value without requiring an explicit `return`.
  const body = `${preamble}${moduleScope}return (function(){\n${wrapReturn(js)}\n})();`;
  const factory = new Function('__dry', `'use strict';\n${body}`) as (dry: Dry) => unknown;
  return factory;
}

// Re-emit the transpiled body so the final expression statement is returned. Sucrase handles module
// syntax without rewriting import-like text inside strings/comments; the injected require above resolves
// only @dry/sdk. The final implementation uses a top-level scanner for semicolonless statements.
function wrapReturn(js: string): string {
  const cleaned = js
    .split('\n')
    .filter((l) => !/^\s*"use strict";\s*$/.test(l))
    .join('\n')
    .trim();
  // If the author already returns, keep as-is; else turn the last statement into a return.
  if (/\breturn\b/.test(cleaned)) return cleaned;
  const body = cleaned.replace(/;\s*$/, '');
  const semi = body.lastIndexOf(';');
  if (semi === -1) return `return (${body});`;
  const head = body.slice(0, semi + 1);
  const tail = body.slice(semi + 1).trim();
  return tail ? `${head}\nreturn (${tail});` : `${head}\nreturn undefined;`;
}

export function runSnippet(src: string, dry: Dry):
  | { ok: true; value: unknown }
  | { ok: false; error: string } {
  try {
    return { ok: true, value: compileSnippet(src)(dry) };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
```

- [ ] **Step 4: Run the tests — expect PASS**

Run: `cd docs/site && npx vitest run .vitepress/theme/run-snippet.test.ts`
Expected: all three PASS. If the `wrapReturn` heuristic misbehaves on the multi-statement case, adjust so the **last** expression statement (one ending the source, no trailing `;` required) is the returned value; the tests pin the contract.

- [ ] **Step 5: Commit**

```bash
git add docs/site/.vitepress/theme/run-snippet.ts docs/site/.vitepress/theme/run-snippet.test.ts
git commit -m "feat(docs-site): compileSnippet — eval editable TS against the injected dry"
```

---

### Task 5: `render-ir.ts` — draw the resolved IR on a canvas

**Files:**
- Create: `docs/site/.vitepress/theme/render-ir.ts`
- Test: `docs/site/.vitepress/theme/render-ir.test.ts`

**Interfaces:**
- Consumes: `type Toolpath`, `type Segment` from `@sdk/ops`; `splinePoints` from the shared `web/spline.js` (aliased).
- Produces: `computeViewBox(segs: Segment[], w: number, h: number, pad?: number): ViewBox`, `drawIr(ctx: CanvasRenderingContext2D, ir: Toolpath, w: number, h: number): void`, `type ViewBox = { scale: number; ox: number; oy: number }`.

- [ ] **Step 1: Add a Vite alias for the shared spline sampler** — append to the `resolve.alias` map in `docs/site/.vitepress/config.ts` AND `docs/site/vitest.config.ts`:

```ts
// in config.ts (alongside '@sdk'):
'@webspline': path.resolve(repoRoot, 'web/spline.js'),
```
```ts
// in vitest.config.ts (alongside '@sdk'):
'@webspline': path.resolve(repoRoot, 'web/spline.js'),
```

- [ ] **Step 2: Write the failing test** — `docs/site/.vitepress/theme/render-ir.test.ts`

```ts
import { test, expect } from 'vitest';
import { computeViewBox, drawIr } from './render-ir';
import type { Segment, Toolpath } from '@sdk/ops';

const seg = (sx: number, sy: number, ex: number, ey: number, travel = false): Segment =>
  ({ start: [sx, sy, 0], end: [ex, ey, 0], travel, kind: 'line', speed: 0, length: 0, volume: 0,
     filament: 0, width: 0.4, height: 0.2, centre: null, clockwise: false }) as Segment;

test('computeViewBox fits all segment endpoints into the canvas with padding', () => {
  const vb = computeViewBox([seg(0, 0, 100, 50)], 200, 200, 10);
  // x spans 0..100 (the wider axis); scale fits 100 into 180 px => 1.8
  expect(vb.scale).toBeCloseTo(1.8, 5);
});

test('drawIr issues stroke calls for each segment without throwing on a minimal ctx', () => {
  const calls: string[] = [];
  const ctx = new Proxy({}, {
    get: (_t, p) => (typeof p === 'string' && p.endsWith('Style')) ? '' :
      (...a: unknown[]) => { calls.push(String(p)); return undefined; },
    set: () => true,
  }) as unknown as CanvasRenderingContext2D;
  const ir: Toolpath = { version: 1, segments: [seg(0, 0, 10, 0), seg(10, 0, 10, 10, true)] };
  drawIr(ctx, ir, 100, 100);
  expect(calls.filter((c) => c === 'stroke').length).toBeGreaterThanOrEqual(2);
});
```

- [ ] **Step 3: Run it — expect FAIL**

Run: `cd docs/site && npx vitest run .vitepress/theme/render-ir.test.ts`
Expected: FAIL — `./render-ir` does not exist.

- [ ] **Step 4: Create `docs/site/.vitepress/theme/render-ir.ts`**

```ts
// Lean 2D renderer for the resolved Dry IR. Extrude moves draw solid, travels draw faint/dashed
// (mirroring web/viewer.js conventions). Splines are sampled with the shared web/spline.js sampler so
// the docs and the gallery render identical curves. Self-contained — no coupling to viewer.js.
import type { Segment, Toolpath } from '@sdk/ops';
// @ts-expect-error - plain-JS shared module, no types
import { splinePoints } from '@webspline';

export interface ViewBox { scale: number; ox: number; oy: number }

function points(seg: Segment): [number, number][] {
  if (seg.kind === 'spline') {
    const pts = (splinePoints(seg) as number[][] | null) ?? [seg.start as number[], seg.end as number[]];
    return pts.map((p) => [p[0] ?? 0, p[1] ?? 0]);
  }
  return [
    [(seg.start[0] ?? 0), (seg.start[1] ?? 0)],
    [(seg.end[0] ?? 0), (seg.end[1] ?? 0)],
  ];
}

export function computeViewBox(segs: Segment[], w: number, h: number, pad = 12): ViewBox {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const s of segs) for (const [x, y] of points(s)) {
    minX = Math.min(minX, x); maxX = Math.max(maxX, x);
    minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  }
  if (!Number.isFinite(minX)) { minX = 0; minY = 0; maxX = 1; maxY = 1; }
  const spanX = Math.max(1e-6, maxX - minX), spanY = Math.max(1e-6, maxY - minY);
  const scale = Math.min((w - 2 * pad) / spanX, (h - 2 * pad) / spanY);
  // Centre the drawing; canvas y grows downward so we flip y at draw time.
  const ox = pad - minX * scale + (w - 2 * pad - spanX * scale) / 2;
  const oy = pad - minY * scale + (h - 2 * pad - spanY * scale) / 2;
  return { scale, ox, oy };
}

export function drawIr(ctx: CanvasRenderingContext2D, ir: Toolpath, w: number, h: number): void {
  ctx.clearRect(0, 0, w, h);
  const vb = computeViewBox(ir.segments, w, h);
  const tx = (x: number) => vb.ox + x * vb.scale;
  const ty = (y: number) => h - (vb.oy + y * vb.scale); // flip y
  for (const seg of ir.segments) {
    const pts = points(seg);
    if (pts.length < 2) continue;
    ctx.beginPath();
    ctx.moveTo(tx(pts[0][0]), ty(pts[0][1]));
    for (let i = 1; i < pts.length; i++) ctx.lineTo(tx(pts[i][0]), ty(pts[i][1]));
    if (seg.travel) {
      ctx.strokeStyle = 'rgba(120,140,170,0.45)';
      ctx.setLineDash([4, 4]);
      ctx.lineWidth = 1;
    } else {
      ctx.strokeStyle = '#3aa0ff';
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
    ctx.stroke();
  }
  ctx.setLineDash([]);
}
```

- [ ] **Step 5: Run the tests — expect PASS**

Run: `cd docs/site && npx vitest run .vitepress/theme/render-ir.test.ts`
Expected: both PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/site/.vitepress/theme/render-ir.ts docs/site/.vitepress/theme/render-ir.test.ts docs/site/.vitepress/config.ts docs/site/vitest.config.ts
git commit -m "feat(docs-site): lean IR canvas renderer (reuses the shared spline sampler)"
```

---

### Task 6: `<LiveExample>` component + theme registration

**Files:**
- Create: `docs/site/.vitepress/theme/LiveExample.vue`
- Modify: `docs/site/.vitepress/theme/index.ts`
- Test: `docs/site/.vitepress/theme/LiveExample.test.ts`

**Interfaces:**
- Consumes: `getDry`/`initDryEngine` (`./dry-engine`), `runSnippet` (`./run-snippet`), `drawIr` (`./render-ir`), CodeMirror 6, the `examples/*.ts` raw map via `import.meta.glob`.
- Produces: a global `<LiveExample src="<example-name>" />` component (slot text also accepted as inline source).

- [ ] **Step 1: Write the failing test** — `docs/site/.vitepress/theme/LiveExample.test.ts`

```ts
import { test, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { setWasmBinding, type DryWasm } from '@sdk/engine';
import LiveExample from './LiveExample.vue';

// The canonical component detects Vitest mode and executes synchronously without loading browser wasm.
// Lifecycle tests mock the Worker client and hold a deferred result to verify post-unmount guards.

setWasmBinding({
  resolve_ir: () => JSON.stringify({ version: 1, segments: [
    { start: [0,0,0], end: [10,0,0], travel: false, kind: 'line', width: 0.4, height: 0.2, centre: null, clockwise: false }] }),
  resolve_gcode: () => ['G1 X10 Y0 E0.5'],
  resolve_metrics: () => JSON.stringify({ total_time_s: 1 }),
} as unknown as DryWasm);

// jsdom canvas has no 2D context; stub it so drawIr is a no-op in the unit test.
HTMLCanvasElement.prototype.getContext = vi.fn(() => null) as never;

test('renders the seeded code and the g-code output, no error banner', async () => {
  const code = `import { Design } from '@dry/sdk';\nnew Design().geometry(0.6,0.2).extruder(true).point(0,0,0.2).point(10,0,0.2)`;
  const w = mount(LiveExample, { props: { code, outputs: ['gcode'] } });
  await new Promise((r) => setTimeout(r, 50)); // let the debounced run + onMounted init settle
  expect(w.text()).toContain('G1 X10 Y0');
  expect(w.find('.live-error').exists()).toBe(false);
});

test('a broken edit shows an inline error banner instead of throwing', async () => {
  const w = mount(LiveExample, { props: { code: `throw new Error('nope')`, outputs: ['gcode'] } });
  await new Promise((r) => setTimeout(r, 50));
  expect(w.find('.live-error').text()).toMatch(/nope/);
});
```

- [ ] **Step 2: Run it — expect FAIL**

Run: `cd docs/site && npx vitest run .vitepress/theme/LiveExample.test.ts`
Expected: FAIL — `./LiveExample.vue` does not exist.

- [ ] **Step 3: Create `docs/site/.vitepress/theme/LiveExample.vue`**

```vue
<script setup lang="ts">
import { ref, shallowRef, onMounted, onBeforeUnmount, watch, computed } from 'vue';
import { EditorView, basicSetup } from 'codemirror';
import { javascript } from '@codemirror/lang-javascript';
import { EditorState } from '@codemirror/state';
import { getDry, initDryEngine } from './dry-engine';
import { runSnippet } from './run-snippet';
import { drawIr } from './render-ir';
import type { Toolpath } from '@sdk/ops';

const EXAMPLES = import.meta.glob('../../examples/*.ts', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const props = withDefaults(defineProps<{ src?: string; code?: string; outputs?: string[] }>(), {
  outputs: () => ['gcode', 'ir', 'metrics'],
});

function seed(): string {
  if (props.code) return props.code.trim();
  const hit = Object.entries(EXAMPLES).find(([k]) => k.endsWith(`/${props.src}.ts`));
  return (hit?.[1] ?? `// example '${props.src}' not found`).trim();
}

const source = ref(seed());
const error = ref('');
const tab = ref(props.outputs[0]);
const gcode = ref<string[]>([]);
const irText = ref('');
const metricsText = ref('');
const verifyText = ref('');
const canvas = ref<HTMLCanvasElement | null>(null);
const editorHost = ref<HTMLElement | null>(null);
const ready = ref(false);
const view = shallowRef<EditorView | null>(null);

let timer: ReturnType<typeof setTimeout> | undefined;
let unmounted = false;
function schedule() {
  if (unmounted) return;
  clearTimeout(timer);
  timer = setTimeout(runNow, 250);
}

function clearResultState() {
  gcode.value = [];
  irText.value = '';
  metricsText.value = '';
  verifyText.value = '';
  const ctx = canvas.value?.getContext('2d');
  if (ctx && canvas.value) ctx.clearRect(0, 0, canvas.value.width, canvas.value.height);
}

function runNow() {
  if (!ready.value || unmounted) return;
  clearResultState();
  const r = runSnippet(source.value, getDry());
  if (!r.ok) { error.value = r.error; return; }
  error.value = '';
  try {
    const d = r.value as { ir?: () => Toolpath; gcode?: () => string[]; simulate?: () => unknown; verify?: (...a: unknown[]) => unknown; segments?: unknown };
    const ir: Toolpath | undefined = typeof d?.ir === 'function' ? d.ir() : (d?.segments ? (d as unknown as Toolpath) : undefined);
    if (ir && canvas.value) { const ctx = canvas.value.getContext('2d'); if (ctx) drawIr(ctx, ir, canvas.value.width, canvas.value.height); }
    irText.value = ir ? JSON.stringify(ir, null, 2) : '';
    gcode.value = typeof d?.gcode === 'function' ? d.gcode() : [];
    metricsText.value = typeof d?.simulate === 'function' ? JSON.stringify(d.simulate(), null, 2) : '';
    verifyText.value = typeof d?.verify === 'function' ? JSON.stringify(d.verify('generic', 0, 0, [[0,250],[0,210],[0,220]]), null, 2) : '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

const tabs = computed(() => props.outputs);

onMounted(async () => {
  view.value = new EditorView({
    parent: editorHost.value!,
    state: EditorState.create({
      doc: source.value,
      extensions: [basicSetup, javascript({ typescript: true }), EditorView.updateListener.of((u) => {
        if (u.docChanged) { source.value = u.state.doc.toString(); schedule(); }
      })],
    }),
  });
  try { await initDryEngine(); if (unmounted) return; ready.value = true; runNow(); }
  catch (e) { error.value = `couldn't load the Dry engine (wasm): ${e instanceof Error ? e.message : String(e)}`; }
});

onBeforeUnmount(() => {
  unmounted = true;
  clearTimeout(timer);
  view.value?.destroy();
  view.value = null;
});

watch(source, schedule);
function reset() { const s = seed(); source.value = s; view.value?.dispatch({ changes: { from: 0, to: view.value.state.doc.length, insert: s } }); schedule(); }
</script>

<template>
  <ClientOnly>
    <div class="live">
      <div class="live-code">
        <div class="live-bar"><span>TypeScript</span><button @click="reset">Reset</button></div>
        <div ref="editorHost" class="live-editor"></div>
      </div>
      <div class="live-demo">
        <canvas ref="canvas" width="360" height="240"></canvas>
        <div class="live-tabs"><button v-for="t in tabs" :key="t" :class="{ on: tab === t }" @click="tab = t">{{ t }}</button></div>
        <pre v-if="tab === 'gcode'" class="live-out">{{ gcode.join('\n') }}</pre>
        <pre v-else-if="tab === 'ir'" class="live-out">{{ irText }}</pre>
        <pre v-else-if="tab === 'metrics'" class="live-out">{{ metricsText }}</pre>
        <pre v-else-if="tab === 'verify'" class="live-out">{{ verifyText }}</pre>
        <div v-if="error" class="live-error">⚠ {{ error }}</div>
        <div v-else-if="!ready" class="live-loading">loading engine…</div>
      </div>
    </div>
  </ClientOnly>
</template>

<style scoped>
.live { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; border: 1px solid var(--vp-c-divider); border-radius: 8px; padding: 8px; margin: 16px 0; }
@media (max-width: 720px) { .live { grid-template-columns: 1fr; } }
.live-bar { display: flex; justify-content: space-between; font-size: 12px; opacity: .7; padding: 2px 4px; }
.live-editor { max-height: 320px; overflow: auto; }
.live-demo canvas { width: 100%; background: #0b0f17; border-radius: 6px; }
.live-tabs { display: flex; gap: 4px; margin: 6px 0; }
.live-tabs button { font-size: 12px; padding: 2px 8px; border-radius: 4px; }
.live-tabs button.on { background: var(--vp-c-brand-1); color: #fff; }
.live-out { max-height: 220px; overflow: auto; font-size: 12px; }
.live-error { color: #ff6b6b; background: #2a1a1a; border: 1px solid #ff4444; border-radius: 6px; padding: 8px; font-size: 13px; }
.live-loading { opacity: .6; font-size: 13px; padding: 8px; }
</style>
```

- [ ] **Step 4: Register the component** — replace `docs/site/.vitepress/theme/index.ts` body:

```ts
import DefaultTheme from 'vitepress/theme';
import type { Theme } from 'vitepress';
import LiveExample from './LiveExample.vue';

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('LiveExample', LiveExample);
  },
} satisfies Theme;
```

- [ ] **Step 5: Run the component tests — expect PASS**

Run: `cd docs/site && npx vitest run .vitepress/theme/LiveExample.test.ts`
Expected: both PASS (seeded g-code shown; broken edit shows `.live-error`). If CodeMirror's DOM init errors under jsdom, guard the `onMounted` editor construction behind a `typeof window !== 'undefined'` check and assert against `source`/output refs — the engine run path is what these tests pin.

- [ ] **Step 6: Commit**

```bash
git add docs/site/.vitepress/theme/LiveExample.vue docs/site/.vitepress/theme/index.ts docs/site/.vitepress/theme/LiveExample.test.ts
git commit -m "feat(docs-site): LiveExample component — editable TS, realtime canvas+g-code+IR+metrics+verify"
```

---

### Task 7: Guide content — landing hero + 6 tour pages + example files

**Files:**
- Create: `docs/site/examples/{author,lower,simulate,verify,optimize,generative}.ts`
- Create: `docs/site/guide/{index,author,lower,simulate,verify,optimize,generative}.md`
- Modify: `docs/site/index.md` (add a hero `<LiveExample>` below the home block)

**Interfaces:**
- Consumes: `<LiveExample>` (Task 6); each `.ts` example uses only the `@dry/sdk` surface in the injected `dry`.

- [ ] **Step 1: Create the six example snippets**

`docs/site/examples/author.ts`:
```ts
import { Design } from '@dry/sdk';
// A line, a G3 arc, then a line — the engine resolves extrusion for you.
new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(10, 0, 0.2)
  .arc({ cx: 0, cy: 0, x: 0, y: 10 })
  .point(0, 20, 0.2);
```

`docs/site/examples/lower.ts`:
```ts
import { Design } from '@dry/sdk';
// Lower the L1 design to the typed L2 Dry IR ({ version, segments }).
new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2).point(20, 0, 0.2).point(20, 20, 0.2).point(0, 20, 0.2).point(0, 0, 0.2)
  .ir();
```

`docs/site/examples/simulate.ts`:
```ts
import { Design } from '@dry/sdk';
// Simulate to get time / distances / material / peak flow.
new Design()
  .geometry(0.6, 0.2).extruder(true).speed(1800)
  .point(0, 0, 0.2).point(50, 0, 0.2).point(50, 50, 0.2).point(0, 50, 0.2).point(0, 0, 0.2)
  .simulate();
```

`docs/site/examples/verify.ts`:
```ts
import { Design } from '@dry/sdk';
// Verify against machine-safety contracts. Shrink the bounds below and watch out-of-bounds fire.
new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2).point(300, 0, 0.2) // 300mm in X
  .verify('generic', 0, 0, [[0, 250], [0, 210], [0, 220]]);
```

`docs/site/examples/optimize.ts`:
```ts
import { Design } from '@dry/sdk';
// Compare the safe optimization vs the kinematics-aware balanced pass.
const d = new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2).arc({ cx: 25, cy: 0, x: 50, y: 0 }).point(50, 50, 0.2);
d.balancedIr('generic', { max_acceleration_mm_s2: 3000, max_junction_velocity_mm_s: 8 });
```

`docs/site/examples/generative.ts`:
```ts
import { tpms } from '@dry/sdk';
// A gyroid TPMS infill block — generated by the engine, returned as a Design.
tpms({ surface: 'gyroid', cellSize: 10, cellsX: 2, cellsY: 2, cellsZ: 1, layerHeight: 0.2 });
```

- [ ] **Step 2: Create the guide overview** — `docs/site/guide/index.md`

```md
# The guided tour

Dry is a toolpath compiler: a design is a *program* that lowers through a typed IR
(design → path → motion → target) which the engine simulates, verifies, optimises and emits.

Every code block below is **live** — edit it and the canvas, g-code, IR, metrics and verify panes
re-run against the same Rust/wasm engine the CLI and the Python/TypeScript SDKs use.

1. [Author a path](./author) — the fluent `Design` API
2. [Lower to the Dry IR](./lower) — the typed L2 motion segments
3. [Simulate](./simulate) — time, distance, material, peak flow
4. [Verify](./verify) — machine-safety contracts
5. [Optimize](./optimize) — safe vs kinematics-aware balanced
6. [Generative](./generative) — TPMS and lattice generators
```

- [ ] **Step 3: Create the six guide pages** (each embeds its example with the relevant output tabs)

`docs/site/guide/author.md`:
```md
# 1. Author a path

The `Design` API is a chain of L1 ops; the engine resolves extrusion, feedrates and units for you.
Move points, sweep a `G3` arc, drop a spline — then read the motion g-code on the right.

<LiveExample src="author" :outputs="['gcode', 'ir']" />

Try: change the arc centre, or add `.spline([[20,30,0.2],[0,40,0.2]])`.
```

`docs/site/guide/lower.md`:
```md
# 2. Lower to the Dry IR

`.ir()` lowers the design to the typed L2 Dry IR — an array of motion `segments` with endpoints,
kind (line/arc/spline), width/height and process channels. This is the product the targets emit from.

<LiveExample src="lower" :outputs="['ir', 'gcode']" />
```

`docs/site/guide/simulate.md`:
```md
# 3. Simulate

`.simulate()` runs the motion model and returns metrics: total/print/travel time, extruding and
travel distance, extruded volume, filament length, and the peak volumetric flow rate.

<LiveExample src="simulate" :outputs="['metrics', 'gcode']" />

Try: raise `.speed(...)` and watch `total_time_s` and `max_flow_rate` move.
```

`docs/site/guide/verify.md`:
```md
# 4. Verify

`.verify(printer, maxFlow, minTemp, bounds, …)` checks the resolved toolpath against machine-safety
contracts and returns findings. The example prints a point outside the build volume — shrink or grow
the bounds and watch the out-of-bounds finding appear and clear.

<LiveExample src="verify" :outputs="['verify', 'gcode']" />
```

`docs/site/guide/optimize.md`:
```md
# 5. Optimize

`.optimizedIr()` runs the standard L2 optimization; `.balancedIr(printer, kinematics)` adds
kinematics-aware arc-speed clamping and junction-velocity capping. Compare the IR each produces.

<LiveExample src="optimize" :outputs="['ir', 'metrics']" />

Try: drop `max_acceleration_mm_s2` and see the balanced IR change.
```

`docs/site/guide/generative.md`:
```md
# 6. Generative

The TPMS and star-polygon lattice generators emit op-lists in the engine and hand you a `Design`.
Here's a gyroid infill block; swap `surface` for `schwarz-p`, `iwp`, `neovius`, `frd`, …

<LiveExample src="generative" :outputs="['gcode', 'ir']" />
```

- [ ] **Step 4: Add a hero LiveExample** — append to `docs/site/index.md` (after the `---` frontmatter block):

```md
<LiveExample src="author" :outputs="['gcode', 'ir']" />
```

- [ ] **Step 5: Type-check the examples and build the site — expect success**

Run: `cd docs/site && npm run typecheck && bash build.sh wasm-only && npm run build`
Expected: `tsc` clean (examples valid against the SDK source); `vitepress build` completes with the guide pages and the hero rendered (SSR renders the inert shell — engine runs client-side).

- [ ] **Step 6: Commit**

```bash
git add docs/site/examples docs/site/guide docs/site/index.md
git commit -m "feat(docs-site): the 6-page guided tour with live examples"
```

---

### Task 8: Playwright anti-drift smoke

**Files:**
- Create: `docs/site/playwright.config.ts`
- Create: `docs/site/smoke/examples.spec.ts`
- Create: `docs/site/smoke/oracle.ts`

**Interfaces:**
- Consumes: the built site (served by `vitepress preview`), the built `@dry/sdk` (Node) as the oracle, `compileSnippet` for the same transform.
- Produces: a CI-runnable smoke asserting each live example executed (no error banner) and its browser g-code equals the Node-engine g-code for the same source.

- [ ] **Step 1: Create the Node oracle** — `docs/site/smoke/oracle.ts`

```ts
// Run an example's source through the real @dry/sdk in Node — the byte-identity oracle the browser
// output must match. Reuses compileSnippet (same sucrase transform the browser uses) over the built
// Node SDK surface, so "browser == node" is a true engine-parity check.
import * as sdk from '@dry/sdk';
import { compileSnippet } from '../.vitepress/theme/run-snippet';
import type { Dry } from '../.vitepress/theme/dry-engine';

const dry = sdk as unknown as Dry;

export function oracleGcode(src: string): string[] {
  const v = compileSnippet(src)(dry) as { gcode?: () => string[] };
  return typeof v?.gcode === 'function' ? v.gcode() : [];
}
```

- [ ] **Step 2: Create `docs/site/playwright.config.ts`**

```ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './smoke',
  webServer: {
    command: 'npm run preview',
    url: 'http://localhost:4173/',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  use: { baseURL: 'http://localhost:4173' },
});
```

- [ ] **Step 3: Write the smoke** — `docs/site/smoke/examples.spec.ts`

```ts
import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { oracleGcode } from './oracle';

const here = path.dirname(fileURLToPath(import.meta.url));
const examplesDir = path.resolve(here, '../examples');

// (example file, guide page that embeds it with a gcode tab)
const PAGES: Array<{ name: string; url: string }> = [
  { name: 'author', url: '/guide/author' },
  { name: 'lower', url: '/guide/lower' },
  { name: 'simulate', url: '/guide/simulate' },
  { name: 'verify', url: '/guide/verify' },
  { name: 'optimize', url: '/guide/optimize' },
  { name: 'generative', url: '/guide/generative' },
];

for (const { name, url } of PAGES) {
  test(`live example '${name}' executes and matches the engine`, async ({ page }) => {
    await page.goto(url);
    const root = page.locator('.live').first();
    await expect(root).toBeVisible();
    // it ran: no error banner, engine finished loading
    await expect(root.locator('.live-loading')).toHaveCount(0, { timeout: 30_000 });
    await expect(root.locator('.live-error')).toHaveCount(0);

    // compare the browser g-code to the Node engine for the same source
    const src = fs.readFileSync(path.join(examplesDir, `${name}.ts`), 'utf8');
    const expected = oracleGcode(src);
    if (expected.length === 0) return; // generative/ir-only examples: existence is enough
    await root.getByRole('button', { name: 'gcode' }).click();
    const shown = (await root.locator('.live-out').first().innerText()).trim().split('\n');
    expect(shown).toEqual(expected);
  });
}
```

- [ ] **Step 4: Run the smoke — expect PASS**

Run:
```bash
cd docs/site && (cd ../../sdk/ts && bash build.sh) && npm install @dry/sdk@file:../../sdk/ts \
  && bash build.sh wasm-only && npm run build && npx playwright install --with-deps chromium && npm run smoke
```
Expected: all six example specs PASS — each `<LiveExample>` rendered without an error banner and its browser g-code equals the Node engine's. (For `optimize`/`generative`, which surface IR rather than g-code, existence-without-error is the assertion.)

> If `@dry/sdk@file:` linking is awkward in CI, point `oracle.ts`'s import at `../../sdk/ts/dist/src/index.js` via a tsconfig path instead; the requirement is only that the oracle runs the *built* Node SDK.

- [ ] **Step 5: Commit**

```bash
git add docs/site/playwright.config.ts docs/site/smoke docs/site/package.json
git commit -m "test(docs-site): Playwright anti-drift smoke — browser output must match the engine"
```

---

### Task 9: CI job

**Files:**
- Modify: `.github/workflows/ci.yml` (add a `docs-site` job)

**Interfaces:**
- Consumes: the self-hosted runner (see the repo's CI notes); the pinned wasm-bindgen toolchain already used by the web/TS jobs.

- [ ] **Step 1: Inspect the existing web/TS jobs** to mirror their runner label, Rust/wasm toolchain setup, and Node version.

Run: `sed -n '1,80p' .github/workflows/ci.yml` (read the web-smoke and ts-sdk jobs; reuse their `runs-on`, the wasm-bindgen `=0.2.123` install, and `node-version`).

- [ ] **Step 2: Add the `docs-site` job** — append under `jobs:` in `.github/workflows/ci.yml`, matching the existing jobs' runner + toolchain blocks (copy the wasm-bindgen install step verbatim from the web-smoke job):

```yaml
  docs-site:
    runs-on: [self-hosted, gluon-runner]   # match the label the other jobs use
    steps:
      - uses: actions/checkout@v4
      - name: Set up Rust wasm target
        run: rustup target add wasm32-unknown-unknown
      - name: Install wasm-bindgen-cli (pinned)
        run: cargo install wasm-bindgen-cli --version 0.2.123 --locked
      - name: Build the Node SDK (oracle)
        run: cd sdk/ts && bash build.sh
      - name: Install docs-site deps
        run: cd docs/site && npm install && npm install @dry/sdk@file:../../sdk/ts
      - name: Type-check examples
        run: cd docs/site && npm run typecheck
      - name: Unit tests
        run: cd docs/site && bash build.sh wasm-only && npm run test
      - name: Build site
        run: cd docs/site && npm run build
      - name: Smoke (anti-drift)
        run: cd docs/site && npx playwright install --with-deps chromium && npm run smoke
```

- [ ] **Step 3: Validate the workflow locally** (syntax) and confirm job wiring.

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml ok')"`
Expected: `yaml ok`. (Full CI runs on push to the self-hosted runner; adjust the `runs-on` label and toolchain steps to exactly match the sibling jobs you read in Step 1.)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: build, type-check, unit-test and anti-drift smoke the docs site"
```

---

## Self-review

**Spec coverage:**
- VitePress site under `docs/site/`, Guide+Reference sidebar split → Task 2 (Reference sidebar entries deferred with Phase 2 content; Guide complete).
- Editable TS, realtime re-run → Tasks 4 + 6.
- Reuse the real `@dry/sdk` via the engine split (Approach A) → Task 1.
- Browser consumes agnostic modules only (no `node:` in bundle) → Tasks 1 (engine.web has no `node:`/`import.meta`) + 3 (imports `@sdk/design` etc., never `@dry/sdk`/`index.ts`).
- `<LiveExample>` (CodeMirror + sucrase + debounce + code-left/demo-right + g-code·IR·metrics·verify tabs + inline error + client-only) → Task 6.
- Lean IR renderer reusing `web/spline.js` → Task 5.
- Guide = 6-step progression + landing → Task 7.
- Anti-drift smoke (browser output matches engine) → Task 8.
- CI job mirroring the existing jobs on the self-hosted runner → Task 9.
- SDK regression (Node tests + `web/smoke.cjs` green) → Task 1 Steps 8-9.
- **Phase 2 (exhaustive Reference + coverage-audit test) is intentionally out of scope** for this plan — it is a separate plan per the spec's phasing.

**Placeholder scan:** no TBD/TODO; every code step shows real code; commands have expected output. The two "if it misbehaves, adjust" notes (Task 4 `wrapReturn`, Task 6 jsdom CodeMirror) are fallbacks around tested contracts, not missing content.

**Type consistency:** `DryWasm`/`setWasmBinding` (Task 1) consumed by Tasks 3/6 tests; `Dry`/`getDry`/`initDryEngine` (Task 3) consumed by Tasks 4/6/8; `runSnippet`/`compileSnippet` (Task 4) consumed by Tasks 6/8; `drawIr`/`computeViewBox` (Task 5) consumed by Task 6; `<LiveExample src outputs>` props (Task 6) consumed by Task 7; example filenames (`author/lower/simulate/verify/optimize/generative`) consistent across Tasks 7/8. The `@sdk` and `@webspline` aliases are declared in both `config.ts` and `vitest.config.ts`.
