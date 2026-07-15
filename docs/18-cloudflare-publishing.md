# Private Cloudflare publishing

Dry's hosted surface is the WebAssembly-backed VitePress site in `docs/site`. It is deployed to the
`dry-docs` Cloudflare Pages project by Direct Upload. Cloudflare receives only the generated files in
`docs/site/.vitepress/dist`; it is not connected to the private GitHub repository.

This is separate from tagged product releases. CLI, Python and TypeScript artifacts remain available
only from private GitHub Releases as described in [`12-releasing.md`](12-releasing.md). npm and PyPI
publishing remain disabled.

## Security boundary

Cloudflare Pages hostnames are public by default. **Do not upload Dry assets unless Cloudflare Access is
enabled for both `dry-docs.pages.dev` and its preview deployments.** The Access policy is the primary
control; the site's `X-Robots-Tag` header is only defense in depth.

Use Direct Upload rather than Pages Git integration. This keeps repository access and build credentials
out of Cloudflare and sends only the generated static site and browser WebAssembly bundle.

## Build and deploy

Prerequisites:

- Wrangler is authenticated to the Cloudflare account that owns `dry-docs`.
- The Cloudflare dashboard shows an active Access policy for the production hostname and previews.
- An unauthenticated request to `https://dry-docs.pages.dev/` is rejected or redirected to Access.

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

1. Repeat the unauthenticated request check; it must still be rejected or redirected to Access.
2. Sign in through Access and load the guide, reference pages and a live example.
3. Confirm `/pkg/dry_wasm.js` and `/pkg/dry_wasm_bg.wasm` load only in the authenticated session.

List deployments without changing production:

```sh
npx --yes wrangler@4.111.0 pages deployment list --project-name dry-docs
```

## Automation boundary

The repository does not contain a Cloudflare API token and does not grant Cloudflare access to GitHub.
Automated deployments may be added later only with a narrowly scoped Pages token stored as a GitHub
Actions secret and a workflow that uploads the built output. Until then, deployment is an explicit local
release operation after the Access preflight check.
