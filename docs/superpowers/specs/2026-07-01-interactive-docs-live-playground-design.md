# Interactive docs — live code + realtime execution (VitePress)

**Date:** 2026-07-01
**Status:** Approved design, ready for implementation
**Branch:** `feat/live-docs`

## Problem

Dry's browser surface today is a *showcase* (`web/index.html` gallery, `web/blocks.html` visual authoring,
static `architecture.html` / `opportunities.html`) — it demonstrates canned designs but never shows the
**authoring code beside its live result**. A newcomer can't read "this is the code you'd write" next to
"this is what it produces" and edit it in place. We want **interactive documentation**: each feature
presented as prose + an *editable* TypeScript snippet using the real `@dry/sdk` fluent API, executing
against the same wasm engine the CLI/Python/TS SDK use, re-rendering its canvas + g-code + metrics +
verify report **in realtime** as the reader types.

This must not become a second engine or a forked API — the repo's load-bearing claim is "one engine,
byte-identical across native / Python / wasm." The docs must edit and run the *genuine* SDK.

## Decisions (resolved during brainstorming)

1. **Delivery: a VitePress docs site** (Vite + Vue 3, markdown-driven), new app under `docs/site/`. Chosen
   over a bare new `web/*.html` page (wanted real prose/nav/search) and over Docusaurus (lighter; Vue
   matches a Vite/wasm project). Kept **out of `web/`** so the existing static showcase stays zero-build
   (`python3 -m http.server`).
2. **Realtime model: editable TypeScript, live re-run.** The reader edits a snippet using the fluent
   `Design` API; each keystroke (debounced) re-runs it against the wasm engine and updates the demo pane.
3. **Coverage: both a guided tour AND an exhaustive reference** (user: "both 1 and 2"). Implemented in two
   independently-shippable phases (Guide first, Reference second — see *Content & phasing*).
4. **Mechanism: reuse the real `@dry/sdk` in-browser (Approach A).** The `Design` API
   (`sdk/ts/src/design.ts`, `ops.ts`, `generators/`) is already platform-agnostic; its *only* Node coupling
   is the wasm loader in `engine.ts:56-57` (`createRequire` of the **nodejs**-target glue). The browser
   already has the **web**-target wasm at `web/pkg/`. So we split the wasm-loading out of `engine.ts` and
   reuse every other SDK file untouched. Rejected: Vite-aliasing `./engine` to a hand-maintained browser
   shim (B) and duplicating a browser `Design` (C) — both re-create the drift surface the 2026-06-30 webapp
   gap audit flagged (the `web/tpms.js` JS-fork that broke byte-identity for TPMS).

## The engine split (Approach A) — non-breaking, internal

`sdk/ts/src/engine.ts` becomes **binding-agnostic**: the typed resolve wrappers (`resolveGcode`,
`resolveMetrics`, `resolveMetricsIr`, `resolveIr`, `resolveBinary`, `resolveOptimizedIr`,
`resolveBalancedIr`, `resolveVerify`, `tpmsOps`) keep their exact signatures but call a lazily-obtained
binding instead of a module-load-time `requireWasm`:

```ts
// engine.ts — no node imports at module scope
let wasm: DryWasm | undefined;
export function setWasmBinding(b: DryWasm): void { wasm = b; }
function bind(): DryWasm {
  if (!wasm) throw new Error('Dry wasm not initialised: call the platform loader first');
  return wasm;
}
// each wrapper uses bind().resolve_*(…)
```

Two thin platform loaders:

- `engine.node.ts` — does today's `createRequire(__filename)` load of `../../wasm/dry_wasm.js` and calls
  `setWasmBinding`. **`index.ts` imports it for its side effect** so the published Node package keeps
  auto-initialising on import (zero behaviour change for existing `import { Design } from '@dry/sdk'`
  users). Node tests (`sdk/ts/test/*`) and the byte-identity smoke are unchanged.
- `engine.web.ts` — `async initDryWeb(wasmUrl?: string): Promise<void>` that loads the **web**-target glue
  (`web/pkg/dry_wasm.js`), runs its async `init()` (or `default(wasmUrl)`), and calls `setWasmBinding`.
  Only the docs site imports this. Contains **no `node:` imports**, so it is safe in a browser bundle.

`design.ts`, `ops.ts`, `generators/starPolygonLattice.ts`, `generators/tpms.ts`, `index.ts`'s re-exports:
**unchanged**. Public API surface: **unchanged**. This is the whole SDK change.

**Browser consumes the agnostic modules directly, never `index.ts`.** `index.ts` (the package main)
imports `engine.node` for its Node auto-init side effect, so it is node-only by construction. The docs site
therefore imports the platform-agnostic source files directly — `design.ts`, the generators, the agnostic
`engine.ts` wrappers, and `engine.web.ts` — and never the `@dry/sdk` package entry, keeping every `node:`
import out of the Vite browser graph. (No `@dry/sdk` Vite alias; `dry-engine.ts` uses relative imports into
`sdk/ts/src/`.)

