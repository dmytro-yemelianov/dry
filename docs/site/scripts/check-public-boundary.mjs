#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const distRoot = path.join(siteRoot, '.vitepress/dist');
const sourceManifestName = '.dry-public-source-manifest.json';
const sourceManifestPath = path.join(distRoot, sourceManifestName);
const forbiddenDirectories = ['pkg', 'gallery'];
const forbiddenExtensions = ['.wasm', '.tgz', '.whl', '.tar.gz'];
const forbiddenImplementationMarkers = [
  'SnippetWorkerClient',
  'initDryWeb',
  'dry_wasm_bg.wasm',
  'snippet.worker',
];
const forbiddenSourceRoots = ['crates/', 'py/', 'sdk/', 'web/'];
const forbiddenProductSources = [
  'docs/site/.vitepress/theme/LiveExample.vue',
  'docs/site/.vitepress/theme/dry-engine',
  'docs/site/.vitepress/theme/run-snippet',
  'docs/site/.vitepress/theme/snippet-output',
  'docs/site/.vitepress/theme/snippet-worker-client',
  'docs/site/.vitepress/theme/snippet.worker',
  'docs/site/.vitepress/theme/three-ir-viewer',
];
const allowedAssetExtensions = new Set(['.css', '.js', '.map', '.mjs', '.woff', '.woff2']);
const allowedRootFiles = new Set(['_headers', 'hashmap.json', 'vp-icons.css', sourceManifestName]);
const documentedPreviews = new Set([
  'reference/previews/author.svg',
  'reference/previews/generative.svg',
  'reference/previews/lower.svg',
  'reference/previews/optimize.svg',
  'reference/previews/simulate.svg',
  'reference/previews/verify.svg',
]);
const allowedPublicSourceFiles = new Set([
  '\0plugin-vue:export-helper',
  'docs/site/.vitepress/theme/PublicExample.vue',
  'docs/site/.vitepress/theme/index.ts',
  'docs/site/.vitepress/theme/style.css',
  'docs/site/examples/author.ts',
  'docs/site/examples/generative.ts',
  'docs/site/examples/lower.ts',
  'docs/site/examples/optimize.ts',
  'docs/site/examples/simulate.ts',
  'docs/site/examples/verify.ts',
  'docs/site/activate.md',
  'docs/site/index.md',
  'docs/site/licensing.md',
  'docs/site/pricing.md',
  'public-url:/@siteData',
  ...[...documentedPreviews].map((preview) => `public-url:/${preview}`),
]);
const allowedPublicContentPrefixes = [
  'docs/site/cloud/',
  'docs/site/guide/',
  'docs/site/marketing/',
  'docs/site/reference/',
];
const textExtensions = new Set(['', '.cjs', '.css', '.html', '.js', '.json', '.map', '.mjs', '.svg']);
const failures = [];

/** Recursively enumerate every emitted public documentation file. */
function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(absolute) : [absolute];
  });
}

/** Return whether an emitted file belongs to the explicit public artifact allowlist. */
function isAllowedOutput(relative) {
  if (allowedRootFiles.has(relative) || relative.endsWith('.html')) return true;
  if (documentedPreviews.has(relative)) return true;
  return relative.startsWith('assets/') && allowedAssetExtensions.has(path.extname(relative));
}

for (const file of walk(distRoot)) {
  const relative = path.relative(distRoot, file).split(path.sep).join('/');
  const components = relative.toLowerCase().split('/');
  for (const directory of forbiddenDirectories) {
    if (components.includes(directory)) failures.push(`forbidden directory component: ${relative}`);
  }
  if (forbiddenExtensions.some((extension) => relative.endsWith(extension))) {
    failures.push(`forbidden artifact: ${relative}`);
  }
  if (!isAllowedOutput(relative)) failures.push(`output is not allow-listed: ${relative}`);
  if (!textExtensions.has(path.extname(relative)) && relative !== '_headers') continue;
  const source = fs.readFileSync(file, 'utf8');
  for (const marker of forbiddenImplementationMarkers) {
    if (source.includes(marker)) failures.push(`implementation marker ${marker} in ${relative}`);
  }
}

if (!fs.existsSync(sourceManifestPath)) {
  failures.push(`missing source provenance manifest: ${sourceManifestName}`);
} else {
  const modules = JSON.parse(fs.readFileSync(sourceManifestPath, 'utf8'));
  if (!Array.isArray(modules) || modules.some((moduleId) => typeof moduleId !== 'string')) {
    failures.push(`invalid source provenance manifest: ${sourceManifestName}`);
  } else {
    for (const moduleId of modules) {
      if (forbiddenSourceRoots.some((root) => moduleId.startsWith(root))) {
        failures.push(`product source included in public bundle: ${moduleId}`);
      }
      if (forbiddenProductSources.some((source) => moduleId.startsWith(source))) {
        failures.push(`interactive product source included in public bundle: ${moduleId}`);
      }
      const approvedDocumentationPage =
        moduleId.endsWith('.md') &&
        allowedPublicContentPrefixes.some((prefix) => moduleId.startsWith(prefix));
      const knownPublicSource =
        allowedPublicSourceFiles.has(moduleId) ||
        approvedDocumentationPage ||
        moduleId.startsWith('node_modules/') ||
        moduleId.includes('/node_modules/');
      if (!knownPublicSource) failures.push(`source module is outside the public allowlist: ${moduleId}`);
    }
  }
  fs.rmSync(sourceManifestPath);
}

if (failures.length) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'));
  process.exit(1);
}

console.log('public documentation boundary ok (allow-listed artifacts and source provenance verified)');
