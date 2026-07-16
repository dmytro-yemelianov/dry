#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(webRoot, '..');
const fixturesRoot = path.join(repoRoot, 'conformance/gallery');
const outputFile = path.join(webRoot, 'fullcontrol-gallery.generated.js');
const upstreamCommit = '9a90c40d62d88a32a5752c7f337af3174d7dfc13';

const notebook = (name) =>
  `https://github.com/FullControlXYZ/fullcontrol/blob/${upstreamCommit}/models/${name}.ipynb`;
const website = (id) => `https://fullcontrol.xyz/#/models/${id}`;

const metadata = {
  nonplanar_spacer: {
    label: 'Nonplanar Spacer',
    group: 'FullControl notebooks (Dry ports)',
    description: 'A wavy-Z functional spacer reconstructed as executable Dry L1 operations.',
    tags: ['notebook', 'fullcontrol.xyz', 'non-planar'],
    links: [
      ['Original notebook', notebook('nonplanar_spacer')],
      ['fullcontrol.xyz', website('971ff7')],
    ],
  },
  hex_adapter: {
    label: 'Hex Adapter',
    group: 'FullControl notebooks (Dry ports)',
    description: 'A continuous-path lattice hex adapter reconstructed in Dry.',
    tags: ['notebook', 'fullcontrol.xyz', 'lattice'],
    links: [
      ['Original notebook', notebook('hex_adapter')],
      ['fullcontrol.xyz', website('ff1d4e')],
    ],
  },
  fractional_design_engine: {
    label: 'Fractional Design Engine (Polar)',
    group: 'FullControl notebooks (Dry ports)',
    description: 'A polar lattice example reconstructed from the published default design.',
    tags: ['notebook', 'fullcontrol.xyz', 'polar'],
    links: [
      ['Original notebook', notebook('fractional_design_engine_polar')],
      ['fullcontrol.xyz', website('a72616')],
    ],
  },
  ripple_vase: {
    label: 'Ripple Texture Demo',
    group: 'FullControl notebooks (Dry ports)',
    description: 'Offset rippled paths reconstructed as a Dry toolpath.',
    tags: ['notebook', 'fullcontrol.xyz', 'texture'],
    links: [
      ['Original notebook', notebook('ripple_texture')],
      ['fullcontrol.xyz', website('4a0397')],
    ],
  },
  phone_stand: {
    label: 'AnyAngle Phone Stand',
    group: 'FullControl notebooks (Dry ports)',
    description: 'The published lattice phone stand default reconstructed in Dry.',
    tags: ['notebook', 'fullcontrol.xyz', 'lattice'],
    links: [
      ['Original notebook', notebook('anyangle_phone_stand')],
      ['fullcontrol.xyz', website('4d0e78')],
    ],
  },
  blob_printing: {
    label: 'Blob Printing',
    group: 'FullControl notebooks (Dry ports)',
    description: 'A stationary-deposition example that prints with blobs instead of lines.',
    tags: ['notebook', 'fullcontrol.xyz', 'deposit'],
    links: [
      ['Original notebook', notebook('blob_printing')],
      ['fullcontrol.xyz', website('800020')],
    ],
  },
  nuts_and_bolts: {
    label: 'Nuts and Bolts',
    group: 'FullControl notebooks (Dry ports)',
    description: 'The published threaded-part default reconstructed as Dry L1 operations.',
    tags: ['notebook', 'fullcontrol.xyz', 'thread'],
    links: [
      ['Original notebook', notebook('nuts_and_bolts')],
      ['fullcontrol.xyz', website('393a4c')],
    ],
  },
  star_polygon_lattice: {
    label: 'Star-Polygon Lattice Research',
    group: 'FullControl notebooks (Dry ports)',
    description: 'The published star-polygon lattice default reconstructed in Dry.',
    tags: ['notebook', 'fullcontrol.xyz', 'research', 'lattice'],
    links: [
      ['Original notebook', notebook('star_polygon_lattice')],
      ['fullcontrol.xyz', website('1d3528')],
    ],
  },
  tape_reinforcement: {
    label: 'Tape-Reinforcement Research Demo',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The published TPU tape-reinforcement default reconstructed in Dry.',
    tags: ['fullcontrol.xyz', 'research', 'lattice'],
    links: [['fullcontrol.xyz', website('eac87f')]],
  },
  overhang_challenge: {
    label: 'Overhang Challenge',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The base 90-degree overhang challenge reconstructed in Dry.',
    tags: ['fullcontrol.xyz', 'overhang'],
    links: [['fullcontrol.xyz', website('b70938')]],
  },
  overhang_challenge_plus: {
    label: 'Overhang Challenge Plus',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The distinct Plus variant with its published default shape and direction.',
    tags: ['fullcontrol.xyz', 'overhang', 'plus'],
    links: [['fullcontrol.xyz', website('2d37a5')]],
  },
  retraction_test: {
    label: '2000-Retractions Test',
    group: 'FullControl.xyz (Dry ports)',
    description: 'A compact oracle-backed reconstruction of the published retraction stress test.',
    tags: ['fullcontrol.xyz', 'retraction', 'test'],
    links: [['fullcontrol.xyz', website('3bfcdb')]],
  },
  pin_support_challenge: {
    label: 'Pin-Support Challenge',
    group: 'FullControl.xyz (Dry ports)',
    description: 'A continuous-Z pin and supported top feature reconstructed in Dry.',
    tags: ['fullcontrol.xyz', 'continuous Z'],
    links: [['fullcontrol.xyz', website('67cf20')]],
  },
  snake_soapdish: {
    label: 'Snake-Mode Soapdish',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The open-structure snake-mode soapdish reconstructed in Dry.',
    tags: ['fullcontrol.xyz', 'snake mode', 'continuous Z'],
    links: [['fullcontrol.xyz', website('7d3dc2')]],
  },
  lampshade: {
    label: 'FullControl Lampshade',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The mathematical lampshade default reconstructed in Dry from the public example.',
    tags: ['fullcontrol.xyz', 'gist', 'parametric'],
    links: [
      ['Public notebook gist', 'https://gist.github.com/fullcontrol-xyz/589c78de0093698a07ec724af6428f09'],
      ['fullcontrol.xyz', website('ebdc86')],
    ],
  },
  freeform_frosting: {
    label: 'Freeform Frosting Challenge',
    group: 'FullControl.xyz (Dry ports)',
    description: 'The published freeform helical default reconstructed in Dry.',
    tags: ['fullcontrol.xyz', 'helix', 'texture'],
    links: [['fullcontrol.xyz', website('c5042e')]],
  },
  arc_vase: {
    label: 'Arc Vase', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed vase design using native arc moves.', tags: ['oracle gallery', 'arc', 'vase'], links: [],
  },
  bead_studs: {
    label: 'Bead Studs', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed bead-stud deposition design.', tags: ['oracle gallery', 'deposit'], links: [],
  },
  brush_lettering: {
    label: 'Brush Lettering', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed freeform lettering toolpath.', tags: ['oracle gallery', 'lettering'], links: [],
  },
  gyroid_infill: {
    label: 'Gyroid Infill', group: 'FullControl gallery (Dry ports)',
    description: 'A compact oracle-backed gyroid infill reconstruction.', tags: ['oracle gallery', 'gyroid', 'infill'], links: [],
  },
  helical_screw: {
    label: 'Helical Screw', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed helical screw toolpath.', tags: ['oracle gallery', 'helix', 'thread'], links: [],
  },
  mobius_band: {
    label: 'Möbius Band', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed non-planar Möbius-band path.', tags: ['oracle gallery', 'non-planar'], links: [],
  },
  spiral_vase: {
    label: 'Spiral Vase', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed continuous spiral-vase reconstruction.', tags: ['oracle gallery', 'vase', 'continuous Z'], links: [],
  },
  textured_cone: {
    label: 'Textured Cone', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed textured cone toolpath.', tags: ['oracle gallery', 'texture', 'cone'], links: [],
  },
  towers_grid: {
    label: 'Towers Grid', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed grid of printed towers.', tags: ['oracle gallery', 'towers'], links: [],
  },
  trefoil_tube: {
    label: 'Trefoil Tube', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed trefoil tube path.', tags: ['oracle gallery', 'tube', 'non-planar'], links: [],
  },
  twisted_polygon_vase: {
    label: 'Twisted Polygon Vase', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed twisted polygon vase.', tags: ['oracle gallery', 'vase', 'twist'], links: [],
  },
  wave_bowl: {
    label: 'Wave Bowl', group: 'FullControl gallery (Dry ports)',
    description: 'An oracle-backed wave-textured bowl path.', tags: ['oracle gallery', 'bowl', 'texture'], links: [],
  },
};

