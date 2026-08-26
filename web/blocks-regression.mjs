import fs from 'node:fs';

function read(path) {
  return fs.readFileSync(new URL(path, import.meta.url), 'utf8');
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const blocksHtml = read('blocks.html');
const templatesJs = read('templates.js');
const patternsJs = read('patterns.js');
const tpmsEngineJs = read('tpms-engine.js');
const designsJs = read('designs.js');
const viewerJs = read('viewer.js');
// Studio 2.0 turned index.html into a Vite entry point, so the gallery assertions that used to
// live here moved to docs/site/smoke, where they run against the built app in a browser instead of
// grepping source text. The entry point itself is still shipped, so it stays under the innerHTML
// hygiene check below.
const indexHtml = read('index.html');
const toolUiCss = read('tool-ui.css');

assert(blocksHtml.includes('Blockly.Blocks.dry_spline'), 'dynamic dry_spline block is missing');
assert(blocksHtml.includes('mutationToDom()'), 'dry_spline XML mutation writer is missing');
assert(blocksHtml.includes('domToMutation(xmlElement)'), 'dry_spline XML mutation reader is missing');
assert(blocksHtml.includes('saveExtraState()'), 'dry_spline JSON state writer is missing');
assert(blocksHtml.includes('loadExtraState(state)'), 'dry_spline JSON state reader is missing');
assert(!blocksHtml.includes('dry_spline3'), 'fixed dry_spline3 block leaked into blocks.html');
assert(!templatesJs.includes('dry_spline3'), 'fixed dry_spline3 block leaked into templates.js');

for (const category of ['Dry setup', 'Dry motion', 'Dry patterns', 'Dry process', 'Flow', 'Logic', 'Math', 'Lists', 'Variables']) {
  assert(blocksHtml.includes(`<category name="${category}"`), `toolbox missing ${category} category`);
}

for (const type of ['controls_repeat_ext', 'controls_for', 'math_change']) {
  assert(blocksHtml.includes(`<block type="${type}"`), `toolbox missing ${type}`);
  assert(blocksHtml.includes(`case '${type}'`), `generator missing ${type}`);
}

for (const type of ['dry_sin_rad', 'dry_cos_rad']) {
  assert(blocksHtml.includes(`type: '${type}'`), `custom ${type} block is missing`);
  assert(blocksHtml.includes(`<block type="${type}"`), `toolbox missing ${type}`);
  assert(blocksHtml.includes(`generator.${type}`), `generator missing ${type}`);
}
assert(blocksHtml.includes("type: 'dry_vase_helix'"), 'compact vase helix block is missing');
assert(blocksHtml.includes('<block type="dry_vase_helix"'), 'toolbox missing compact vase helix block');
assert(blocksHtml.includes("case 'dry_vase_helix'"), 'generator missing compact vase helix block');
assert(blocksHtml.includes('MAX_PATTERN_POINTS'), 'pattern generators should cap emitted point counts');

assert(blocksHtml.includes('safeEvalExpression'), 'expression evaluator should use the safe parser');
assert(!blocksHtml.includes('new Function'), 'expression evaluator must not execute generated JavaScript');
assert(blocksHtml.includes('SAFE_HELPERS'), 'expression evaluator should whitelist generated helpers');
assert(blocksHtml.includes('safeMathMember'), 'expression evaluator should guard Math member access');
assert(blocksHtml.includes('safeIndex'), 'expression evaluator should guard bracket access');
assert(blocksHtml.includes('value = safeIndex(value, index)'), 'expression evaluator must not use raw bracket property access');
assert(viewerJs.includes('VIEW_PANELS'), 'viewer multi-view panel definitions are missing');
assert(viewerJs.includes('renderViews'), 'viewer multi-view renderer is missing');
assert(viewerJs.includes('activeViewPanels'), 'viewer should switch between Iso-only and multi-view panels');
assert(viewerJs.includes('viewModeEl'), 'viewer missing view-mode control binding');
assert(viewerJs.includes("R.viewMode === 'iso'"), 'viewer should support Iso-only view mode');
assert(viewerJs.includes('cameraRect'), 'viewer split viewport geometry is missing');
assert(viewerJs.includes("key: 'front'"), 'viewer should include a front XZ view');
assert(viewerJs.includes("new OrbitControls(cameras.get('iso'), el)"), 'OrbitControls should bind to the full viewport element');
assert(viewerJs.includes('resetViewEl'), 'viewer should expose a reset view control');
assert(viewerJs.includes('__viewerDebug'), 'viewer should expose debug state for interaction checks');
assert(viewerJs.includes("addEventListener('wheel'"), 'viewer should claim wheel events before scroll containers');
assert(viewerJs.includes('inputStats'), 'viewer should expose interaction counters for viewport debugging');
assert(viewerJs.includes('previousRatio'), 'viewer should preserve playback ratio across model refreshes');
assert(viewerJs.includes('preserveIso'), 'viewer should preserve the Iso camera when params update');
assert(viewerJs.includes('setModel(ir, { preserveState: V.hasModel })'), 'viewer should only fit camera on first render');
assert(viewerJs.includes("className = 'speed-select'"), 'speed controls should use a compact select menu');
assert(!viewerJs.includes("1× realtime"), 'speed controls should use compact labels');
assert(viewerJs.includes('uGhostAlpha'), 'viewer should render unprinted bead geometry with partial transparency');
assert(viewerJs.includes('transparent: true'), 'viewer bead material should support printed/planned alpha');
assert(viewerJs.includes('formatDuration'), 'viewer should format time as human-readable duration');
assert(viewerJs.includes('metric-number'), 'viewer metrics should split numeric values from units');
assert(viewerJs.includes('MAX_GCODE_DOM_ROWS'), 'viewer should cap large G-code DOM rendering');
assert(viewerJs.includes('AUTO_MESH_MOVE_LIMIT'), 'viewer should auto-fallback for large render meshes');
assert(viewerJs.includes('buildRoundedBeads'), 'viewer should expose realistic rounded bead rendering');
assert(viewerJs.includes('buildTimedLineGeometry'), 'viewer should cache timed line geometry for large/travel paths');
assert(viewerJs.includes('nearestLayerKey'), 'viewer should support single-layer travel filtering');
assert(viewerJs.includes('buildGcodeSections'), 'viewer should build G-code layer sections');
assert(viewerJs.includes('renderGcodeTools'), 'viewer should render G-code section/search controls');
assert(viewerJs.includes('renderGcodeLine'), 'viewer should render tokenized G-code rows');
assert(viewerJs.includes('groupFindings'), 'viewer should group repeated verification findings');
assert(viewerJs.includes('renderFindingGroup'), 'viewer should render grouped verification summaries');
assert(viewerJs.includes('window.__dryProfile'), 'viewer should expose handling/rendering profile data');
assert(viewerJs.includes('keepLineVisible'), 'viewer should scroll only the g-code panel for active lines');
assert(!viewerJs.includes('scrollIntoView'), 'active g-code line should not scroll parent layout panels');
assert(viewerJs.includes('view-grid-labels'), 'viewer does not render multi-view labels');
assert(blocksHtml.includes('.view-grid-labels'), 'blocks page missing multi-view styles');
assert(blocksHtml.includes('tool-ui.css'), 'blocks page does not load shared UI stylesheet');
assert(blocksHtml.includes('class="topbar"'), 'blocks page missing flex topbar');
assert(blocksHtml.includes('id="resetView"'), 'blocks page missing reset view control');
assert(blocksHtml.includes('Fit</button>'), 'blocks reset control should use compact Fit label');
assert(blocksHtml.includes('<div class="speeds" id="speeds"><span class="lbl">speed</span></div>\n    </div>'), 'blocks speed controls should live inside the playback strip');
assert(blocksHtml.includes('id="fitBlocks"'), 'blocks page missing fit workspace control');
assert(blocksHtml.includes('id="cleanBlocks"'), 'blocks page missing clean workspace control');
assert(blocksHtml.includes('zoomToFit'), 'blocks page should fit loaded templates into view');
assert(!blocksHtml.includes('<div class="legend">'), 'blocks should not duplicate path toggles with a separate legend');
assert(!blocksHtml.includes('.legend'), 'blocks should not retain dead legend CSS');
for (const source of [blocksHtml]) {
  for (const layer of ['printed', 'planned', 'travel', 'travelLayer', 'toolhead', 'bed']) {
    assert(source.includes(`data-render-layer="${layer}"`), `render controls missing ${layer} toggle`);
  }
  assert(source.includes('data-render-mode'), 'render controls missing render quality selector');
  assert(source.includes('data-view-mode'), 'render controls missing view layout selector');
  assert(source.includes('<option value="grid" selected>3+1 views</option>'), 'view layout selector missing 3+1 option');
  assert(source.includes('<option value="iso">Iso only</option>'), 'view layout selector missing Iso-only option');
  assert(source.includes('id="renderProfile"'), 'render profile output is missing');
  assert(source.includes('id="gcodeTools"'), 'G-code toolbar output is missing');
}
assert(toolUiCss.includes('@media (max-width: 700px)'), 'shared UI stylesheet missing mobile breakpoint');
assert(toolUiCss.includes('grid-template-columns: 1fr'), 'mobile nav should collapse to one column');
assert(toolUiCss.includes('touch-action: none'), 'viewport should reserve pointer gestures for toolpath controls');
assert(toolUiCss.includes('overscroll-behavior: contain'), 'viewport should contain wheel/scroll gestures');
assert(toolUiCss.includes('.playback .speeds'), 'shared UI stylesheet should compact speed controls into playback strip');
assert(toolUiCss.includes('grid-template-columns: auto auto minmax(80px, 1fr) auto'), 'mobile playback controls should stay compact');
assert(toolUiCss.includes('.metric-unit'), 'shared UI stylesheet should keep metric units after values');
assert(toolUiCss.includes('.render-controls'), 'shared UI stylesheet should style render layer toggles');
assert(toolUiCss.includes('.render-profile'), 'shared UI stylesheet should style render profile output');
assert(toolUiCss.includes('.view-grid-labels.is-iso-only'), 'shared UI stylesheet should hide multi-view grid in Iso-only mode');
assert(toolUiCss.includes('.reset-defaults'), 'shared UI stylesheet should style group default resets');
assert(toolUiCss.includes('.default-reset'), 'shared UI stylesheet should style per-control default resets');
assert(toolUiCss.includes('.g-token'), 'shared UI stylesheet should style G-code command/param tokens');
assert(toolUiCss.includes('.gcode-tools'), 'shared UI stylesheet should style G-code search/jump controls');
assert(toolUiCss.includes('.g-section'), 'shared UI stylesheet should style G-code layer section headers');
assert(toolUiCss.includes('.finding-count'), 'shared UI stylesheet should style grouped verifier counts');
assert(toolUiCss.includes('.finding-samples'), 'shared UI stylesheet should style grouped verifier examples');
assert(toolUiCss.includes('grid-template-columns: minmax(8ch, 1fr) minmax(8ch, auto) minmax(5ch, auto)'), 'metrics should use title/value/UOM columns');
assert(viewerJs.includes('getParams = () => params'), 'viewer should accept dynamic resolve parameter getter');
assert(viewerJs.includes('activeParams = getParams() || params'), 'viewer should resolve with current params');
assert(viewerJs.includes('params: activeParams'), 'viewer debug/export state should expose current params');
for (const id of ['macroHeader', 'macroHeat', 'macroHome', 'macroPrimeLine', 'macroPrimeBlob', 'macroPresent', 'macroCooldown', 'macroMotorsOff']) {
}
assert(designsJs.includes('DESIGN_DEFS'), 'gallery designs should be defined as metadata');
assert(designsJs.includes('materializeDesign'), 'gallery designs should materialize default ops from metadata');
assert(designsJs.includes('params:'), 'gallery designs missing parameter definitions');
assert(designsJs.includes('build:'), 'gallery designs missing build functions');
assert(toolUiCss.includes('.select-field'), 'shared UI stylesheet should compact select controls');
assert(toolUiCss.includes('.source-card'), 'shared UI stylesheet should style source cards');
assert(toolUiCss.includes('.design-card'), 'shared UI stylesheet should style design cards');
assert(toolUiCss.includes('.export-panel'), 'shared UI stylesheet should style export panel');
assert(toolUiCss.includes('.macro-grid'), 'shared UI stylesheet should style optional macros');
for (const id of [
  'latticeAlpha', 'latticeUnit', 'latticeCols', 'latticeRows',
  'latticeLayers', 'latticeLayerH', 'latticeBead', 'latticeSpeed',
]) {
}
assert(toolUiCss.includes('grid-template-columns: var(--source-w) 7px'), 'resizable app layout should expose source grid width');
assert(toolUiCss.includes('.panel-region.is-collapsed > :not(.panel-heading)'), 'collapsed panels should hide body content');
assert(toolUiCss.includes('.range-row input[type="range"]'), 'shared UI stylesheet missing lattice slider styling');
for (const id of [
  'tpmsCell', 'tpmsSamples', 'tpmsCellsX', 'tpmsCellsY', 'tpmsCellsZ',
  'tpmsLayerH', 'tpmsIso', 'tpmsBead', 'tpmsSpeed',
  'tpmsAdaptiveMin', 'tpmsAdaptiveMax',
]) {
}
assert(!designsJs.includes('pathMode'), 'designs.js should not reference the dropped TPMS pathMode option');
assert(!designsJs.includes('arcFit'), 'designs.js should not reference the dropped TPMS arcFit* options');
// The former tpms.js reimplementation is gone; TPMS Op generation now delegates to the Rust engine
// over wasm (tpms-engine.js), the same idiom sdk/ts/src/generators/tpms.ts uses. No behavioural
// coverage of its live output existed anywhere before this fix (web/smoke.cjs didn't call
// tpms_ops_json and no conformance vectors covered TPMS); this file runs before the wasm build in
// CI (see .github/workflows/ci.yml), so it only checks the delegation wiring statically here. Live
// TPMS output is now asserted in web/smoke.cjs against the built web/pkg-node/ module.
assert(tpmsEngineJs.includes("from './pkg/dry_wasm.js'"), 'TPMS generator should import the wasm engine binding');
assert(tpmsEngineJs.includes('tpms_ops_json'), 'TPMS generator should delegate Op generation to tpms_ops_json');
for (const surface of [
  'gyroid', 'schwarz-p', 'schwarz-d', 'iwp', 'neovius',
  'fischer-koch-s', 'fischer-koch-y', 'frd', 'lidinoid', 'split-p',
]) {
  assert(tpmsEngineJs.includes(`'${surface}'`) || tpmsEngineJs.includes(`${surface}:`), `TPMS generator missing surface metadata for ${surface}`);
}

for (const unsupported of ['controls_whileUntil', 'controls_forEach', 'procedures_defnoreturn', 'procedures_defreturn']) {
  assert(!blocksHtml.includes(`<block type="${unsupported}"`), `unsupported ${unsupported} leaked into toolbox`);
}

for (const [name, source] of [
  ['blocks.html', blocksHtml],
  ['viewer.js', viewerJs],
  ['index.html', indexHtml],
]) {
  assert(!source.includes('innerHTML'), `${name} still uses innerHTML`);
}

const templatesModule = await import(`data:text/javascript;base64,${Buffer.from(templatesJs).toString('base64')}`);
const patternsModule = await import(`data:text/javascript;base64,${Buffer.from(patternsJs).toString('base64')}`);
const fakeBlockly = {
  utils: { xml: { textToDom: (xml) => xml } },
  Xml: { domToWorkspace: (xml, workspace) => { workspace.xml = xml; } },
};
const countMatches = (source, pattern) => (source.match(pattern) || []).length;

let templateCount = 0;
for (const [key, template] of Object.entries(templatesModule.TEMPLATES)) {
  const workspace = { clear() { this.xml = ''; } };
  template.build(fakeBlockly, workspace);
  assert(workspace.xml && workspace.xml.includes('<xml'), `${key} did not produce Blockly XML`);
  assert(!workspace.xml.includes('dry_spline3'), `${key} still uses dry_spline3`);
  templateCount++;
  assert(!workspace.xml.includes('<field name="VAR">i</field>'), `${key} has a name-only i variable reference`);
  if (workspace.xml.includes('type="variables_get"')) {
    assert(workspace.xml.includes('<field name="VAR" id="varI">i</field>'), `${key} variables_get should bind to the dry_for variable id`);
  }

  if (key === 'spline_wave') {
    assert(workspace.xml.includes('type="dry_spline"'), 'spline_wave does not use dry_spline');
    assert(workspace.xml.includes('<mutation points="6"></mutation>'), 'spline_wave does not preserve 6 spline points');
    for (let i = 1; i <= 6; i++) {
      for (const axis of ['X', 'Y', 'Z']) {
        assert(workspace.xml.includes(`name="${axis}${i}"`), `spline_wave missing ${axis}${i}`);
      }
    }
  }

  if (key === 'layered_tower') {
    assert(workspace.xml.includes('<field name="ON">false</field>'), 'layered_tower must travel between layer starts');
    assert(countMatches(workspace.xml, /type="dry_move"/g) >= 5, 'layered_tower should include a closed perimeter body');
  }

  if (key === 'zigzag') {
    assert(template.label === 'Zig-zag infill panel', 'zigzag template label no longer matches its panel logic');
    assert(countMatches(workspace.xml, /type="dry_extruder"/g) >= 4, 'zigzag panel should switch between perimeter, travel, and infill');
    assert(workspace.xml.includes('<field name="N">10</field>'), 'zigzag panel should draw the remaining rail-to-rail segments after its travel move');
  }

  if (key === 'twisted_vase') {
    assert(template.label === 'Twisted vase (continuous vase mode)', 'twisted_vase label should describe continuous vase mode');
    assert(workspace.xml.includes('type="dry_vase_helix"'), 'twisted_vase should use the compact vase helix block');
    assert(workspace.xml.includes('<field name="TURNS">16</field>'), 'twisted_vase should use 16 turns');
    assert(workspace.xml.includes('<field name="SAMPLES">60</field>'), 'twisted_vase should use 60 samples per turn');
    assert(workspace.xml.includes('<field name="HEIGHT">48</field>'), 'twisted_vase should rise over a tall vase-height Z span');
    assert(workspace.xml.includes('<field name="FLUTES">8</field>'), 'twisted_vase should keep radial flutes');
    assert(!workspace.xml.includes('<field name="N">960</field>'), 'twisted_vase should not expose the old unreadable 960-iteration formula loop');
    assert(!workspace.xml.includes('type="math_trig"'), 'twisted_vase should not use Blockly degree-based trig for radian formulas');
  }
}

assert(templateCount === 9, `expected 9 Blockly templates, got ${templateCount}`);

const { ops: vaseOps, rawSteps: vaseRawSteps, steps: vaseSteps } = patternsModule.vaseHelixOps();
const vasePoints = vaseOps.filter((op) => op.op === 'move');
const vaseRadii = vasePoints.map((p) => Math.hypot(p.x - 50, p.y - 50));
let vasePrevAngle = null, vaseTotalAngle = 0;
for (const p of vasePoints) {
  const angle = Math.atan2(p.y - 50, p.x - 50);
  if (vasePrevAngle != null) {
    let delta = angle - vasePrevAngle;
    while (delta > Math.PI) delta -= Math.PI * 2;
    while (delta < -Math.PI) delta += Math.PI * 2;
    vaseTotalAngle += delta;
  }
  vasePrevAngle = angle;
}
const vaseZ = vasePoints.map((p) => p.z);
assert(vaseOps.length === 963, `vaseHelixOps should emit 963 ops, got ${vaseOps.length}`);
assert(vaseRawSteps === 960 && vaseSteps === 960, 'vaseHelixOps should emit 16 turns with 60 samples per turn');
assert(Math.max(...vaseZ) - Math.min(...vaseZ) === 48, 'vaseHelixOps should span 48 mm in Z');
assert(Math.abs((vaseTotalAngle / (Math.PI * 2)) - 16) < 1e-9, 'vaseHelixOps should complete 16 turns');
assert(Math.min(...vaseRadii) < 8.3 && Math.max(...vaseRadii) > 17, 'vaseHelixOps should retain the fluted belly profile');

// TPMS Op generation itself (printable defaults, adaptive slicing, resolution-budget rejection) is
// no longer JS the way `patternsModule`/`templatesModule` above are: `tpms-engine.js` delegates to
// the wasm engine (`tpms_ops_json`), which needs the built `web/pkg/` module. This file runs before
// that build in CI (see .github/workflows/ci.yml), so it only checks delegation wiring statically
// (above), and does not exercise live TPMS output itself. Live output is now covered by both the
// engine's own Rust test suite and behavioural assertions in `web/smoke.cjs` against the built
// `web/pkg-node/` module.
console.log(`Blockly regression checks passed (${templateCount} templates)`);
