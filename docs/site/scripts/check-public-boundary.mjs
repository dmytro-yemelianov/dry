#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const distRoot = path.join(siteRoot, '.vitepress/dist');
const forbiddenDirectories = ['pkg', 'gallery'];
const forbiddenExtensions = ['.wasm', '.tgz', '.whl', '.tar.gz'];
const forbiddenImplementationMarkers = ['SnippetWorkerClient', 'initDryWeb', 'dry_wasm_bg.wasm'];
const failures = [];

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(absolute) : [absolute];
  });
}

for (const directory of forbiddenDirectories) {
  if (fs.existsSync(path.join(distRoot, directory))) failures.push(`forbidden directory: /${directory}`);
}

for (const file of walk(distRoot)) {
  const relative = path.relative(distRoot, file).split(path.sep).join('/');
  if (forbiddenExtensions.some((extension) => relative.endsWith(extension))) {
    failures.push(`forbidden artifact: ${relative}`);
  }
  if (!relative.endsWith('.js')) continue;
  const source = fs.readFileSync(file, 'utf8');
  for (const marker of forbiddenImplementationMarkers) {
    if (source.includes(marker)) failures.push(`implementation marker ${marker} in ${relative}`);
  }
}

if (failures.length) {
  console.error(failures.map((failure) => `- ${failure}`).join('\n'));
  process.exit(1);
}

console.log('public documentation boundary ok (no product packages, WASM, gallery, or SDK runtime)');
