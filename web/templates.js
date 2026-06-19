// Blockly starter templates for the Dry authoring page (blocks.html).
//
// Each template is `key -> { label, group, tags, build(Blockly, workspace) }`. `build` clears the
// workspace and loads a ready-made block design — a real Dry L1 design expressed as blocks. The
// parametric ones use the new `dry_for` loop plus native math/variables blocks (cos/sin of i, etc.)
// so the generator produces finite, non-empty g-code. They are authored as XML strings (Blockly's
// classic serialization) and loaded with Blockly.Xml.domToWorkspace.
//
// XML helpers below keep the templates readable: `num`/`expr` build value inputs (shadow vs real
// math), `varRef`/`arith`/`trig`/`unary`/`pi` build expression sub-trees, and `move`/`arc`/etc.
// build statement blocks. `chain(...)` links statements via <next>. `program(...)` wraps a top-level
// chain at a workspace position.

// ---- value-input helpers (go inside a <value name="..."> ... </value>) ----
const num = (n) => `<shadow type="math_number"><field name="NUM">${n}</field></shadow>`;
const varRef = (v = 'i') => `<block type="variables_get"><field name="VAR">${v}</field></block>`;
const pi = () => `<block type="math_constant"><field name="CONSTANT">PI</field></block>`;
const arith = (op, a, b) =>
  `<block type="math_arithmetic"><field name="OP">${op}</field>` +
  `<value name="A">${a}</value><value name="B">${b}</value></block>`;
const trig = (op, x) => // SIN | COS | TAN
  `<block type="math_trig"><field name="OP">${op}</field><value name="NUM">${x}</value></block>`;
const unary = (op, x) => // NEG | ABS | ROOT | POW10 | ...
  `<block type="math_single"><field name="OP">${op}</field><value name="NUM">${x}</value></block>`;

const val = (name, inner) => `<value name="${name}">${inner}</value>`;

// ---- statement-block helpers ----
const geometry = (w = 0.6, h = 0.2) =>
  `<block type="dry_geometry"><field name="W">${w}</field><field name="H">${h}</field>{next}</block>`;
const extruder = (on = true) =>
  `<block type="dry_extruder"><field name="ON">${on}</field>{next}</block>`;
const speed = (v = 1000) => `<block type="dry_speed"><field name="PRINT">${v}</field>{next}</block>`;
const temperature = (c) => `<block type="dry_temperature"><field name="NOZZLE">${c}</field>{next}</block>`;
const fan = (s) => `<block type="dry_fan"><field name="SPEED">${s}</field>{next}</block>`;
// move/arc: x,y,z (and cx,cy) are value-input HTML fragments (use num(...) or an expression block)
const move = (x, y, z) =>
  `<block type="dry_move">${val('X', x)}${val('Y', y)}${val('Z', z)}{next}</block>`;
const arc = (cx, cy, x, y, z, cw = true) =>
  `<block type="dry_arc"><field name="CW">${cw ? 'true' : 'false'}</field>` +
  `${val('CX', cx)}${val('CY', cy)}${val('X', x)}${val('Y', y)}${val('Z', z)}{next}</block>`;
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

// 1. Square — five line moves (the classic starter, all literal numbers).
const squareXml = program(
  geometry(0.6, 0.2), extruder(true),
  move(num(0), num(0), num(0.2)), move(num(10), num(0), num(0.2)),
  move(num(10), num(10), num(0.2)), move(num(0), num(10), num(0.2)),
  move(num(0), num(0), num(0.2)),
);

// 2. Regular polygon — for i in 0..N: vertex at (cx + r·cos(2π i/N), cy + r·sin(2π i/N)).
// theta = i * (2π / sides). x = 50 + 20*cos(theta); y = 50 + 20*sin(theta); z = 0.2.
const POLY_N = 6;
const theta = () => arith('MULTIPLY', varRef('i'), num(2 * Math.PI / POLY_N));
const polyXml = program(
  geometry(0.6, 0.2), extruder(true),
  forI(POLY_N + 1, chain(
    move(
      arith('ADD', num(50), arith('MULTIPLY', num(20), trig('COS', theta()))),
      arith('ADD', num(50), arith('MULTIPLY', num(20), trig('SIN', theta()))),
      num(0.2),
    ),
  )),
);

// 3. Spiral — radius grows with i: r = 3 + 0.6*i, angle = 0.5*i.
const spiralXml = program(
  geometry(0.6, 0.2), extruder(true),
  forI(72, chain(
    move(
      arith('ADD', num(50), arith('MULTIPLY', arith('ADD', num(3), arith('MULTIPLY', num(0.6), varRef('i'))), trig('COS', arith('MULTIPLY', num(0.5), varRef('i'))))),
      arith('ADD', num(50), arith('MULTIPLY', arith('ADD', num(3), arith('MULTIPLY', num(0.6), varRef('i'))), trig('SIN', arith('MULTIPLY', num(0.5), varRef('i'))))),
      num(0.2),
    ),
  )),
);

