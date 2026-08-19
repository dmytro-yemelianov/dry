/**
 * Cloudflare Pages Function: /api/machines
 * Public REST API for listing verified machine profiles and kinematic envelopes.
 */

interface Env {}

const MACHINES = [
  { id: 'bambu_x1c', name: 'Bambu Lab X1-Carbon', volume: [256, 256, 256], maxSpeed: 30000, maxFlow: 32.0, firmware: 'bambu' },
  { id: 'bambu_p1s', name: 'Bambu Lab P1S / P1P', volume: [256, 256, 256], maxSpeed: 30000, maxFlow: 30.0, firmware: 'bambu' },
  { id: 'prusa_mk4', name: 'Prusa MK4', volume: [250, 210, 220], maxSpeed: 18000, maxFlow: 22.0, firmware: 'marlin' },
  { id: 'prusa_xl', name: 'Prusa XL (5-Toolhead)', volume: [360, 360, 360], maxSpeed: 24000, maxFlow: 28.0, firmware: 'marlin' },
  { id: 'voron_24_350', name: 'Voron 2.4 (350mm)', volume: [350, 350, 330], maxSpeed: 36000, maxFlow: 35.0, firmware: 'klipper' },
  { id: 'voron_v02', name: 'Voron V0.2', volume: [120, 120, 120], maxSpeed: 36000, maxFlow: 25.0, firmware: 'klipper' },
  { id: 'creality_k1_max', name: 'Creality K1 Max', volume: [300, 300, 300], maxSpeed: 36000, maxFlow: 32.0, firmware: 'klipper' },
  { id: 'ender3_v3', name: 'Creality Ender-3 V3', volume: [220, 220, 250], maxSpeed: 15000, maxFlow: 18.0, firmware: 'marlin' },
  { id: 'ratrig_vcore4', name: 'RatRig V-Core 4 (500mm)', volume: [500, 500, 500], maxSpeed: 42000, maxFlow: 40.0, firmware: 'klipper' },
  { id: 'pocketnc_v2', name: 'Pocket NC V2-50 (5-Axis)', volume: [115, 128, 90], maxSpeed: 3000, maxFlow: 0, firmware: 'linuxcnc' },
];

export const onRequestGet: PagesFunction<Env> = async () => {
  return new Response(JSON.stringify({
    success: true,
    total_machines: MACHINES.length,
    machines: MACHINES,
  }, null, 2), {
    headers: {
      'Content-Type': 'application/json; charset=utf-8',
      'Access-Control-Allow-Origin': '*',
      'Cache-Control': 'public, max-age=86400',
    },
  });
};
