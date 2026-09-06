import assert from 'node:assert/strict';
import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const threeSri = 'sha384-CI3ELBVUz9XQO+97x6nwMDPosPR5XvsxW2ua7N1Xeygeh1IxtgqtCkGfQY9WWdHu';
const controlsSri = 'sha384-wagZhIFgY4hD+7awjQjR4e2E294y6J2HSnd8eTNc15ZubTeQeVRZwhQJ+W6hnBsf';
const threeUrl = 'https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js';
const controlsUrl = 'https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js';

function requirePinnedScript(source, path, url, digest) {
  const externalScripts = source.match(/<script\b[^>]*\bsrc="https:\/\/[^\"]+"[^>]*><\/script>/g) ?? [];
  const tag = externalScripts.find((candidate) => candidate.includes(`src="${url}"`));
  assert.ok(tag, `${path}: expected external script ${url}`);
  assert.ok(tag.includes(`integrity="${digest}"`), `${path}: ${url} must carry its pinned SRI digest`);
  assert.ok(tag.includes('crossorigin="anonymous"'), `${path}: ${url} must enable cross-origin SRI`);
}

for (const path of [
  'web/visualizer.html',
  'examples/output/graded_tpms_viewer.html',
  'examples/output/twisted_vase_3d_viewer.html',
  'examples/output/trochoidal_pocket_viewer.html',
  'sdk/ts/src/visualizer.ts',
  'py/python/dry/visualizer.py',
]) {
  const source = read(path);
  requirePinnedScript(source, path, threeUrl, threeSri);
  requirePinnedScript(source, path, controlsUrl, controlsSri);
}

const verifier = read('web/verify.html');
assert.doesNotMatch(
  verifier,
  /findingsContainer['"]\)\.innerHTML|container\.innerHTML/,
  'verification findings and exceptions must never be reinterpreted as HTML',
);
assert.match(verifier, /replaceChildren\(/, 'verification output must use DOM-safe replacement');

const design = read('sdk/ts/src/design.ts');
assert.doesNotMatch(design, /const DECIMAL\s*=\s*\//, 'contract decimals must use the linear scanner');
assert.match(design, /function isPlainDecimal\(/, 'the linear decimal scanner is required');

const codeql = read('.github/codeql/codeql-config.yml');
const pathsIgnoreBlock = codeql.match(/^paths-ignore:\s*\n((?:\s+-\s+[^\n]+\n?)*)/m);
assert.ok(pathsIgnoreBlock, 'CodeQL configuration must declare paths-ignore');
const ignoredPaths = pathsIgnoreBlock[1]
  .split('\n')
  .map((line) => line.match(/^\s*-\s+(.+?)\s*$/)?.[1])
  .filter(Boolean);
assert.deepEqual(ignoredPaths, ['web/vendor/**'], 'CodeQL must exclude only the vendored web dependency');

console.log('web security regressions passed');
