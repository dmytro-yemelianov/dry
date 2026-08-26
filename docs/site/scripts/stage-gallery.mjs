#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(siteRoot, '../..');
const webRoot = path.join(repoRoot, 'web');
const galleryBuild = path.join(webRoot, 'dist-gallery');
const targetRoot = path.join(siteRoot, '.vitepress/dist/gallery');

// The gallery used to be a hand-written static page staged file-by-file from `web/`. Studio 2.0
// moved it to a Vite + React app, so the staged artifact is now a build output: `web/dist-gallery`,
// produced by `npm run build:gallery` in `web/` with `--base=/gallery/`. The `/web/` mount used by
// scripts/build_site.sh keeps its own `web/dist` build; the two differ only in base and outDir.
if (!fs.existsSync(galleryBuild)) {
  throw new Error(
    `Missing gallery build: web/dist-gallery\n` +
      "Run `npm ci && npm run build:gallery` in web/ before staging the product site.",
  );
}

fs.rmSync(targetRoot, { recursive: true, force: true });
fs.cpSync(galleryBuild, targetRoot, { recursive: true });

// Assets the app reaches for at runtime rather than importing, so the bundler never sees them and
// cannot emit them. `machines.json` is fetched relative to the page: the /web/ deploy gets it from
// scripts/build_site.sh, and the /gallery/ mount has to stage its own copy or the machine catalog
// silently falls back to empty. The engine glue is served at the documented unhashed path, not the
// fingerprinted copy Vite inlines into assets/.
const RUNTIME_ASSETS = ['pkg/dry_wasm.js', 'pkg/dry_wasm_bg.wasm', 'machines.json'];
for (const relative of RUNTIME_ASSETS) {
  const source = path.join(webRoot, relative);
  if (!fs.existsSync(source)) {
    throw new Error(
      `Missing gallery build input: web/${relative}\n` +
        'Run `bash docs/site/build.sh wasm-only` to build the engine, and check the file exists.',
    );
  }
  const target = path.join(targetRoot, relative);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(source, target);
}

// Staging was only ever checked in one direction: every input had to exist. The other half is what
// actually shipped broken (#190) — a page referencing a module nobody staged 404s at runtime, in
// the browser, with nothing failing here. So walk what was staged, collect the same-tree modules
// and assets it references, and require each one to have been staged too. This is what caught the
// Studio 2.0 entry point still pointing at unbuilt `./src/main.tsx`.
// `base` is where a relative specifier resolves from, and the two kinds genuinely differ:
// an import or a src/href resolves against the file that contains it, while a runtime fetch
// resolves against the *document* URL. A bundled `fetch('./machines.json')` sitting in
// gallery/assets/index-*.js therefore asks for /gallery/machines.json, not
// /gallery/assets/machines.json. Resolving it like an import reports a file that is staged
// correctly as missing.
const REFERENCE_PATTERNS = [
  { base: 'module', re: /\bfrom\s*['"]([^'"]+)['"]/g }, // import ... from './x.js'
  { base: 'module', re: /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g }, // dynamic import('./x.js')
  { base: 'module', re: /\bsrc\s*=\s*['"]([^'"]+)['"]/g }, // <script src="./x.js">
  { base: 'module', re: /\bhref\s*=\s*['"]([^'"]+\.css)['"]/g }, // <link href="./x.css">
  // Runtime fetches are invisible to the bundler, so they are exactly the references that reach
  // production unstaged — `fetch('./machines.json')` 404'd the machine catalog on this mount.
  { base: 'document', re: /\bfetch\s*\(\s*['"]([^'"]+)['"]/g }, // fetch('./x.json')
];

function walk(dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const dangling = [];
for (const absolute of walk(targetRoot)) {
  if (!/\.(js|mjs|html|css)$/.test(absolute)) continue;
  // Source maps are a debugging aid, not a runtime reference, and they legitimately name paths
  // that were never staged (the original TypeScript sources).
  if (absolute.endsWith('.map')) continue;
  const relative = path.relative(targetRoot, absolute);
  const text = fs.readFileSync(absolute, 'utf8');
  const fromDir = path.dirname(absolute);

  for (const { base, re } of REFERENCE_PATTERNS) {
    for (const [, spec] of text.matchAll(re)) {
      const clean = spec.split(/[?#]/)[0];
      if (clean === '') continue;
      // A bare specifier is a bundler/CDN concern, and a data: or protocol URL is not ours.
      if (!clean.startsWith('.') && !clean.startsWith('/')) continue;
      if (/^[a-z]+:/i.test(clean)) continue;
      // Vite emits absolute URLs under the configured base; those resolve from the mount root.
      const resolved = clean.startsWith('/')
        ? path.join(targetRoot, clean.replace(/^\/gallery\//, '/'))
        : path.resolve(base === 'document' ? targetRoot : fromDir, clean);
      if (!resolved.startsWith(targetRoot)) continue;
      if (fs.existsSync(resolved)) continue;
      dangling.push(`  gallery/${relative} references ${spec} -> not staged`);
    }
  }
}

if (dangling.length > 0) {
  throw new Error(
    `The staged gallery references ${dangling.length} file(s) that were not staged.\n` +
      `${dangling.join('\n')}\n` +
      'Left unstaged they are a 404 in the browser, which no build step would otherwise catch.',
  );
}

const staged = walk(targetRoot).length;
console.log(`staged gallery from web/dist-gallery (${staged} files) -> .vitepress/dist/gallery`);
