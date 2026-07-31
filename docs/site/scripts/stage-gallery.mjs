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

console.log(`staged public gallery (${files.length} allow-listed files) -> ${path.relative(repoRoot, targetRoot)}`);
