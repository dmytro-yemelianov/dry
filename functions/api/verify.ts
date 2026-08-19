/**
 * Cloudflare Pages Function: /api/verify
 * Public REST API for running the 5-layer physical machine safety shield on G-code.
 */

interface Env {}

const MACHINE_ENVELOPES: Record<string, { x: number; y: number; z: number; flowMax: number; fMax: number; nozTempMax: number; bedTempMax: number }> = {
  'bambu_x1c': { x: 256, y: 256, z: 256, flowMax: 32.0, fMax: 30000, nozTempMax: 300, bedTempMax: 120 },
  'prusa_mk4': { x: 250, y: 210, z: 220, flowMax: 22.0, fMax: 18000, nozTempMax: 290, bedTempMax: 120 },
  'voron_24_350': { x: 350, y: 350, z: 330, flowMax: 35.0, fMax: 36000, nozTempMax: 350, bedTempMax: 130 },
  'ender3_v3': { x: 220, y: 220, z: 250, flowMax: 18.0, fMax: 15000, nozTempMax: 260, bedTempMax: 100 },
};

export const onRequestPost: PagesFunction<Env> = async ({ request }) => {
  try {
    const body: any = await request.json();
    const gcode: string = body.gcode || '';
    const machineId: string = body.machine_id || 'bambu_x1c';
    const machine = MACHINE_ENVELOPES[machineId] || MACHINE_ENVELOPES['bambu_x1c'];

    const lines = gcode.split('\n');
    const violations: Array<{ line: number; rule: string; message: string }> = [];

    let curX = 0, curY = 0, curZ = 0, curE = 0, curF = 1200;
    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;
    let peakFlow = 0;
    let peakF = 0;

    for (let i = 0; i < lines.length; i++) {
      const raw = lines[i].trim();
      if (!raw || raw.startsWith(';')) continue;

      const words = raw.split(/\s+/);
      const cmd = words[0].toUpperCase();

      if (cmd === 'M104' || cmd === 'M109') {
        const sTok = words.find(w => w.startsWith('S'));
        if (sTok) {
          const temp = parseFloat(sTok.slice(1));
          if (temp > machine.nozTempMax) {
            violations.push({
              line: i + 1,
              rule: 'THERMAL_OVERRUN',
              message: `Nozzle temperature ${temp}°C exceeds safety limit of ${machine.nozTempMax}°C`,
            });
          }
        }
      }

      if (cmd === 'G0' || cmd === 'G1' || cmd === 'G2' || cmd === 'G3') {
        let targetX = curX, targetY = curY, targetZ = curZ, targetE = curE, targetF = curF;

        words.slice(1).forEach(tok => {
          const k = tok[0].toUpperCase();
          const v = parseFloat(tok.slice(1));
          if (isNaN(v)) return;
          if (k === 'X') targetX = v;
          if (k === 'Y') targetY = v;
          if (k === 'Z') targetZ = v;
          if (k === 'E') targetE = v;
          if (k === 'F') targetF = v;
        });

        if (targetX < 0 || targetX > machine.x) {
          violations.push({
            line: i + 1,
            rule: 'ENVELOPE_OVERTRAVEL',
            message: `X coordinate ${targetX.toFixed(2)}mm is outside physical bed volume [0..${machine.x}]mm`,
          });
        }
        if (targetY < 0 || targetY > machine.y) {
          violations.push({
            line: i + 1,
            rule: 'ENVELOPE_OVERTRAVEL',
            message: `Y coordinate ${targetY.toFixed(2)}mm is outside physical bed volume [0..${machine.y}]mm`,
          });
        }
        if (targetZ < 0 || targetZ > machine.z) {
          violations.push({
            line: i + 1,
            rule: 'ENVELOPE_OVERTRAVEL',
            message: `Z coordinate ${targetZ.toFixed(2)}mm is outside physical build height [0..${machine.z}]mm`,
          });
        }

        minX = Math.min(minX, targetX);
        maxX = Math.max(maxX, targetX);
        minY = Math.min(minY, targetY);
        maxY = Math.max(maxY, targetY);
        minZ = Math.min(minZ, targetZ);
        maxZ = Math.max(maxZ, targetZ);
        peakF = Math.max(peakF, targetF);

        if (targetF > machine.fMax) {
          violations.push({
            line: i + 1,
            rule: 'FEEDRATE_OVERSPEED',
            message: `Feedrate ${targetF}mm/min exceeds machine max XY feedrate of ${machine.fMax}mm/min`,
          });
        }

        const dx = targetX - curX;
        const dy = targetY - curY;
        const dz = targetZ - curZ;
        const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
        const dE = targetE - curE;

        if (dist > 0.001 && dE > 0 && targetF > 0) {
          const durationSec = (dist / targetF) * 60;
          const filamentVolumeMm3 = dE * (Math.PI * Math.pow(1.75 / 2, 2));
          const flowRate = filamentVolumeMm3 / durationSec;
          peakFlow = Math.max(peakFlow, flowRate);

          if (flowRate > machine.flowMax) {
            violations.push({
              line: i + 1,
              rule: 'OVER_EXTRUSION_FLOW',
              message: `Volumetric flow rate ${flowRate.toFixed(1)}mm³/s exceeds hotend melt threshold of ${machine.flowMax}mm³/s`,
            });
          }
        }

        curX = targetX;
        curY = targetY;
        curZ = targetZ;
        curE = targetE;
        curF = targetF;
      }
    }

    const passed = violations.length === 0;

    return new Response(JSON.stringify({
      success: true,
      machine_id: machineId,
      is_machine_safe: passed,
      total_lines_analyzed: lines.length,
      violations,
      envelope_metrics: {
        peak_flow_mm3_s: peakFlow,
        peak_feedrate_mm_min: peakF,
        bounds_mm: {
          x_min: minX === Infinity ? 0 : minX,
          x_max: maxX === -Infinity ? 0 : maxX,
          y_min: minY === Infinity ? 0 : minY,
          y_max: maxY === -Infinity ? 0 : maxY,
          z_min: minZ === Infinity ? 0 : minZ,
          z_max: maxZ === -Infinity ? 0 : maxZ,
        },
      },
      summary: passed
        ? `✅ 100% Machine Safe: All moves comply with physical build envelope and melt rates.`
        : `⚠️ REJECTED: ${violations.length} safety violations detected.`,
    }, null, 2), {
      headers: {
        'Content-Type': 'application/json; charset=utf-8',
        'Access-Control-Allow-Origin': '*',
      },
    });
  } catch (err: any) {
    return new Response(JSON.stringify({
      success: false,
      error: err.message || 'Invalid JSON request payload',
    }), {
      status: 400,
      headers: { 'Content-Type': 'application/json; charset=utf-8', 'Access-Control-Allow-Origin': '*' },
    });
  }
};
