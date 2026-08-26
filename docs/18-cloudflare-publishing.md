# Public Cloudflare sites

The repository publishes two independent Cloudflare Pages projects. `dry-public-docs` serves the
documentation and interactive browser product built from `docs/site`. `drymachina` serves the portal
and Studio at `drymachina.com`, built from the repository root. They share no build tooling, and a
deployment of one has no effect on the other.

## The `dry-public-docs` documentation site

Dry's documentation and interactive browser product in `docs/site` are public at the
`dry-public-docs` Cloudflare Pages project. Direct Upload receives only the generated files in
`docs/site/.vitepress/dist`; no Cloudflare credential is committed to the repository.

The production artifact includes:

- the guide, generated API reference, market material, and licensing page;
- executable guide examples backed by the TypeScript SDK;
- the full `/gallery/` explorer, Three.js renderer, playback, verification, optimization, and export;
- `dry_wasm.js` and `dry_wasm_bg.wasm`, built from the Rust engine;
- the committed FullControl clean-room reconstructions, lattice generator, and TPMS generator.

Source and downloadable release artifacts are public through GitHub as described in
[`12-releasing.md`](12-releasing.md). They remain proprietary under `LICENSE`; public access does not
grant permission to use, modify, or redistribute them.

### Build modes

`npm run build` creates the production artifact and is the build deployed to Cloudflare.

`npm run build:docs-only` retains the smaller documentation-only boundary. It omits `/pkg` and
`/gallery`, substitutes static examples, and runs the source-provenance and artifact allow-list audit.
Use it only when a lightweight docs-only deployment is specifically required.

`npm run build:product` is an explicit alias for the full production build and remains useful in local
development and CI.

### Build and deploy

Prerequisites:

- Wrangler is authenticated to the Cloudflare account that owns `dry-public-docs`.
- The full generated output has passed tests and browser smoke checks.

From `docs/site`:

```sh
npm ci
npm run typecheck
npm run test
npm run deploy:cloudflare
```

`deploy:cloudflare` always rebuilds the full public product artifact before using Direct Upload. The
`main` branch is the Pages production branch.

After deployment:

1. Confirm an unauthenticated request to `https://dry-public-docs.pages.dev/` returns `200`.
2. Load the guide, reference, marketing, and licensing pages.
3. Confirm `/gallery/`, `/gallery/pkg/dry_wasm.js`, and `/gallery/pkg/dry_wasm_bg.wasm` return `200`.
4. Confirm the gallery initializes its Three.js canvas, resolves an example through WASM, and renders
   generated G-code without browser errors.

List deployments without changing production:

```sh
npx --yes wrangler@4.111.0 pages deployment list --project-name dry-public-docs
```

## The `drymachina` portal and Studio site

`drymachina.com`, `www.drymachina.com`, and `drymachina.pages.dev` are served by the `drymachina`
Pages project, built from the repository root rather than from `docs/site`. The `main` branch is the
Pages production branch.

`scripts/build_site.sh` stages the static bundle into `dist-site/`: the root portal (`index.html`,
`README.md`), the `docs/` tree, the standalone `web/*.html` portals, `web/machines.json`, and the
compiled Vite Studio from `web/dist`. It also writes `_redirects` and `_headers`.

### `functions/` is not part of `dist-site`

The four public API endpoints — `/api/verify`, `/api/macros`, `/api/mcp`, `/api/machines` — are
Cloudflare Pages Functions in `functions/` at the repository root. `scripts/build_site.sh` never
copies them into `dist-site/`, and nothing in the staged bundle references them.

They reach production only because Wrangler discovers a `functions/` directory relative to the
**current working directory**, not relative to the directory being uploaded. The deploy must therefore
run from the repository root:

```sh
bash scripts/build_site.sh
npx wrangler pages deploy dist-site --project-name drymachina --branch main   # from the repo root
```

Running the same command from any other directory uploads the identical static bundle and silently
produces a site with no API endpoints. Wrangler reports success and the deploy looks normal. Confirm
`/api/machines` after every deploy rather than inferring it from the upload summary.

### Verification after deploy

1. Confirm `https://drymachina.com/` and `https://www.drymachina.com/` return `200`.
2. Confirm `/web/` loads the Studio and `/web/machines.json` returns the machine registry.
3. Confirm all four `/api/*` endpoints return `200` with `application/json` bodies.

`dist-site/` ships no `404.html`. Unmatched paths fall back to the root `index.html` with a `200`
status, so a status code alone does not prove a file was deployed — compare the response body.

### Deployment records and the working tree

A Pages deployment records the commit that was checked out, but Direct Upload sends the working tree
as it stands. The two can disagree: the deployment the dashboard attributes to `1530784` already
served `functions/`, which was committed ten minutes later as `9b43703`. The window was short here,
but nothing bounds it. Treat the recorded commit as a hint, not as a description of what is running.

### Current state: offline

As of 2026-08-25 the production deployment is a static maintenance page that returns
"Dry Machina — Temporarily Offline" on every path. The Pages project, its custom domains, and its
deployment history are intact; only the served content was replaced.

To restore the site, rebuild and redeploy from the repository root using the command above, then run
the verification steps. Rolling back to a stored deployment through the Cloudflare dashboard restores
the same content without a rebuild.

Every past deployment also remains reachable at its own `<hash>.drymachina.pages.dev` URL, including
the API endpoints. Replacing the production deployment does not withdraw those. Removing them requires
deleting the deployments or placing a Cloudflare Access policy over `*.drymachina.pages.dev`, which
closes the per-deployment URLs while leaving `drymachina.pages.dev` itself public.

`tools/check_pages_exposure.sh` enumerates the stored deployments of a Pages project and reports which
are still publicly reachable. It is read-only and applies to either project:

```sh
tools/check_pages_exposure.sh                              # drymachina, probing /api/mcp
tools/check_pages_exposure.sh dry-public-docs --path /gallery/
```

It exits non-zero while any deployment still answers, so it can gate a takedown being called done.

## Automation boundary

The repository contains no Cloudflare API token and grants Cloudflare no GitHub access. Automated
deployment may be added later with a narrowly scoped Pages token stored as a GitHub Actions secret. The
workflow must run the full CI suite and `npm run build` before upload.
