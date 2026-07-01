#!/usr/bin/env node
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(siteRoot, '../..');
const generatedDir = path.join(siteRoot, 'reference/generated');

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  return fs.readFileSync(repoPath(relativePath), 'utf8');
}

function hash(relativePath) {
  return createHash('sha256').update(read(relativePath)).digest('hex');
}

function fail(message) {
  failures.push(message);
}

function parseTsExports() {
  const indexSource = read('sdk/ts/src/index.ts');
  const names = [];
  const groupRe = /export\s+(?:type\s+)?\{([\s\S]*?)\}\s+from\s+['"].+?['"]/g;
  let match;
  while ((match = groupRe.exec(indexSource))) {
    for (const name of match[1].split(',')) {
      const clean = name.trim().replace(/\s+as\s+.+$/, '').trim();
      if (clean) names.push(clean);
    }
  }
  return names;
}

function parsePythonAll() {
  const source = read('py/python/dry/__init__.py');
  const match = source.match(/__all__\s*=\s*\[([\s\S]*?)\]/);
  if (!match) return [];
  return [...match[1].matchAll(/["']([^"']+)["']/g)].map((item) => item[1]);
}

function parseCliCommandsFromPage(cliPage) {
  const commands = [];
  const tableMatch = cliPage.match(/\| Command \| Summary \|[\s\S]*?(?=\n## |\n### |$)/);
  if (!tableMatch) return commands;
  for (const line of tableMatch[0].split('\n')) {
    const match = line.match(/^\| `([^`]+)` \|/);
    if (match) commands.push(match[1]);
  }
  return commands;
}

const failures = [];

const manifestPath = path.join(generatedDir, 'manifest.json');
if (!fs.existsSync(manifestPath)) {
  fail('Missing generated manifest. Run npm run docs:gen.');
} else {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  for (const [key, item] of Object.entries(manifest.sources || {})) {
    if (!fs.existsSync(repoPath(item.path))) {
      fail(`Manifest source ${key} is missing: ${item.path}`);
      continue;
    }
    const currentHash = hash(item.path);
    if (currentHash !== item.sha256) {
      fail(`Generated reference is stale for ${item.path}. Run npm run docs:gen.`);
    }
  }
}

const tsPagePath = path.join(generatedDir, 'typescript-sdk.md');
const tsPage = fs.existsSync(tsPagePath) ? fs.readFileSync(tsPagePath, 'utf8') : '';
for (const name of parseTsExports()) {
  if (!tsPage.includes(`\`${name}\``)) fail(`TypeScript export is missing from generated reference: ${name}`);
}
for (const required of ['### Fields', '### Parameters', '### Method details']) {
  if (!tsPage.includes(required)) fail(`TypeScript reference is missing structured section: ${required}`);
}

const pyPagePath = path.join(generatedDir, 'python-sdk.md');
const pyPage = fs.existsSync(pyPagePath) ? fs.readFileSync(pyPagePath, 'utf8') : '';
for (const name of parsePythonAll()) {
  if (!pyPage.includes(`\`${name}\``)) fail(`Python public name is missing from generated reference: ${name}`);
}
for (const required of ['### Method details', '| Parameter | Annotation | Default | Required |']) {
  if (!pyPage.includes(required)) fail(`Python reference is missing structured section: ${required}`);
}

const cliPagePath = path.join(generatedDir, 'cli.md');
const cliPage = fs.existsSync(cliPagePath) ? fs.readFileSync(cliPagePath, 'utf8') : '';
if (!cliPage.includes('Generated from actual CLI help output.')) fail('CLI reference did not use actual CLI help output.');
if (!cliPage.includes('## Root help')) fail('CLI reference is missing root help.');
for (const command of parseCliCommandsFromPage(cliPage)) {
  if (!cliPage.includes(`### \`${command}\``)) fail(`CLI command is missing detailed help section: ${command}`);
}

const examples = JSON.parse(read('docs/site/reference/source/examples.json'));
const examplesPage = fs.existsSync(path.join(generatedDir, 'examples.md'))
  ? fs.readFileSync(path.join(generatedDir, 'examples.md'), 'utf8')
  : '';
for (const example of examples) {
  if (!examplesPage.includes(`/guide/${example.slug}`)) fail(`Example guide link is missing from generated reference: ${example.slug}`);
  for (const source of Object.values(example.sources)) {
    if (!examplesPage.includes(source)) fail(`Example source is missing from generated reference: ${source}`);
  }
}

if (failures.length) {
  console.error(failures.map((item) => `- ${item}`).join('\n'));
  process.exit(1);
}

console.log('reference coverage ok');
