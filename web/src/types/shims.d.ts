/// <reference types="vite/client" />
declare module '*/fullcontrol-gallery.generated.js' {
  export const FULLCONTROL_DESIGNS: Record<string, {
    label?: string;
    name?: string;
    description?: string;
    tags?: string[];
    ops: any[];
    /** Attribution pairs [label, href] for the reconstructed source. */
    links?: Array<[string, string]>;
  }>;
}

declare module '*/lattice-research.js' {
  export function starPolygonLatticeOps(options: any): any[];
}

declare module '*/dry_wasm.js' {
  export default function init(module_or_path?: any): Promise<any>;
  export function resolve_gcode(
    ops_json: string,
    params_json: string,
    relative_e: boolean,
    absolute_coordinates: boolean,
    g2_g3_clockwise_standard: boolean,
    plane: string
  ): string[];
  export function resolve_ir(ops_json: string, params_json: string): string;
  export function resolve_metrics(ops_json: string, params_json: string): string;
  export function resolve_optimized_ir(ops_json: string, params_json: string): string;
  export function resolve_verify(ops_json: string, params_json: string, max_flow?: number, min_temp?: number): string;
  export function import_gcode_to_ir(gcode_text: string): string;
  export function tpms_ops_json(options_json: string): string;
  export function check_machine_compatibility(ops_json: string, params_json: string, capabilities_json: string): string;
}