> Note the two senses of "kinematics" the repo already disambiguates: the `rotaryAxes` *string* (ab/ac/bc)
> on `gcode()` vs the `MachineKinematics` *object* on `balancedIr()`/`verify()`. The docs reuse the SDK's
> own names verbatim, so the distinction carries through for free.

## Site structure & toolchain

```
docs/site/
  package.json            # vitepress + sucrase + @codemirror/* ; scripts: dev/build/preview/smoke
  .vitepress/
    config.ts             # title, nav, sidebar (Guide vs Reference), markdown config
    theme/
      index.ts            # extends DefaultTheme; enhanceApp registers <LiveExample>
      LiveExample.vue      # the editor + live-demo component
      dry-engine.ts        # singleton: await initDryWeb() once; exposes the injected `dry` object
      render-ir.ts         # lean canvas renderer (IR -> 2D), reuses web/spline.js conventions
      run-snippet.ts       # sucrase transpile + sandboxed eval -> Design/outputs
  public/pkg/             # web-target wasm copied here by build.sh (dry_wasm.js + .wasm)
  guide/*.md              # the tour (Phase 1)
  reference/*.md          # exhaustive (Phase 2)
  build.sh                # 1) bash web/build.sh web  2) copy web/pkg -> docs/site/public/pkg
                          #    3) npm --prefix docs/site run build
```

The agnostic SDK source is consumed directly via relative imports in `dry-engine.ts` (`design.ts`,
generators, the agnostic `engine.ts`, and `engine.web.ts`) so the docs always track the live SDK, not a
stale `dist/`, and no node-only module enters the browser bundle. No new wasm build target — `build.sh`
reuses the existing `web/build.sh web` output and the same triple-pinned wasm-bindgen `=0.2.123` toolchain.

## `<LiveExample>` component

Used in markdown two ways: a fenced ```` ```ts live ```` block (a markdown-it rule rewrites it to the
component) or explicit `<LiveExample>` with the code as a slot / `src` to a file under
`docs/site/examples/`. Examples live in real `.ts` files so they are type-checked and smoke-run.

- **Layout:** editable code **left**, live demo **right**; stacks vertically below ~720px. The demo pane
  has the canvas on top and a tab strip: **g-code · IR · metrics · verify** (tabs shown only for the
  outputs the snippet actually requests).
- **Editor:** CodeMirror 6 (`@codemirror/lang-javascript` in TS mode). Seeded with the example source;
  a "Reset" restores the original.
- **Run loop:** on change, debounce ~250ms → `run-snippet.ts`:
  1. `sucrase` transform (`transforms: ['typescript', 'imports']`) strips types and rewrites/removes
     `import`/`export` → plain JS, so a displayed snippet can stay idiomatic
     (`import { Design } from '@dry/sdk'`) yet eval cleanly.
  2. Prepend a destructure of the injected binding (`const { Design, tpms, starPolygonLattice, resolveVerify,
     PRINTERS, … } = dry;`) so the stripped imports resolve to the real SDK, then
     `new Function('dry', '"use strict";' + preamble + js)` invoked with the injected `dry` object. The
     snippet's last expression / an explicit `return` yields the `Design` (or a raw IR/metrics object) to
     render.
  3. Whatever methods the snippet's result exposes drive the panes (`.ir()` → canvas + IR tab,
     `.gcode()` → g-code tab, `.simulate()` → metrics, `.verify(…)` → verify tab). A small adapter renders
     a `Design`, or a plain `{segments}` IR, or a metrics/report object.
- **Realtime + safety:** runs only client-side. The component renders an inert code block during SSR /
  before the engine is ready, then hydrates and runs on mount (guard with VitePress `<ClientOnly>` and an
  `onMounted` engine `await`). Eval errors and engine `JsError`s are caught and shown **inline in the demo
  pane** (red banner with the message) — a broken edit never blanks the canvas or throws past the
  component. A per-run guard (try/catch + a soft op-count ceiling on generated examples) keeps a pathological
  edit from hanging the tab.

## Renderer (`render-ir.ts`)

A lean (~100-line) 2D canvas renderer over the resolved IR `Toolpath` (`{ version, segments }`): iterate
segments, draw extrude moves solid and travel moves dashed/faint (mirroring `web/viewer.js` colour
conventions), auto-fit bounds with padding, light axis hint. Reuses the shared `web/spline.js` sampler for
spline segments. Deliberately **not** coupled to the 1443-line `web/viewer.js` — self-contained and
testable, but visually consistent so the docs and the gallery read as one product.

## Content & phasing

**Phase 1 — Guide (the spine, fully live).** `guide/`:

1. *Author a path* — `geometry`/`extruder`/`point`/`arc`/`spline`; canvas + g-code live.
2. *Lower to the Dry IR* — `.ir()`; show the typed `{segments}` beside the canvas.
3. *Simulate* — `.simulate()`; time / distances / material / peak flow.
4. *Verify* — `.verify(printer, …, bounds, …)`; show findings; toggle a limit and watch a rule fire.
5. *Optimize* — `.optimizedIr()` vs `.balancedIr(printer, kinematics)`; safe vs balanced/max, before/after
   metrics (the `metrics_ir` path).
6. *Generative* — `tpms(...)` and `starPolygonLattice(...)` driving a `Design`.

Plus an `index.md` landing page (what Dry is, one hero `<LiveExample>`) and a `guide/index.md` overview.

**Phase 2 — Reference (exhaustive).** `reference/`: one focused live example per **L1 op** (move, arc,
spline, geometry, extruder, speed, temperature, fan, flow, tool, orient, dwell, manual_gcode, retract,
unretract, deposit), per **verify rule** (the catalog in `docs/11-profiles-and-reports.md`), and per
**engine pass / call** (`gcode`/`ir`/`binary`/`simulate`/`optimizedIr`/`balancedIr`/`verify`). Each Phase-2
page is the same `<LiveExample>` component — no new mechanism, only content. The rule catalog and op list
are enumerated against the engine so coverage is auditable.

Each phase is independently shippable: Phase 1 delivers a complete, working live-docs site; Phase 2 only
adds pages.

## Data flow (one `<LiveExample>` render)

```
reader edits CodeMirror  ──debounce──▶ run-snippet.ts
  sucrase(ts → js) ─▶ new Function('dry', js)(dryInjected)        // dryInjected from dry-engine singleton
     └─ executes real @dry/sdk Design chain ─▶ wasm (web/pkg) ─▶ result (Design | IR | metrics | report)
  result ─▶ adapter ─▶ { ir?, gcode?, metrics?, report? }
     ├─ render-ir.ts ─▶ canvas
     └─ tabs ◀─ g-code | IR JSON | metrics | verify
  any throw ─▶ inline red banner in demo pane (page stays alive)
