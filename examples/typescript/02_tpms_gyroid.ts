/**
 * Dry TypeScript Example 02: TPMS Cellular Gyroid Lattice Infill
 */
import { tpms_gcode, tpms_ops } from '@dry/sdk';

function main() {
  console.log("=== Dry TypeScript Example 02: TPMS Gyroid Lattice ===");

  const options = {
    surface: "gyroid",
    cell_size: 15.0,
    grid_size: [2, 2, 2] as [number, number, number],
    layer_height: 0.3,
    iso_level: 0.0,
    stepover: 0.6,
  };

  const ops = tpms_ops(options);
  console.log(`✓ Generated ${ops.length} TPMS L1 toolpath operations.`);

  const gcode = tpms_gcode(options);
  console.log(`✓ Emitted ${gcode.split('\n').length} lines of machine-ready G-code.`);
}

main();
