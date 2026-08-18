import initWasm, {
  resolve_gcode,
  resolve_ir,
  resolve_metrics,
  resolve_optimized_ir,
  resolve_verify,
  import_gcode_to_ir,
  tpms_ops_json,
  check_machine_compatibility,
} from '../../pkg/dry_wasm.js';
import type { Op, ResolveParams, Toolpath, Metrics } from '../types/domain';

let isInitialized = false;
let initPromise: Promise<unknown> | null = null;

export async function ensureWasmInitialized(): Promise<void> {
  if (isInitialized) return;
  if (!initPromise) {
    initPromise = initWasm();
  }
  await initPromise;
  isInitialized = true;
}

export function compileGcode(ops: Op[], params: ResolveParams): string[] {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const lines = resolve_gcode(opsJson, paramsJson, true, false, false, 'ab');
  return lines || [];
}

export function compileIR(ops: Op[], params: ResolveParams): Toolpath {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const irJson = resolve_ir(opsJson, paramsJson);
  return JSON.parse(irJson) as Toolpath;
}

export function compileMetrics(ops: Op[], params: ResolveParams): Metrics {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const metricsJson = resolve_metrics(opsJson, paramsJson);
  return JSON.parse(metricsJson) as Metrics;
}

export function compileOptimizedIR(ops: Op[], params: ResolveParams): Toolpath {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const irJson = resolve_optimized_ir(opsJson, paramsJson);
  return JSON.parse(irJson) as Toolpath;
}

export function compileVerify(ops: Op[], params: ResolveParams, maxFlow?: number, minTemp?: number): unknown {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const reportJson = resolve_verify(opsJson, paramsJson, maxFlow, minTemp);
  return JSON.parse(reportJson);
}

export function importGcode(gcodeText: string): Toolpath {
  const irJson = import_gcode_to_ir(gcodeText);
  return JSON.parse(irJson) as Toolpath;
}

export function generateTpmsOps(options: Record<string, unknown>): Op[] {
  const jsonStr = tpms_ops_json(JSON.stringify(options));
  return JSON.parse(jsonStr) as Op[];
}

export function checkMachineCompat(ops: Op[], params: ResolveParams, capabilities: Record<string, unknown>): unknown {
  const opsJson = JSON.stringify(ops);
  const paramsJson = JSON.stringify(params);
  const capsJson = JSON.stringify(capabilities);
  const res = check_machine_compatibility(opsJson, paramsJson, capsJson);
  return JSON.parse(res);
}
