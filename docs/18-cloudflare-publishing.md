# Public Cloudflare publishing

Dry's hosted surface is the WebAssembly-backed VitePress site in `docs/site`. It is deployed to the
`dry-docs` Cloudflare Pages project by Direct Upload. Cloudflare receives only the generated files in
`docs/site/.vitepress/dist`; it is not connected to the private GitHub repository.

This is separate from tagged product releases. CLI, Python and TypeScript artifacts remain available
only from private GitHub Releases as described in [`12-releasing.md`](12-releasing.md). npm and PyPI
publishing remain disabled.

## Distribution boundary

The hosted documentation and browser WebAssembly bundle are intentionally public at
`https://dry-docs.pages.dev`. They are not protected by Cloudflare Access. The site's `X-Robots-Tag`
header asks search engines not to index the deployment, but it is not an access control and must not be
treated as one.

Use Direct Upload rather than Pages Git integration. This keeps repository access and build credentials
out of Cloudflare and sends only the generated static site and browser WebAssembly bundle.

## Build and deploy

Prerequisites:

- Wrangler is authenticated to the Cloudflare account that owns `dry-docs`.
- The content in `docs/site/.vitepress/dist` is approved for public distribution.

From `docs/site`:

```sh
npm ci
npm run typecheck
npm run test
npm run deploy:cloudflare
```

`deploy:cloudflare` rebuilds the web-target WASM package, builds VitePress, and uploads
`.vitepress/dist` using the repository-pinned Wrangler version. The `main` branch is the Pages production
branch.

After deployment:

1. Confirm an unauthenticated request to `https://dry-docs.pages.dev/` returns `200`.
2. Load the guide, reference pages and a live example.
3. Confirm `/pkg/dry_wasm.js` and `/pkg/dry_wasm_bg.wasm` load publicly.

List deployments without changing production:

```sh
npx --yes wrangler@4.111.0 pages deployment list --project-name dry-docs
```

## Automation boundary

The repository does not contain a Cloudflare API token and does not grant Cloudflare access to GitHub.
Automated deployments may be added later only with a narrowly scoped Pages token stored as a GitHub
Actions secret and a workflow that uploads the built output. Until then, deployment is an explicit local
release operation after the public-content preflight check.
