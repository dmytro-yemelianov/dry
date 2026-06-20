// Blockly starter templates for the Dry authoring page (blocks.html).
//
// Each template is `key -> { label, group, tags, build(Blockly, workspace) }`. `build` clears the
// workspace and loads a ready-made block design — a real Dry L1 design expressed as blocks. The
// parametric ones use the new `dry_for` loop plus native math/variables blocks (cos/sin of i, etc.)
// so the generator produces finite, non-empty g-code. They are authored as XML strings (Blockly's
// classic serialization) and loaded with Blockly.Xml.domToWorkspace.
//
// XML helpers below keep the templates readable: `num`/`expr` build value inputs (shadow vs real
// math), `varRef`/`arith`/`trig`/`pi` build expression sub-trees, and `move`/`arc`/`spline`
// build statement blocks. `chain(...)` links statements via <next>. `program(...)` wraps a top-level
// chain at a workspace position.

// ---- value-input helpers (go inside a <value name="..."> ... </value>) ----
const TAU = Math.PI * 2;
const num = (n) => `<shadow type="math_number"><field name="NUM">${n}</field></shadow>`;
const varRef = (v = 'i') => `<block type="variables_get"><field name="VAR">${v}</field></block>`;
const pi = () => `<block type="math_constant"><field name="CONSTANT">PI</field></block>`;
const arith = (op, a, b) =>
  `<block type="math_arithmetic"><field name="OP">${op}</field>` +
  `<value name="A">${a}</value><value name="B">${b}</value></block>`;
const add = (a, b) => arith('ADD', a, b);
const mul = (a, b) => arith('MULTIPLY', a, b);
const div = (a, b) => arith('DIVIDE', a, b);
const idx1 = () => add(varRef('i'), num(1));
const trig = (op, x) => // SIN | COS | TAN
  `<block type="math_trig"><field name="OP">${op}</field><value name="NUM">${x}</value></block>`;
const cos = (x) => trig('COS', x);
const sin = (x) => trig('SIN', x);

const val = (name, inner) => `<value name="${name}">${inner}</value>`;

// ---- statement-block helpers ----
const geometry = (w = 0.6, h = 0.2) =>
  `<block type="dry_geometry"><field name="W">${w}</field><field name="H">${h}</field>{next}</block>`;
const extruder = (on = true) =>
  `<block type="dry_extruder"><field name="ON">${on}</field>{next}</block>`;
const speed = (v = 1000) => `<block type="dry_speed"><field name="PRINT">${v}</field>{next}</block>`;
const temperature = (c) => `<block type="dry_temperature"><field name="NOZZLE">${c}</field>{next}</block>`;
// move/arc: x,y,z (and cx,cy) are value-input HTML fragments (use num(...) or an expression block)
const move = (x, y, z) =>
  `<block type="dry_move">${val('X', x)}${val('Y', y)}${val('Z', z)}{next}</block>`;
const arc = (cx, cy, x, y, z, cw = true) =>
  `<block type="dry_arc"><field name="CW">${cw ? 'true' : 'false'}</field>` +
  `${val('CX', cx)}${val('CY', cy)}${val('X', x)}${val('Y', y)}${val('Z', z)}{next}</block>`;
const spline = (...points) =>
  `<block type="dry_spline"><mutation points="${points.length}"></mutation>` +
  `<field name="POINTS">${points.length}</field>` +
  points.map(([x, y, z], i) =>
    `${val(`X${i + 1}`, x)}${val(`Y${i + 1}`, y)}${val(`Z${i + 1}`, z)}`).join('') +
  `{next}</block>`;
const forI = (n, bodyChain) =>
  `<block type="dry_for"><field name="VAR" id="varI">i</field><field name="N">${n}</field>` +
  `<statement name="DO">${bodyChain}</statement>{next}</block>`;

// Link a list of statement-fragment strings (each containing a single `{next}` slot) into a chain.
function chain(...frags) {
  const parts = frags.filter(Boolean);
  let xml = '';
  for (let i = parts.length - 1; i >= 0; i--) {
    xml = parts[i].replace('{next}', xml ? `<next>${xml}</next>` : '');
  }
  return xml;
}

