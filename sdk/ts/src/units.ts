// Public dimensional quantity constructors (D1.1).
// Normalizes user-facing dimensional values into Dry's canonical internal units:
// - Length: millimetres (mm)
// - Angle: radians (rad)
// - Feedrate: mm/min (standard G-code F value)
// - Temperature: degrees Celsius (°C)
// - Time: seconds (s)

/** Length in millimetres (canonical unit). */
export function mm(value: number): number {
  return value;
}

/** Length in centimetres -> converted to mm. */
export function cm(value: number): number {
  return value * 10.0;
}

/** Length in inches -> converted to mm. */
export function inch(value: number): number {
  return value * 25.4;
}

/** Angle in degrees -> converted to radians. */
export function deg(value: number): number {
  return (value * Math.PI) / 180.0;
}

/** Angle in radians (canonical unit). */
export function rad(value: number): number {
  return value;
}

/** Feedrate in mm/s -> converted to mm/min (canonical G-code F value). */
export function mm_s(value: number): number {
  return value * 60.0;
}

/** Feedrate in mm/min (canonical unit). */
export function mm_min(value: number): number {
  return value;
}

/** Temperature in degrees Celsius (canonical unit). */
export function celsius(value: number): number {
  return value;
}

/** Duration in seconds (canonical unit). */
export function s(value: number): number {
  return value;
}

/** Duration in milliseconds -> converted to seconds. */
export function ms(value: number): number {
  return value / 1000.0;
}
