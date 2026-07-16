#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { JSDOM } from 'jsdom';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const distRoot = path.join(siteRoot, '.vitepress/dist');

function walkHtml(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = path.join(directory, entry.name);
    if (entry.isDirectory()) return walkHtml(absolute);
    return entry.name.endsWith('.html') ? [absolute] : [];
  });
}

function targetFile(pathname) {
  const relative = decodeURIComponent(pathname).replace(/^\/+/, '');
  const candidates = relative
    ? [relative, `${relative}.html`, path.join(relative, 'index.html')]
    : ['index.html'];
  return candidates.map((candidate) => path.join(distRoot, candidate)).find(fs.existsSync);
}

if (!fs.existsSync(distRoot)) {
  console.error('Missing VitePress output. Run npm run build first.');
  process.exit(1);
}

const pages = walkHtml(distRoot);
const documents = new Map(
  pages.map((file) => [file, new JSDOM(fs.readFileSync(file, 'utf8')).window.document])
);
const failures = new Set();

for (const [sourceFile, document] of documents) {
  const sourceRelative = path.relative(distRoot, sourceFile).split(path.sep).join('/');
  const sourceUrl = new URL(sourceRelative, 'https://dry.local/');

  for (const anchor of document.querySelectorAll('a[href]')) {
    const href = anchor.getAttribute('href');
    if (!href || /^(?:mailto:|tel:|javascript:|data:)/i.test(href)) continue;

    const url = new URL(href, sourceUrl);
    if (url.origin !== sourceUrl.origin) continue;

    const destination = targetFile(url.pathname);
    if (!destination) {
      failures.add(`${sourceRelative} -> ${href} (missing page)`);
      continue;
    }

    if (!url.hash || !destination.endsWith('.html')) continue;
    const targetDocument = documents.get(destination);
    const id = decodeURIComponent(url.hash.slice(1));
    if (!targetDocument?.getElementById(id)) {
      failures.add(`${sourceRelative} -> ${href} (missing anchor #${id})`);
    }
  }
}

if (failures.size) {
  console.error([...failures].sort().map((failure) => `- ${failure}`).join('\n'));
  process.exit(1);
}

console.log(`internal link check ok (${pages.length} rendered pages)`);
