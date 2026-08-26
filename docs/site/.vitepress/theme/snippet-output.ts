import type { Dry } from './dry-engine';
import type { Metrics, Report, Toolpath } from '@sdk/ops';

export interface SnippetOutputs {
  ir: Toolpath | null;
  gcode: string[];
  metrics: Metrics | null;
  verify: Report | null;
}

function isToolpath(value: unknown): value is Toolpath {
  return !!value && typeof value === 'object' && Array.isArray((value as Toolpath).segments);
}

function isMetrics(value: unknown): value is Metrics {
  return !!value && typeof value === 'object' && typeof (value as Metrics).total_time_s === 'number';
}

function isReport(value: unknown): value is Report {
  return !!value && typeof value === 'object' && Array.isArray((value as Report).findings);
}

/** Resolve only requested outputs and return a structured-clone-safe payload. */
export function resolveSnippetOutputs(value: unknown, requested: readonly string[], dry: Dry): SnippetOutputs {
  const wants = (name: string): boolean => requested.includes(name);
  const result = value as {
    ir?: () => Toolpath;
    gcode?: () => string[];
    simulate?: () => Metrics;
    verify?: (...args: unknown[]) => Report;
  };
  const ir = isToolpath(value) ? value : typeof result?.ir === 'function' ? result.ir() : null;

  const gcode = !wants('gcode')
    ? []
    : Array.isArray(value) && value.every((line) => typeof line === 'string')
      ? value
      : typeof result?.gcode === 'function'
        ? result.gcode()
        : [];

  const metrics = !wants('metrics')
    ? null
    : isMetrics(value)
      ? value
      : typeof result?.simulate === 'function'
        ? result.simulate()
        : ir
          ? dry.resolveMetricsIr(JSON.stringify(ir))
          : null;

  const verify = !wants('verify')
    ? null
    : isReport(value)
      ? value
      : typeof result?.verify === 'function'
        ? result.verify({ bounds: [[0, 250], [0, 210], [0, 220]] })
        : null;

  return { ir, gcode, metrics, verify };
}
