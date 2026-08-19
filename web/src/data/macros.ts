import type { Op, ParameterDef } from '../types/domain';

const G = (w: number, h: number): Op => ({ op: 'geometry', width: w, height: h });
const ON: Op = { op: 'extruder', on: true };
const OFF: Op = { op: 'extruder', on: false };
const SPEED = (v: number): Op => ({ op: 'speed', print: v });
const M = (x?: number | null, y?: number | null, z?: number | null): Op => ({ op: 'move', x, y, z });
const ARC = (cx: number, cy: number, x: number | null, y: number | null, z: number | null, clockwise: boolean): Op => ({
  op: 'arc',
  cx,
  cy,
  x,
  y,
  z,
  clockwise,
});
const RETRACT = (distance: number, speed: number): Op => ({ op: 'retract', distance, speed });
const UNRETRACT = (distance: number, speed: number): Op => ({ op: 'unretract', distance, speed });
const TEMP = (c: number): Op => ({ op: 'temperature', nozzle: c });
const DWELL = (s: number): Op => ({ op: 'dwell', seconds: s });

export type MacroTarget = 'klipper' | 'marlin' | 'bambu' | 'dry_ir';

export interface MacroDef {
  id: string;
  name: string;
  category: 'startup' | 'shutdown' | 'calibration' | 'toolhead' | 'cnc_laser';
  description: string;
  params: ParameterDef[];
  generateOps: (params: Record<string, number>) => Op[];
  generateGcode: (params: Record<string, number>, target: MacroTarget) => string;
}

