#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceRoot = path.join(siteRoot, 'public');
const targetRoot = path.join(siteRoot, '.public-docs');

const files = [
  '_headers',
  'reference/previews/author.svg',
  'reference/previews/generative.svg',
  'reference/previews/lower.svg',
  'reference/previews/optimize.svg',
  'reference/previews/simulate.svg',
  'reference/previews/verify.svg',
];

fs.rmSync(targetRoot, { recursive: true, force: true });
for (const relative of files) {
  const source = path.join(sourceRoot, relative);
  if (!fs.existsSync(source)) throw new Error(`Missing public documentation asset: ${relative}`);
  const target = path.join(targetRoot, relative);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(source, target);
}

console.log(`staged public documentation assets (${files.length} allow-listed files) -> .public-docs`);