function program(...frags) {
  const inner = chain(...frags).replace(/^<block /, '<block x="30" y="30" ');
  return `<xml xmlns="https://developers.google.com/blockly/xml"><variables>` +
    `<variable id="varI">i</variable></variables>${inner}</xml>`;
}

function loader(xml) {
  return (Blockly, workspace) => {
    workspace.clear();
    const dom = Blockly.utils.xml.textToDom(xml);
    Blockly.Xml.domToWorkspace(dom, workspace);
  };
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

// 1. Square — one closed perimeter; travel to the start, then extrude the loop.
const squareXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(0), num(0), num(0.2)), extruder(true),
  move(num(10), num(0), num(0.2)),
  move(num(10), num(10), num(0.2)), move(num(0), num(10), num(0.2)),
  move(num(0), num(0), num(0.2)),
);

// 2. Regular polygon — travel to vertex 0, then extrude vertices 1..N to close the perimeter.
// theta = (i + 1) * (2π / sides). x = 50 + 20*cos(theta); y = 50 + 20*sin(theta); z = 0.2.
const POLY_N = 6;
const theta = () => mul(idx1(), num(TAU / POLY_N));
const polyXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(70), num(50), num(0.2)), extruder(true),
  forI(POLY_N, chain(
    move(
      add(num(50), mul(num(20), cos(theta()))),
      add(num(50), mul(num(20), sin(theta()))),
      num(0.2),
    ),
  )),
);

// 3. Spiral — radius grows with i: r = 3 + 0.6*i, angle = 0.5*i.
const spiralR = () => add(num(3), mul(num(0.6), idx1()));
const spiralA = () => mul(num(0.5), idx1());
const spiralXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(53), num(50), num(0.2)), extruder(true),
  forI(72, chain(
    move(
      add(num(50), mul(spiralR(), cos(spiralA()))),
      add(num(50), mul(spiralR(), sin(spiralA()))),
      num(0.2),
    ),
  )),
);

// 4. Star — 10 alternating outer/inner radius points (literal coords, continuous stroke).
function starMoves() {
  const cx = 50, cy = 50, outer = 20, inner = 8, pts = 5, z = 0.2;
  const verts = [];
  for (let i = 0; i <= pts * 2; i++) {
    const r = i % 2 === 0 ? outer : inner;
    const a = (i / (pts * 2)) * Math.PI * 2 - Math.PI / 2;
    verts.push(move(num(+(cx + r * Math.cos(a)).toFixed(4)), num(+(cy + r * Math.sin(a)).toFixed(4)), num(z)));
  }
  return verts;
}
const starPath = starMoves();
const starXml = program(geometry(0.6, 0.2), extruder(false), starPath[0], extruder(true), ...starPath.slice(1));

// 5. Layered tower — each layer travels to its start, then extrudes one square perimeter.
function towerLayer() {
  // each move needs its own z expression instance (XML can't share nodes)
  const zEx = () => add(num(0.2), mul(num(0.3), varRef('i')));
  return chain(
    extruder(false), move(num(40), num(40), zEx()), extruder(true),
    move(num(60), num(40), zEx()),
    move(num(60), num(60), zEx()), move(num(40), num(60), zEx()),
    move(num(40), num(40), zEx()),
  );
}
const towerXml = program(
  geometry(0.6, 0.2), speed(1200),
  forI(10, towerLayer()),
);

// 6. Zig-zag infill panel — one perimeter, travel to the infill, then a serpentine fill path.
const N_ZIG_RAILS = 11;
const N_ZIG_SEGMENTS = N_ZIG_RAILS - 1;
const zigX = () => add(num(34), mul(num(3.2), idx1()));
const zigY = () => add(num(45), mul(num(11), cos(mul(pi(), idx1()))));
const zigzagXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(30), num(30), num(0.2)), extruder(true),
  move(num(70), num(30), num(0.2)), move(num(70), num(60), num(0.2)),
  move(num(30), num(60), num(0.2)), move(num(30), num(30), num(0.2)),
  extruder(false), move(num(34), num(56), num(0.2)), extruder(true),
  forI(N_ZIG_SEGMENTS, chain(move(zigX(), zigY(), num(0.2)))),
);

