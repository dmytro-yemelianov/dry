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

/** What a toolpath is checked against. Every field is optional; an unset one disables its rule. */
export interface VerifyContracts {
  maxFlow?: number;
  minTemp?: number;
  /** [min_x, max_x, min_y, max_y, min_z, max_z] */
  bounds?: number[];
  monotonicZ?: boolean;
  /** [min, max] in mm/min */
  speedRange?: number[];
  maxRetractionDistance?: number;
  maxRetractionSpeed?: number;
  maxTravelWithoutRetract?: number;
  firstLayerHeightRange?: number[];
  firstLayerSpeedRange?: number[];
  kinematics?: { max_acceleration_mm_s2?: number; max_junction_velocity_mm_s?: number };
}

export interface VerifyFinding {
  rule: string;
  severity: 'error' | 'warning' | 'info' | string;
  segment?: number | null;
  message: string;
}

export interface VerifyReport {
  findings: VerifyFinding[];
  /** Zero means the pass proved nothing — a clean report over no segments is not a clean toolpath. */
  segments_inspected: number;
  /** Which rules were actually in force. A rule absent here was never evaluated. */
  rules_evaluated: string[];
}

/** `0` disables a scalar ceiling in the binding; ranges are disabled by passing undefined. */
const scalar = (value: number | undefined): number => (Number.isFinite(value) ? (value as number) : 0);
const range = (value: number[] | undefined): Float64Array | undefined =>
  value && value.length ? Float64Array.from(value) : undefined;

export function compileVerify(
  ops: Op[],
  params: ResolveParams,
  contracts: VerifyContracts = {},
): VerifyReport {
  const reportJson = resolve_verify(
    JSON.stringify(ops),
    JSON.stringify(params),
    scalar(contracts.maxFlow),
    scalar(contracts.minTemp),
    range(contracts.bounds),
    Boolean(contracts.monotonicZ),
    range(contracts.speedRange),
    scalar(contracts.maxRetractionDistance),
    scalar(contracts.maxRetractionSpeed),
    scalar(contracts.maxTravelWithoutRetract),
    range(contracts.firstLayerHeightRange),
    range(contracts.firstLayerSpeedRange),
    contracts.kinematics ? JSON.stringify(contracts.kinematics) : '',
  );
  return JSON.parse(reportJson) as VerifyReport;
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

/** The shim `thumb.js` expects: just the one IR entry point, so it stays independent of this module. */
export const thumbnailWasm = { resolve_ir };