// 4. Star — 10 alternating outer/inner radius points (literal coords, continuous stroke).
function starFrags() {
  const cx = 50, cy = 50, outer = 20, inner = 8, pts = 5, z = 0.2;
  const verts = [];
  for (let i = 0; i <= pts * 2; i++) {
    const r = i % 2 === 0 ? outer : inner;
    const a = (i / (pts * 2)) * Math.PI * 2 - Math.PI / 2;
    verts.push(move(num(+(cx + r * Math.cos(a)).toFixed(4)), num(+(cy + r * Math.sin(a)).toFixed(4)), num(z)));
  }
  return verts;
}
const starXml = program(geometry(0.6, 0.2), extruder(true), ...starFrags());

// 5. Layered tower — for each layer i: z = 0.2 + i*0.3; a unrolled square perimeter at that z.
//    The square corners are literal; only z is parametric (depends on i).
function towerLayer() {
  // each move needs its own z expression instance (XML can't share nodes)
  const zEx = () => arith('ADD', num(0.2), arith('MULTIPLY', num(0.3), varRef('i')));
  return chain(
    move(num(40), num(40), zEx()), move(num(60), num(40), zEx()),
    move(num(60), num(60), zEx()), move(num(40), num(60), zEx()),
    move(num(40), num(40), zEx()),
  );
}
const towerXml = program(
  geometry(0.6, 0.2), speed(1200), extruder(true),
  forI(10, towerLayer()),
);

// 6. Zig-zag infill — for i in 0..N: a vertical sweep at x = 30 + 4*i, alternating y by parity.
//    y = 30 if i even else 60 (using ((i % 2)) — expressed as i - 2*floor(i/2) is awkward, so use a
//    cosine parity trick: y = 45 + 15*cos(π*i) gives 60,30,60,30,…). x = 30 + 4*i, z = 0.2.
const N_ZIG = 12;
const zigX = () => arith('ADD', num(30), arith('MULTIPLY', num(4), varRef('i')));
const zigY = () => arith('ADD', num(45), arith('MULTIPLY', num(15), trig('COS', arith('MULTIPLY', pi(), varRef('i')))));
const zigzagXml = program(
  geometry(0.6, 0.2), extruder(true),
  forI(N_ZIG, chain(move(zigX(), zigY(), num(0.2)))),
);

// 7. Twisted vase — non-planar helix: for i in 0..N, angle = 0.35*i, r = 14 + 3*cos(5*angle),
//    rising z = 0.2 + 0.25*i. A genuinely 3D parametric surface stroke.
const vaseAngle = () => arith('MULTIPLY', num(0.35), varRef('i'));
const vaseR = () => arith('ADD', num(14), arith('MULTIPLY', num(3), trig('COS', arith('MULTIPLY', num(5), arith('MULTIPLY', num(0.35), varRef('i'))))));
const twistedVaseXml = program(
  geometry(0.6, 0.2), temperature(210), extruder(true),
  forI(120, chain(
    move(
      arith('ADD', num(50), arith('MULTIPLY', vaseR(), trig('COS', vaseAngle()))),
      arith('ADD', num(50), arith('MULTIPLY', vaseR(), trig('SIN', vaseAngle()))),
      arith('ADD', num(0.2), arith('MULTIPLY', num(0.25), varRef('i'))),
    ),
  )),
);

// 8. Rounded square — four straight edges joined by four CCW corner arcs (lines + arcs).
const roundedXml = program(
  geometry(0.6, 0.2), extruder(true),
  move(num(45), num(38), num(0.4)), move(num(55), num(38), num(0.4)),
  arc(num(55), num(43), num(60), num(43), num(0.4), false),
  move(num(60), num(57), num(0.4)),
  arc(num(55), num(57), num(55), num(62), num(0.4), false),
  move(num(45), num(62), num(0.4)),
  arc(num(45), num(57), num(40), num(57), num(0.4), false),
  move(num(40), num(43), num(0.4)),
  arc(num(45), num(43), num(45), num(38), num(0.4), false),
);

export const TEMPLATES = {
  square: { label: 'Square', group: 'Basics', tags: ['line', 'perimeter'], build: loader(squareXml) },
  polygon: { label: 'Regular polygon (for + cos/sin)', group: 'Basics', tags: ['parametric', 'line'], build: loader(polyXml) },
  star: { label: 'Star (continuous stroke)', group: 'Basics', tags: ['line'], build: loader(starXml) },
  rounded_square: { label: 'Rounded square (lines + arcs)', group: 'Curves', tags: ['arc', 'line'], build: loader(roundedXml) },
  spiral: { label: 'Spiral (radius grows with i)', group: 'Curves', tags: ['parametric'], build: loader(spiralXml) },
  zigzag: { label: 'Zig-zag infill (parity sweep)', group: 'Infill & multi-layer', tags: ['infill', 'parametric'], build: loader(zigzagXml) },
  layered_tower: { label: 'Layered tower (z = 0.2 + i·0.3)', group: 'Infill & multi-layer', tags: ['multi-layer', 'parametric'], build: loader(towerXml) },
  twisted_vase: { label: 'Twisted vase (non-planar helix)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'], build: loader(twistedVaseXml) },
};
