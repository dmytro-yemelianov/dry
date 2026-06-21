export const MAX_PATTERN_POINTS = 3000;
export const TAU = Math.PI * 2;

const finite = (value, fallback) => (Number.isFinite(Number(value)) ? Number(value) : fallback);

export function vaseHelixOps(input = {}) {
  const cx = finite(input.cx, 50);
  const cy = finite(input.cy, 50);
  const z0 = finite(input.z0, 0.2);
  const turns = Math.max(0.25, finite(input.turns, 16));
  const samplesPerTurn = Math.max(6, Math.round(finite(input.samplesPerTurn, 60)));
  const height = Math.max(0, finite(input.height, 48));
  const base = Math.max(0, finite(input.base, 8.5));
  const belly = Math.max(0, finite(input.belly, 6.5));
  const shoulder = finite(input.shoulder, 3);
  const flutes = Math.max(0, Math.round(finite(input.flutes, 8)));
  const depth = Math.max(0, finite(input.depth, 0.035));
  const twistTurns = finite(input.twistTurns, 0.75);
  const rawSteps = Math.max(1, Math.round(turns * samplesPerTurn));
  const steps = Math.min(MAX_PATTERN_POINTS, rawSteps);

  const pointAt = (i) => {
    const f = i / steps;
    const angle = i * (TAU / samplesPerTurn);
    const twist = TAU * twistTurns * f;
    const profile = base + belly * Math.sin(Math.PI * f) + shoulder * Math.sin(TAU * f);
    const flute = 1 + depth * Math.cos(flutes * (angle - twist));
    const radius = Math.max(0, profile * flute);
    return {
      x: cx + radius * Math.cos(angle),
      y: cy + radius * Math.sin(angle),
      z: z0 + height * f,
    };
  };

  const start = pointAt(0);
  const ops = [{ op: 'extruder', on: false }, { op: 'move', ...start }, { op: 'extruder', on: true }];
  for (let i = 1; i <= steps; i++) ops.push({ op: 'move', ...pointAt(i) });
  return { ops, rawSteps, steps };
}
