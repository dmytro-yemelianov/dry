import assert from 'node:assert/strict';
import fs from 'node:fs';

const read = (path) => fs.readFileSync(path, 'utf8');
const threeSri = 'sha384-CI3ELBVUz9XQO+97x6nwMDPosPR5XvsxW2ua7N1Xeygeh1IxtgqtCkGfQY9WWdHu';
const controlsSri = 'sha384-wagZhIFgY4hD+7awjQjR4e2E294y6J2HSnd8eTNc15ZubTeQeVRZwhQJ+W6hnBsf';
const threeUrl = 'https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js';
const controlsUrl = 'https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js';

function requirePinnedScript(source, path, url, digest) {
  const externalScripts = source.match(/<script\b[^>]*\ssrc="https:\/\/[^"]+"[^>]*><\/script>/g) ?? [];
  const matchingTags = externalScripts.filter(
    (candidate) => candidate.match(/\ssrc="([^"]+)"/)?.[1] === url,
  );
  assert.ok(matchingTags.length > 0, `${path}: expected external script ${url}`);
  for (const tag of matchingTags) {
    assert.ok(tag.includes(`integrity="${digest}"`), `${path}: ${url} must carry its pinned SRI digest`);
    assert.ok(tag.includes('crossorigin="anonymous"'), `${path}: ${url} must enable cross-origin SRI`);
  }
}

assert.throws(
  () => requirePinnedScript(
    `<script src="${threeUrl}" integrity="${threeSri}" crossorigin="anonymous"></script>`
      + `<script src="${threeUrl}"></script>`,
    'duplicate-script-regression',
    threeUrl,
    threeSri,
  ),
  /pinned SRI digest/,
  'every duplicate external script tag must be protected',
);
assert.throws(
  () => requirePinnedScript(`<script data-src="${threeUrl}"></script>`, 'data-src-regression', threeUrl, threeSri),
  /expected external script/,
  'data-src must not be treated as an executable src attribute',
);

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
const activeCodeqlPolicy = codeql
  .split('\n')
  .map((line) => line.trim())
  .filter((line) => line !== '' && !line.startsWith('#'));
assert.deepEqual(
  activeCodeqlPolicy,
  ['name: DryMachina CodeQL configuration', 'paths-ignore:', '- web/vendor/**'],
  'CodeQL policy must contain only the single vendored-source exclusion',
);

console.log('web security regressions passed');
