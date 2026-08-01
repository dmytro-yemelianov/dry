#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(siteRoot, '../..');
const webRoot = path.join(repoRoot, 'web');
const targetRoot = path.join(siteRoot, '.vitepress/dist/gallery');

const files = [
  'index.html',
  'tool-ui.css',
  'designs.js',
  'fullcontrol-gallery.generated.js',
  'lattice-research.js',
  'tpms-engine.js',
  'viewer.js',
  'thumb.js',
  'spline.js',
  'wasm-load.js',
  'pkg/dry_wasm.js',
  'pkg/dry_wasm_bg.wasm',
  'vendor/OrbitControls.js',
  'vendor/three.module.js',
];

fs.rmSync(targetRoot, { recursive: true, force: true });
for (const relative of files) {
  const source = path.join(webRoot, relative);
  if (!fs.existsSync(source)) throw new Error(`Missing gallery build input: web/${relative}`);
  const target = path.join(targetRoot, relative);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(source, target);
}

// The allow list is checked in one direction above: every entry must exist in `web/`. The other
// direction had no check at all, and that is the half that actually shipped broken (#190). When
// `web/tpms.js` was replaced by `web/tpms-engine.js`, the replacement was *also* missing from the
// list — so had the stale entry merely been deleted, staging would have succeeded and the gallery
// would have 404'd on a module it imports, at runtime, in the browser, with nothing failing here.
//
// So: walk what was actually staged, collect the relative modules and assets it references, and
// require each one to have been staged too.
const REFERENCE_PATTERNS = [
  /\bfrom\s*['"](\.\.?\/[^'"]+)['"]/g, // import ... from './x.js'
  /\bimport\s*\(\s*['"](\.\.?\/[^'"]+)['"]\s*\)/g, // dynamic import('./x.js')
  /\bsrc\s*=\s*['"](\.\.?\/[^'"]+)['"]/g, // <script src="./x.js">
  /\bhref\s*=\s*['"](\.\.?\/[^'"]+\.css)['"]/g, // <link href="./x.css">
];

const dangling = [];
for (const relative of files) {
  if (!/\.(js|mjs|html|css)$/.test(relative)) continue;
  const text = fs.readFileSync(path.join(targetRoot, relative), 'utf8');
  const fromDir = path.dirname(path.join(targetRoot, relative));

  for (const pattern of REFERENCE_PATTERNS) {
    for (const [, spec] of text.matchAll(pattern)) {
      // Only same-tree references are our problem; a bare specifier is a bundler/CDN concern and a
      // query string is a cache-buster, not a different file.
      const resolved = path.resolve(fromDir, spec.split(/[?#]/)[0]);
      if (!resolved.startsWith(targetRoot)) continue;
      if (fs.existsSync(resolved)) continue;
      dangling.push(`  web/${relative} references ${spec} -> not staged`);
    }
  }
}

if (dangling.length > 0) {
  throw new Error(
    `The staged gallery references ${dangling.length} file(s) the allow list above does not stage.\n` +
      `${dangling.join('\n')}\n` +
      'Add them to `files` in this script, or stop referencing them. Left unstaged they are a 404 ' +
      'in the browser, which no build step would otherwise catch.',
  );
}

console.log(
  `staged public gallery (${files.length} allow-listed files, references resolved) -> ${path.relative(repoRoot, targetRoot)}`,
);
