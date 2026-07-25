# Public Cloudflare documentation

Dry's product documentation in `docs/site` is public at the `dry-docs` Cloudflare Pages project.
Cloudflare receives only the generated files in `docs/site/.vitepress/dist`; it is not connected to the
private GitHub repository.

This is separate from product distribution. CLI, Python, TypeScript, Rust, and WebAssembly artifacts
remain available only through authenticated private releases as described in
[`12-releasing.md`](12-releasing.md).

## Distribution boundary

`npm run build` is the only build approved for public deployment. It:

- renders the guide, API reference, product positioning, and static SVG previews;
- replaces every executable `LiveExample` with a non-executable licensed-product notice;
- disables Vite's normal `public/` copy and stages only an explicit documentation asset allow-list;
- omits `/pkg`, `/gallery`, SDK implementation modules, wasm binaries, and package archives;
- runs an automated boundary check before reporting success.

`npm run build:product` creates the internal interactive site and includes proprietary WebAssembly and
gallery code. It must never be uploaded to public hosting.

## Build and deploy

Prerequisites:

- Wrangler is authenticated to the Cloudflare account that owns `dry-docs`.
- The generated public output has passed review.

From `docs/site`:

```sh
npm ci
npm run typecheck
npm run test
npm run deploy:cloudflare
```

`deploy:cloudflare` always rebuilds the safe public artifact before using Direct Upload. The `main`
branch is the Pages production branch.

After deployment:

1. Confirm an unauthenticated request to `https://dry-docs.pages.dev/` returns `200`.
2. Load the guide, reference, and marketing pages.
3. Confirm `/pkg/dry_wasm.js`, `/pkg/dry_wasm_bg.wasm`, and `/gallery/` return `404`.
4. Confirm example panels state that interactive execution belongs to the authenticated product.

List deployments without changing production:

```sh
npx --yes wrangler@4.111.0 pages deployment list --project-name dry-docs
```

## Automation boundary

The repository contains no Cloudflare API token and grants Cloudflare no GitHub access. Automated
deployment may be added later with a narrowly scoped Pages token stored as a GitHub Actions secret, but
the workflow must run `npm run build` and the public-boundary check before upload.
