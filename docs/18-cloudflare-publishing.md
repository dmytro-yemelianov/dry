# Public Cloudflare product site

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

## Build modes

`npm run build` creates the production artifact and is the build deployed to Cloudflare.

`npm run build:docs-only` retains the smaller documentation-only boundary. It omits `/pkg` and
`/gallery`, substitutes static examples, and runs the source-provenance and artifact allow-list audit.
Use it only when a lightweight docs-only deployment is specifically required.

`npm run build:product` is an explicit alias for the full production build and remains useful in local
development and CI.

## Build and deploy

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

## Automation boundary

The repository contains no Cloudflare API token and grants Cloudflare no GitHub access. Automated
deployment may be added later with a narrowly scoped Pages token stored as a GitHub Actions secret. The
workflow must run the full CI suite and `npm run build` before upload.
