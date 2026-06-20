import fs from 'node:fs';

function read(path) {
  return fs.readFileSync(new URL(path, import.meta.url), 'utf8');
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const blocksHtml = read('blocks.html');
const templatesJs = read('templates.js');
const viewerJs = read('viewer.js');
const indexHtml = read('index.html');

assert(blocksHtml.includes('Blockly.Blocks.dry_spline'), 'dynamic dry_spline block is missing');
assert(blocksHtml.includes('mutationToDom()'), 'dry_spline XML mutation writer is missing');
assert(blocksHtml.includes('domToMutation(xmlElement)'), 'dry_spline XML mutation reader is missing');
assert(blocksHtml.includes('saveExtraState()'), 'dry_spline JSON state writer is missing');
assert(blocksHtml.includes('loadExtraState(state)'), 'dry_spline JSON state reader is missing');
assert(!blocksHtml.includes('dry_spline3'), 'fixed dry_spline3 block leaked into blocks.html');
assert(!templatesJs.includes('dry_spline3'), 'fixed dry_spline3 block leaked into templates.js');

for (const category of ['Dry setup', 'Dry motion', 'Dry process', 'Flow', 'Logic', 'Math', 'Lists', 'Variables']) {
  assert(blocksHtml.includes(`<category name="${category}"`), `toolbox missing ${category} category`);
}

for (const type of ['controls_repeat_ext', 'controls_for', 'math_change']) {
  assert(blocksHtml.includes(`<block type="${type}"`), `toolbox missing ${type}`);
  assert(blocksHtml.includes(`case '${type}'`), `generator missing ${type}`);
}

assert(blocksHtml.includes('JS().definitions_'), 'expression evaluator ignores Blockly helper definitions');
assert(blocksHtml.includes("key !== 'variables'"), 'expression evaluator should not inject variable declarations as helpers');

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
    assert(workspace.xml.includes('<field name="N">960</field>'), 'twisted_vase should use 40 turns with 24 samples per turn');
    assert(workspace.xml.includes('<field name="NUM">16</field>'), 'twisted_vase should rise over a vase-height Z span');
    assert(countMatches(workspace.xml, /<field name="OP">DIVIDE<\/field>/g) >= 5, 'twisted_vase should scale height/profile/twist by i / step count');
    assert(countMatches(workspace.xml, /<field name="OP">SIN<\/field>/g) >= 2, 'twisted_vase should include a body profile, not just a coil');
    assert(countMatches(workspace.xml, /<field name="OP">COS<\/field>/g) >= 3, 'twisted_vase should include radial flutes plus polar projection');
  }
}

assert(templateCount === 9, `expected 9 Blockly templates, got ${templateCount}`);
console.log(`Blockly regression checks passed (${templateCount} templates)`);