```

The wasm is initialised exactly once (`dry-engine.ts` `await initDryWeb()` on first component mount; later
components reuse the singleton).

## Error handling

- **Engine not ready / failed to load wasm:** demo pane shows a "loading engine…" then, on failure, a
  clear "couldn't load the Dry engine (wasm)" message (reuse the spirit of `web/wasm-load.js`). Page prose
  still renders.
- **Snippet compile/eval error** (bad TS, ReferenceError): caught, message shown inline; last good render
  left in place if present, else an empty-state hint.
- **Engine `JsError`** (e.g. unknown printer, malformed ops): caught from the resolve call; message shown
  inline. These are *expected* in a teaching context (e.g. an intentionally-invalid edit) and must read as
  data, not a crash.
- **SSR:** component is client-only; `vitepress build` must succeed with zero engine calls at build time.

## Testing & CI

- **Build gate:** `docs/site` `tsc --noEmit` over the example `.ts` files (they're real TS) + `vitepress
  build` succeeds.
- **Live smoke (the anti-drift loop):** a Playwright test (`playwright` is already available in-session)
  loads the built site, and for every `<LiveExample>` asserts it executed without an inline error banner
  and that its rendered output matches the engine run directly in Node via `@dry/sdk` for the same source —
  so a docs example can never silently diverge from the engine. Enumerate examples from the `examples/`
  dir.
- **Coverage audit (Phase 2):** a test asserts the Reference has a page/example for every L1 op and every
  verify-rule id (read from the engine/catalog), failing if the engine grows an op/rule the docs don't
  show — turning "exhaustive" into a checked invariant, not a claim.
- **SDK regression:** the existing `sdk/ts` Node tests + the `web/smoke.cjs` byte-identity check must stay
  green after the `engine.ts` split (proves the refactor is behaviour-preserving).
- CI runs on the self-hosted runner (see [[dry-ci-self-hosted-runner]]); add a `docs-site` job mirroring
  the existing web/TS jobs.

## Scope / YAGNI (deferred)

Monaco (full IDE intellisense) — CodeMirror is enough; live Python execution (no Pyodide — TS is the
in-browser path, Python shown as static parity snippets only where useful); shareable permalink/state
encoding of edits; multi-file examples; 3D/5-axis viewport (the lean 2D renderer covers the toolpaths the
examples produce; link to `web/index.html` for the richer viewer); embedding `<LiveExample>` back into the
old `web/*.html` pages; search tuning beyond VitePress defaults; i18n.
