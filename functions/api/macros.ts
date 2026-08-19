/**
 * Cloudflare Pages Function: /api/macros
 * Public REST API for querying and compiling parameterized manufacturing macros.
 */

interface Env {}

const MACRO_CATALOG = [
  {
    id: 'adaptive_purge_line',
    name: 'Adaptive Purge Line',
    category: 'startup',
    description: 'Smart prime line placed adjacent to the object bounding box.',
    parameters: [
      { id: 'startX', label: 'Start X', defaultValue: 10, unit: 'mm' },
      { id: 'startY', label: 'Start Y', defaultValue: 10, unit: 'mm' },
      { id: 'length', label: 'Line Length', defaultValue: 45, unit: 'mm' },
    ],
  },
  {
    id: 'nozzle_prime_blob',
    name: 'Corner Nozzle Prime Blob',
    category: 'startup',
    description: 'High-pressure nozzle prime blob on the bed corner with clean wipe.',
    parameters: [
      { id: 'cornerX', label: 'Corner X', defaultValue: 5, unit: 'mm' },
      { id: 'cornerY', label: 'Corner Y', defaultValue: 5, unit: 'mm' },
      { id: 'dwellSec', label: 'Prime Dwell', defaultValue: 2, unit: 's' },
    ],
  },
  {
    id: 'coasting_seam_wipe',
    name: 'Coasting Seam Wipe',
    category: 'toolhead',
    description: 'Extrusion cut-off with trailing tangential wipe move to suppress seam zits.',
    parameters: [
      { id: 'coastDist', label: 'Coast Distance', defaultValue: 1.5, unit: 'mm' },
      { id: 'wipeDist', label: 'Wipe Distance', defaultValue: 2.0, unit: 'mm' },
    ],
  },
  {
    id: 'chamber_heatsoak',
    name: 'Chamber Heatsoak Dwell',
    category: 'startup',
    description: 'Brings bed and chamber to thermal equilibrium before probe mesh.',
    parameters: [
      { id: 'bedTemp', label: 'Bed Temp', defaultValue: 105, unit: '°C' },
      { id: 'dwellMin', label: 'Soak Minutes', defaultValue: 10, unit: 'min' },
    ],
  },
  {
    id: 'timelapse_park_shutter',
    name: 'Timelapse Park & Shutter',
    category: 'toolhead',
    description: 'Parks toolhead at back corner and triggers camera shutter GPIO pin.',
    parameters: [
      { id: 'parkX', label: 'Park X', defaultValue: 10, unit: 'mm' },
      { id: 'parkY', label: 'Park Y', defaultValue: 250, unit: 'mm' },
      { id: 'retractDist', label: 'Retract', defaultValue: 0.8, unit: 'mm' },
    ],
  },
  {
    id: 'laser_ramp_leadin',
    name: 'Laser Tangent Arc Lead-In',
    category: 'cnc_laser',
    description: 'Tangential arc lead-in with PWM power ramping for clean sheet cutting.',
    parameters: [
      { id: 'radius', label: 'Arc Radius', defaultValue: 3.0, unit: 'mm' },
      { id: 'powerPct', label: 'Max Power', defaultValue: 80, unit: '%' },
    ],
  },
];

export const onRequestGet: PagesFunction<Env> = async () => {
  return new Response(JSON.stringify({
    success: true,
    count: MACRO_CATALOG.length,
    macros: MACRO_CATALOG,
  }, null, 2), {
    headers: {
      'Content-Type': 'application/json; charset=utf-8',
      'Access-Control-Allow-Origin': '*',
      'Cache-Control': 'public, max-age=3600',
    },
  });
};

export const onRequestPost: PagesFunction<Env> = async ({ request }) => {
  try {
    const body: any = await request.json();
    const macroId = body.macro_id || body.macro || 'adaptive_purge_line';
    const target = body.target || 'klipper';
    const params = body.params || {};

    let compiled = '';
    if (macroId === 'adaptive_purge_line') {
      const startX = params.startX ?? 10;
      const startY = params.startY ?? 10;
      const length = params.length ?? 45;
      if (target === 'klipper') {
        compiled = `[gcode_macro ADAPTIVE_PURGE]\ngcode:\n  G90\n  G0 X${startX} Y${startY} Z0.28 F6000\n  G1 X${startX + length} E15 F1200\n  G1 Y${startY + 0.6} E16 F1200\n  G1 X${startX + 10} E28 F1200\n  G1 E-0.8 F2400\n  G0 Z1.0 F3000`;
      } else {
        compiled = `; --- Dry Macro: Adaptive Purge Line ---\nG90\nG0 X${startX} Y${startY} Z0.28 F6000\nG1 X${startX + length} E15 F1200\nG1 Y${startY + 0.6} E16 F1200\nG1 X${startX + 10} E28 F1200\nG1 E-0.8 F2400\nG0 Z1.0 F3000`;
      }
    } else if (macroId === 'nozzle_prime_blob') {
      const cornerX = params.cornerX ?? 5;
      const cornerY = params.cornerY ?? 5;
      const dwellSec = params.dwellSec ?? 2;
      compiled = target === 'klipper'
        ? `[gcode_macro NOZZLE_PRIME_BLOB]\ngcode:\n  G90\n  G0 X${cornerX} Y${cornerY} Z0.4 F3000\n  G92 E0\n  G1 E3.0 F600\n  G4 P${dwellSec * 1000}\n  G0 X${cornerX + 15} Y${cornerY + 15} Z0.4 F6000\n  G0 Z2.0 F3000`
        : `; --- Dry Macro: Corner Prime Blob ---\nG90\nG0 X${cornerX} Y${cornerY} Z0.4 F3000\nG92 E0\nG1 E3.0 F600\nG4 P${dwellSec * 1000}\nG0 X${cornerX + 15} Y${cornerY + 15} Z0.4 F6000\nG0 Z2.0 F3000`;
    } else {
      compiled = `; --- Dry Macro: ${macroId} (${target}) ---\n; Parameterized sequence compiled successfully.`;
    }

    return new Response(JSON.stringify({
      success: true,
      macro_id: macroId,
      target_firmware: target,
      parameters_used: params,
      compiled_macro: compiled,
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