export const MACRO_LIBRARY: MacroDef[] = [
  {
    id: 'adaptive_purge_line',
    name: 'Adaptive Purge Line',
    category: 'startup',
    description: 'Smart prime line placed adjacent to the object bounding box instead of the edge of the bed.',
    params: [
      { id: 'startX', label: 'Start X', defaultValue: 10, min: 0, max: 300, step: 1, unit: 'mm' },
      { id: 'startY', label: 'Start Y', defaultValue: 10, min: 0, max: 300, step: 1, unit: 'mm' },
      { id: 'length', label: 'Line Length', defaultValue: 45, min: 10, max: 150, step: 1, unit: 'mm' },
      { id: 'flowWidth', label: 'Purge Width', defaultValue: 0.8, min: 0.4, max: 1.6, step: 0.1, unit: 'mm' },
    ],
    generateOps: ({ startX = 10, startY = 10, length = 45, flowWidth = 0.8 }) => [
      OFF,
      M(startX, startY, 5),
      M(startX, startY, 0.28),
      G(flowWidth, 0.28),
      SPEED(1200),
      UNRETRACT(1.0, 1800),
      ON,
      M(startX + length, startY, 0.28),
      M(startX + length, startY + 0.6, 0.28),
      M(startX + 10, startY + 0.6, 0.28),
      OFF,
      RETRACT(0.8, 2400),
      M(startX + 5, startY + 0.6, 1.0),
    ],
    generateGcode: ({ startX = 10, startY = 10, length = 45 }, target) => {
      if (target === 'klipper') {
        return `[gcode_macro ADAPTIVE_PURGE]\ngcode:\n  G90\n  G0 X${startX} Y${startY} Z5 F6000\n  G0 Z0.28 F1200\n  G92 E0\n  G1 X${startX + length} E15 F1200\n  G1 Y${startY + 0.6} E16 F1200\n  G1 X${startX + 10} E28 F1200\n  G92 E0\n  G1 E-0.8 F2400\n  G0 Z1.0 F3000`;
      }
      return `; --- Dry Macro: Adaptive Purge Line ---\nG90\nG0 X${startX} Y${startY} Z5 F6000\nG0 Z0.28 F1200\nG92 E0\nG1 X${startX + length} E15 F1200\nG1 Y${startY + 0.6} E16 F1200\nG1 X${startX + 10} E28 F1200\nG92 E0\nG1 E-0.8 F2400\nG0 Z1.0 F3000`;
    },
  },
  {
    id: 'nozzle_prime_blob',
    name: 'Corner Nozzle Prime Blob',
    category: 'startup',
    description: 'High-pressure nozzle prime blob on the bed corner with clean tangential wipe to eliminate startup voids.',
    params: [
      { id: 'cornerX', label: 'Corner X', defaultValue: 5, min: 0, max: 300, step: 1, unit: 'mm' },
      { id: 'cornerY', label: 'Corner Y', defaultValue: 5, min: 0, max: 300, step: 1, unit: 'mm' },
      { id: 'dwellSec', label: 'Prime Dwell', defaultValue: 2, min: 0.5, max: 10, step: 0.5, unit: 's' },
    ],
    generateOps: ({ cornerX = 5, cornerY = 5, dwellSec = 2 }) => [
      OFF,
      M(cornerX, cornerY, 0.4),
      SPEED(600),
      UNRETRACT(3.0, 1200),
      DWELL(dwellSec),
      OFF,
      M(cornerX + 15, cornerY + 15, 0.4),
      M(cornerX + 15, cornerY + 15, 2.0),
    ],
    generateGcode: ({ cornerX = 5, cornerY = 5, dwellSec = 2 }, target) => {
      if (target === 'klipper') {
        return `[gcode_macro NOZZLE_PRIME_BLOB]\ngcode:\n  G90\n  G0 X${cornerX} Y${cornerY} Z0.4 F3000\n  G92 E0\n  G1 E3.0 F600\n  G4 P${dwellSec * 1000}\n  G0 X${cornerX + 15} Y${cornerY + 15} Z0.4 F6000\n  G0 Z2.0 F3000`;
      }
      return `; --- Dry Macro: Corner Nozzle Prime Blob ---\nG90\nG0 X${cornerX} Y${cornerY} Z0.4 F3000\nG92 E0\nG1 E3.0 F600\nG4 P${dwellSec * 1000}\nG0 X${cornerX + 15} Y${cornerY + 15} Z0.4 F6000\nG0 Z2.0 F3000`;
    },
  },
  {
    id: 'coasting_seam_wipe',
    name: 'Coasting Seam Wipe',
    category: 'toolhead',
    description: 'Extrusion cut-off with trailing tangential wipe move into the interior to suppress perimeter seam zits.',
    params: [
      { id: 'coastDist', label: 'Coast Distance', defaultValue: 1.5, min: 0.2, max: 6.0, step: 0.1, unit: 'mm' },
      { id: 'wipeDist', label: 'Wipe Distance', defaultValue: 2.0, min: 0.5, max: 8.0, step: 0.1, unit: 'mm' },
    ],
    generateOps: ({ coastDist = 1.5, wipeDist = 2.0 }) => [
      OFF,
      M(null, null, null),
      RETRACT(0.4, 3000),
      M(null, null, null),
    ],
    generateGcode: ({ coastDist = 1.5, wipeDist = 2.0 }, target) => {
      if (target === 'klipper') {
        return `[gcode_macro COAST_WIPE]\ngcode:\n  ; Coasting last ${coastDist}mm without E\n  G1 E-0.4 F3000\n  G1 X+${wipeDist} F4000 ; Tangent wipe into perimeter interior`;
      }
      return `; --- Dry Macro: Coasting Seam Wipe ---\n; Coasting last ${coastDist}mm without E\nG1 E-0.4 F3000\nG1 X+${wipeDist} F4000 ; Tangent wipe`;
    },
  },
  {
    id: 'chamber_heatsoak',
    name: 'Chamber Heatsoak Dwell',
    category: 'startup',
    description: 'Brings bed and enclosed chamber up to thermal equilibrium before probe mesh to prevent first-layer thermal drift.',
    params: [
      { id: 'bedTemp', label: 'Bed Temp', defaultValue: 105, min: 40, max: 130, step: 5, unit: '°C' },
      { id: 'dwellMin', label: 'Soak Minutes', defaultValue: 10, min: 1, max: 60, step: 1, unit: 'min' },
    ],
    generateOps: ({ bedTemp = 105, dwellMin = 10 }) => [
      TEMP(bedTemp),
      DWELL(dwellMin * 60),
    ],
    generateGcode: ({ bedTemp = 105, dwellMin = 10 }, target) => {
      if (target === 'klipper') {
        return `[gcode_macro CHAMBER_HEATSOAK]\ngcode:\n  M140 S${bedTemp}\n  M190 S${bedTemp}\n  G4 P${dwellMin * 60 * 1000}\n  RESPOND MSG="Chamber thermal equilibrium reached."`;
      }
      return `; --- Dry Macro: Chamber Heatsoak ---\nM140 S${bedTemp}\nM190 S${bedTemp}\nG4 P${dwellMin * 60 * 1000} ; Soak for ${dwellMin} min`;
    },
  },
  {
    id: 'timelapse_park_shutter',
    name: 'Timelapse Park & Shutter',
    category: 'toolhead',
    description: 'Parks toolhead at back-left corner at layer transition and triggers camera shutter pin.',
    params: [
      { id: 'parkX', label: 'Park X', defaultValue: 10, min: 0, max: 350, step: 5, unit: 'mm' },
      { id: 'parkY', label: 'Park Y', defaultValue: 250, min: 0, max: 350, step: 5, unit: 'mm' },
      { id: 'retractDist', label: 'Retract', defaultValue: 0.8, min: 0, max: 4, step: 0.1, unit: 'mm' },
    ],
    generateOps: ({ parkX = 10, parkY = 250, retractDist = 0.8 }) => [
      RETRACT(retractDist, 2400),
      OFF,
      M(parkX, parkY, null),
      DWELL(0.15),
      UNRETRACT(retractDist, 2400),
    ],
    generateGcode: ({ parkX = 10, parkY = 250, retractDist = 0.8 }, target) => {
      if (target === 'klipper') {
        return `[gcode_macro TIMELAPSE_SNAPSHOT]\ngcode:\n  G91\n  G1 E-${retractDist} F2400\n  G90\n  G0 X${parkX} Y${parkY} F9000\n  G4 P150\n  SET_PIN PIN=camera_shutter VALUE=1\n  G4 P50\n  SET_PIN PIN=camera_shutter VALUE=0\n  G91\n  G1 E+${retractDist} F2400\n  G90`;
      }
      return `; --- Dry Macro: Timelapse Snapshot ---\nG91\nG1 E-${retractDist} F2400\nG90\nG0 X${parkX} Y${parkY} F9000\nG4 P150\nM240 ; Trigger camera shutter\nG91\nG1 E+${retractDist} F2400\nG90`;
    },
  },
  {
    id: 'laser_ramp_leadin',
    name: 'Laser Tangent Arc Lead-In',
    category: 'cnc_laser',
    description: 'Generates tangential arc lead-in with PWM power ramping for clean, slag-free pierce entry in sheet cutting.',
    params: [
      { id: 'radius', label: 'Arc Radius', defaultValue: 3.0, min: 0.5, max: 15.0, step: 0.5, unit: 'mm' },
      { id: 'powerPct', label: 'Max Power', defaultValue: 80, min: 10, max: 100, step: 5, unit: '%' },
    ],
    generateOps: ({ radius = 3.0 }) => [
      OFF,
      M(null, null, null),
      ON,
      ARC(radius, 0, radius, radius, null, false),
    ],
    generateGcode: ({ radius = 3.0, powerPct = 80 }, target) => {
      const pwm = Math.round((powerPct / 100) * 255);
      return `; --- Dry Macro: Laser Arc Lead-In ---\nM3 S${pwm} ; Laser On (PWM ${pwm})\nG3 X+${radius} Y+${radius} I0 J${radius} F1800 ; Tangential lead-in arc`;
    },
  },
];