// 7. Twisted vase — continuous vase-mode spiral with a belly, narrow rim, and twisted flutes.
const VASE_TURNS = 40;
const VASE_STEPS_PER_TURN = 24;
const VASE_N = VASE_TURNS * VASE_STEPS_PER_TURN;
const VASE_LOBES = 6;
const VASE_TWIST = 1.25;
const vaseF = () => div(idx1(), num(VASE_N));
const vaseAngle = () => mul(idx1(), num(TAU / VASE_STEPS_PER_TURN));
const vaseTwist = () => mul(num(TAU * VASE_TWIST), vaseF());
const vaseProfile = () => add(
  add(num(11.2), mul(num(3.2), sin(mul(pi(), vaseF())))),
  mul(num(-1.4), vaseF()),
);
const vaseFlute = () => add(
  num(1),
  mul(num(0.08), cos(mul(num(VASE_LOBES), add(vaseAngle(), mul(num(-1), vaseTwist()))))),
);
const vaseR = () => mul(vaseProfile(), vaseFlute());
const vaseZ = () => add(num(0.2), mul(num(16), vaseF()));
const twistedVaseXml = program(
  geometry(0.6, 0.2), temperature(210), extruder(false),
  move(num(62.1), num(50), num(0.2)), extruder(true),
  forI(VASE_N, chain(
    move(
      add(num(50), mul(vaseR(), cos(vaseAngle()))),
      add(num(50), mul(vaseR(), sin(vaseAngle()))),
      vaseZ(),
    ),
  )),
);

// 8. Rounded square — four straight edges joined by four CCW corner arcs (lines + arcs).
const roundedXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(45), num(38), num(0.4)), extruder(true),
  move(num(55), num(38), num(0.4)),
  arc(num(55), num(43), num(60), num(43), num(0.4), false),
  move(num(60), num(57), num(0.4)),
  arc(num(55), num(57), num(55), num(62), num(0.4), false),
  move(num(45), num(62), num(0.4)),
  arc(num(45), num(57), num(40), num(57), num(0.4), false),
  move(num(40), num(43), num(0.4)),
  arc(num(45), num(43), num(45), num(38), num(0.4), false),
);

// 9. S-curve — one native Catmull-Rom spline op with a six-point control list.
const splineWaveXml = program(
  geometry(0.6, 0.2), extruder(false),
  move(num(20), num(50), num(0.2)), extruder(true),
  spline(
    [num(32), num(68), num(0.2)],
    [num(48), num(32), num(0.2)],
    [num(64), num(50), num(0.2)],
    [num(76), num(68), num(0.2)],
    [num(92), num(32), num(0.2)],
    [num(104), num(50), num(0.2)],
  ),
);

export const TEMPLATES = {
  square: { label: 'Square', group: 'Basics', tags: ['line', 'perimeter'], build: loader(squareXml) },
  polygon: { label: 'Regular polygon (for + cos/sin)', group: 'Basics', tags: ['parametric', 'line'], build: loader(polyXml) },
  star: { label: 'Star (continuous stroke)', group: 'Basics', tags: ['line'], build: loader(starXml) },
  rounded_square: { label: 'Rounded square (lines + arcs)', group: 'Curves', tags: ['arc', 'line'], build: loader(roundedXml) },
  spline_wave: { label: 'S-curve (native spline)', group: 'Curves', tags: ['spline', 'curve'], build: loader(splineWaveXml) },
  spiral: { label: 'Spiral (radius grows with i)', group: 'Curves', tags: ['parametric'], build: loader(spiralXml) },
  zigzag: { label: 'Zig-zag infill panel', group: 'Infill & multi-layer', tags: ['infill', 'parametric'], build: loader(zigzagXml) },
  layered_tower: { label: 'Layered tower (z = 0.2 + i·0.3)', group: 'Infill & multi-layer', tags: ['multi-layer', 'parametric'], build: loader(towerXml) },
  twisted_vase: { label: 'Twisted vase (continuous vase mode)', group: 'Vases & non-planar', tags: ['vase', 'non-planar', '3D', 'parametric'], build: loader(twistedVaseXml) },
};