function fixtureLink(name) {
  return `https://github.com/dmytro-yemelianov/dry/blob/main/conformance/gallery/${name}.json`;
}

function generatedSource() {
  const fixtureNamesSorted = fs.readdirSync(fixturesRoot)
    .filter((name) => name.endsWith('.json'))
    .map((name) => name.replace(/\.json$/, ''))
    .sort();
  const metadataNames = Object.keys(metadata).sort();
  if (JSON.stringify(fixtureNamesSorted) !== JSON.stringify(metadataNames)) {
    throw new Error(`FullControl metadata/fixture mismatch:\nfixtures=${fixtureNamesSorted.join(',')}\nmetadata=${metadataNames.join(',')}`);
  }

  const designs = {};
  for (const name of Object.keys(metadata)) {
    const fixture = JSON.parse(fs.readFileSync(path.join(fixturesRoot, `${name}.json`), 'utf8'));
    if (fixture.design !== name || !Array.isArray(fixture.l1?.ops) || fixture.l1.ops.length === 0) {
      throw new Error(`Invalid gallery fixture: ${name}`);
    }
    const item = metadata[name];
    designs[name] = {
      ...item,
      tags: ['Dry L1', 'oracle-backed', ...item.tags],
      links: [...item.links, ['Dry fixture', fixtureLink(name)]],
      fixture: name,
      params: [],
      defaults: {},
      ops: fixture.l1.ops,
    };
  }

  return [
    '// Generated by web/generate-fullcontrol-gallery.mjs. Do not edit by hand.',
    '// Contains only the committed Dry L1 operation reconstructions, not FullControl source.',
    `const FULLCONTROL_DESIGNS = ${JSON.stringify(designs)};`,
    '',
    'export { FULLCONTROL_DESIGNS };',
    '',
  ].join('\n');
}

const expected = generatedSource();
if (process.argv.includes('--validate')) {
  console.log(`validated FullControl gallery inputs (${Object.keys(metadata).length} designs)`);
} else if (process.argv.includes('--check')) {
  const actual = fs.existsSync(outputFile) ? fs.readFileSync(outputFile, 'utf8') : '';
  if (actual !== expected) {
    console.error('web/fullcontrol-gallery.generated.js is stale; run node web/generate-fullcontrol-gallery.mjs');
    process.exit(1);
  }
  console.log(`FullControl gallery module is current (${Object.keys(metadata).length} designs)`);
} else {
  fs.writeFileSync(outputFile, expected, 'utf8');
  console.log(`generated ${path.relative(repoRoot, outputFile)} (${Object.keys(metadata).length} designs)`);
}
