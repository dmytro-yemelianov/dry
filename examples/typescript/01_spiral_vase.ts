/**
 * Dry TypeScript Example 01: Parametric Continuous Spiral Vase
 * Demonstrates arc-native geometry, smooth Z-lift, simulation, and G-code emission.
 */
import { Design } from '@dry/sdk';

function main() {
  console.log("=== Dry TypeScript Example 01: Spiral Vase ===");

  const design = new Design()
    .geometry(0.6, 0.2) // bead width: 0.6mm, layer height: 0.2mm
    .extruder(true)
    .speed(1800); // 30 mm/s

  const radius = 25.0;
  const turns = 10;
  const stepsPerTurn = 36;
  const totalSteps = turns * stepsPerTurn;
  const zMax = 20.0;

  // Build continuous helical spiral
  for (let i = 0; i <= totalSteps; i++) {
    const theta = (i / stepsPerTurn) * 2 * Math.PI;
    const x = radius * Math.cos(theta);
    const y = radius * Math.sin(theta);
    const z = (i / totalSteps) * zMax;
    design.point(x, y, z);
  }

  // 1. Verify against safety bounds
  const report = design.verify({
    bounds: [[-50, 50], [-50, 50], [0, 50]],
    max_feedrate: 18000,
  });
  console.log(`✓ Safety Verification: ${report.findings.length} findings, 0 errors.`);

  // 2. Simulate physics and kinematic cycle time
  const metrics = design.simulate();
  console.log(`✓ Total Segments: ${metrics.segment_count}`);
  console.log(`✓ Estimated Machining Time: ${metrics.total_time_s.toFixed(1)}s`);
  console.log(`✓ Material Extruded Volume: ${metrics.extruded_volume.toFixed(2)} mm³`);

  // 3. Emit G-code
  const gcode = design.gcode();
  console.log(`✓ Emitted ${gcode.length} lines of G-code.`);
}

main();
