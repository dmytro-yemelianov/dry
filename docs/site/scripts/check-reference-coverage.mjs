#!/usr/bin/env node
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(siteRoot, '../..');
const generatedDir = path.join(siteRoot, 'reference/generated');

function generatedPath(relativePath) {
  return path.join(generatedDir, relativePath);
}

function readGenerated(relativePath) {
  const file = generatedPath(relativePath);
  return fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
}

function readGeneratedMany(paths) {
  return paths.map(readGenerated).join('\n\n');
}

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
  const tableMatch = cliPage.match(/\| Command \|[\s\S]*?(?=\n## |\n### |$)/);
  if (!tableMatch) return commands;
  for (const line of tableMatch[0].split('\n')) {
    const linked = line.match(/^\| \[`([^`]+)`\]\([^)]+\) \|/);
    const plain = line.match(/^\| `([^`]+)` \|/);
    const match = linked || plain;
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

for (const requiredPath of ['typescript-sdk/design.md', 'typescript-sdk/types.md', 'typescript-sdk/generators.md']) {
  if (!fs.existsSync(generatedPath(requiredPath))) fail(`Missing generated TypeScript child page: ${requiredPath}`);
}
const tsPage = readGeneratedMany(['typescript-sdk.md', 'typescript-sdk/design.md', 'typescript-sdk/types.md', 'typescript-sdk/generators.md']);
for (const name of parseTsExports()) {
  if (!tsPage.includes(`\`${name}\``)) fail(`TypeScript export is missing from generated reference: ${name}`);
}
for (const required of ['### Fields', '### Parameters', '### Method summary']) {
  if (!tsPage.includes(required)) fail(`TypeScript reference is missing structured section: ${required}`);
}

for (const requiredPath of ['python-sdk/design.md', 'python-sdk/module.md']) {
  if (!fs.existsSync(generatedPath(requiredPath))) fail(`Missing generated Python child page: ${requiredPath}`);
}
const pyPage = readGeneratedMany(['python-sdk.md', 'python-sdk/design.md', 'python-sdk/module.md']);
for (const name of parsePythonAll()) {
  if (!pyPage.includes(`\`${name}\``)) fail(`Python public name is missing from generated reference: ${name}`);
}
for (const required of ['### Method summary', '| Parameter | Annotation | Default | Required |']) {
  if (!pyPage.includes(required)) fail(`Python reference is missing structured section: ${required}`);
}

const cliPage = readGenerated('cli.md');
if (!cliPage.includes('Generated from actual CLI help output.')) fail('CLI reference did not use actual CLI help output.');
if (!cliPage.includes('## Root help')) fail('CLI reference is missing root help.');
for (const command of parseCliCommandsFromPage(cliPage)) {
  const commandPagePath = `cli/${command}.md`;
  const commandPage = readGenerated(commandPagePath);
  if (!commandPage) fail(`CLI command is missing generated child page: ${command}`);
  if (commandPage && !commandPage.includes(`# \`dry ${command}\``)) fail(`CLI command page is missing heading: ${command}`);
}

for (const requiredPath of ['ir/data-model.md', 'ir/json-wire-form.md']) {
  if (!fs.existsSync(generatedPath(requiredPath))) fail(`Missing generated IR child page: ${requiredPath}`);
}
const irPage = readGeneratedMany(['ir.md', 'ir/data-model.md', 'ir/json-wire-form.md']);
for (const required of ['## 3. Data model', '### 3.3 `Segment`', '## 4. JSON wire form']) {
  if (!irPage.includes(required)) fail(`IR reference is missing extracted section: ${required}`);
}

for (const requiredPath of [
  'profiles-and-reports/profile-schema.md',
  'profiles-and-reports/verification-rules.md',
  'profiles-and-reports/report-outputs.md',
  'profiles-and-reports/supported-profile-matrix.md',
]) {
  if (!fs.existsSync(generatedPath(requiredPath))) fail(`Missing generated profiles child page: ${requiredPath}`);
}
const profilesPage = readGeneratedMany([
  'profiles-and-reports.md',
  'profiles-and-reports/profile-schema.md',
  'profiles-and-reports/verification-rules.md',
  'profiles-and-reports/report-outputs.md',
  'profiles-and-reports/supported-profile-matrix.md',
]);
for (const required of ['## 1. Profile schema (v1)', '## 2. Verification rule catalog', '## Supported profile matrix']) {
  if (!profilesPage.includes(required)) fail(`Profiles reference is missing extracted section: ${required}`);
}

const examples = JSON.parse(read('docs/site/reference/source/examples.json'));
const examplesPage = readGenerated('examples.md');
const inlineSamplePages = readGeneratedMany([
  'typescript-sdk/design.md',
  'typescript-sdk/generators.md',
  'python-sdk/design.md',
  'python-sdk/module.md',
  'cli/emit.md',
  'cli/simulate.md',
  'cli/verify.md',
  'cli/rewrite-gcode.md',
  'generators.md',
  'verification.md',
]);
for (const example of examples) {
  if (!examplesPage.includes(`/guide/${example.slug}`)) fail(`Example guide link is missing from generated reference: ${example.slug}`);
  const previewPath = `docs/site/public/reference/previews/${example.slug}.svg`;
  if (!fs.existsSync(repoPath(previewPath))) fail(`Example preview image is missing: ${previewPath}`);
  if (!examplesPage.includes(`/reference/previews/${example.slug}.svg`)) fail(`Example preview image is missing from generated reference: ${example.slug}`);
  if (!inlineSamplePages.includes(`/reference/previews/${example.slug}.svg`)) fail(`Example preview image is missing from inline generated reference docs: ${example.slug}`);
  for (const source of Object.values(example.sources)) {
    if (!examplesPage.includes(source)) fail(`Example source is missing from generated reference: ${source}`);
  }
}

if (failures.length) {
  console.error(failures.map((item) => `- ${item}`).join('\n'));
  process.exit(1);
}

console.log('reference coverage ok');
