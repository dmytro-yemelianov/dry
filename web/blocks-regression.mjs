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
const viewerJs = read('viewer.js');
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

assert(blocksHtml.includes('JS().definitions_'), 'expression evaluator ignores Blockly helper definitions');
assert(blocksHtml.includes("key !== 'variables'"), 'expression evaluator should not inject variable declarations as helpers');
assert(viewerJs.includes('VIEW_PANELS'), 'viewer multi-view panel definitions are missing');
assert(viewerJs.includes('renderViews'), 'viewer multi-view renderer is missing');
assert(viewerJs.includes('cameraRect'), 'viewer split viewport geometry is missing');
assert(viewerJs.includes("key: 'front'"), 'viewer should include a front XZ view');
assert(viewerJs.includes("new OrbitControls(cameras.get('iso'), el)"), 'OrbitControls should bind to the full viewport element');
assert(viewerJs.includes('resetViewEl'), 'viewer should expose a reset view control');
assert(viewerJs.includes('__viewerDebug'), 'viewer should expose debug state for interaction checks');
assert(viewerJs.includes("addEventListener('wheel'"), 'viewer should claim wheel events before scroll containers');
assert(viewerJs.includes('inputStats'), 'viewer should expose interaction counters for viewport debugging');
assert(viewerJs.includes('uGhostAlpha'), 'viewer should render unprinted bead geometry with partial transparency');
assert(viewerJs.includes('transparent: true'), 'viewer bead material should support printed/planned alpha');
assert(viewerJs.includes('keepLineVisible'), 'viewer should scroll only the g-code panel for active lines');
assert(!viewerJs.includes('scrollIntoView'), 'active g-code line should not scroll parent layout panels');
assert(viewerJs.includes('view-grid-labels'), 'viewer does not render multi-view labels');
assert(blocksHtml.includes('.view-grid-labels'), 'blocks page missing multi-view styles');
assert(indexHtml.includes('.view-grid-labels'), 'gallery page missing multi-view styles');
assert(indexHtml.includes('tool-ui.css'), 'gallery page does not load shared UI stylesheet');
assert(blocksHtml.includes('tool-ui.css'), 'blocks page does not load shared UI stylesheet');
assert(indexHtml.includes('class="topbar"'), 'gallery page missing flex topbar');
assert(blocksHtml.includes('class="topbar"'), 'blocks page missing flex topbar');
assert(indexHtml.includes('id="resetView"'), 'gallery page missing reset view control');
assert(blocksHtml.includes('id="resetView"'), 'blocks page missing reset view control');
assert(blocksHtml.includes('id="fitBlocks"'), 'blocks page missing fit workspace control');
assert(blocksHtml.includes('id="cleanBlocks"'), 'blocks page missing clean workspace control');
assert(blocksHtml.includes('zoomToFit'), 'blocks page should fit loaded templates into view');
assert(indexHtml.includes('<span><span class="swatch planned"></span>planned</span>'), 'gallery legend should expose planned/unprinted geometry');
assert(blocksHtml.includes('<span><span class="swatch planned"></span>planned</span>'), 'blocks legend should expose planned/unprinted geometry');
assert(toolUiCss.includes('@media (max-width: 700px)'), 'shared UI stylesheet missing mobile breakpoint');
assert(toolUiCss.includes('grid-template-columns: 1fr'), 'mobile nav should collapse to one column');
assert(toolUiCss.includes('touch-action: none'), 'viewport should reserve pointer gestures for toolpath controls');
assert(toolUiCss.includes('overscroll-behavior: contain'), 'viewport should contain wheel/scroll gestures');
assert(indexHtml.includes('id="source"'), 'web app missing source selector');
assert(indexHtml.includes('value="lattice"'), 'web app missing lattice generator source');
assert(indexHtml.includes('value="tpms"'), 'web app missing TPMS generator source');
assert(indexHtml.includes('starPolygonLatticeOps'), 'web app does not generate star-polygon lattice ops');
assert(indexHtml.includes('tpmsOps'), 'web app does not generate TPMS ops');
assert(indexHtml.includes('id="latticeAlpha"'), 'lattice generator missing alpha control');
assert(indexHtml.includes('id="tpmsSurface"'), 'TPMS generator missing surface control');
assert(indexHtml.includes('id="tpmsPerimeter"'), 'TPMS generator missing infill perimeter toggle');

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
console.log(`Blockly regression checks passed (${templateCount} templates)`);
