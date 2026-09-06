import assert from 'node:assert/strict';
import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const threeSri = 'sha384-CI3ELBVUz9XQO+97x6nwMDPosPR5XvsxW2ua7N1Xeygeh1IxtgqtCkGfQY9WWdHu';
const controlsSri = 'sha384-wagZhIFgY4hD+7awjQjR4e2E294y6J2HSnd8eTNc15ZubTeQeVRZwhQJ+W6hnBsf';

for (const path of [
  'web/visualizer.html',
  'examples/output/graded_tpms_viewer.html',
  'examples/output/twisted_vase_3d_viewer.html',
  'examples/output/trochoidal_pocket_viewer.html',
  'sdk/ts/src/visualizer.ts',
  'py/python/dry/visualizer.py',
]) {
  const source = read(path);
  assert.ok(source.includes(threeSri), `${path}: Three.js must carry the pinned SRI digest`);
  assert.ok(source.includes(controlsSri), `${path}: OrbitControls must carry the pinned SRI digest`);
  assert.ok(source.includes('crossorigin="anonymous"'), `${path}: cross-origin SRI must be enabled`);
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
assert.match(codeql, /- web\/vendor\/\*\*/, 'only the vendored web dependency is excluded');
assert.doesNotMatch(codeql, /sdk\/|services\/|crates\/|py\//, 'first-party product code must remain scanned');

console.log('web security regressions passed');
