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
  /**
   * Full contract surface. The four-argument form this used to declare left nine parameters
   * undefined at the boundary, which is why the only caller was dead code.
   * `0` disables a scalar ceiling; `undefined` disables a range; `''` disables the kinematics rules.
   */
  export function resolve_verify(
    ops_json: string,
    params_json: string,
    max_flow_opt: number,
    min_temp_opt: number,
    bounds: Float64Array | undefined,
    monotonic_z: boolean,
    speed_range: Float64Array | undefined,
    max_retraction_distance_opt: number,
    max_retraction_speed_opt: number,
    max_travel_without_retract_opt: number,
    first_layer_height_range: Float64Array | undefined,
    first_layer_speed_range: Float64Array | undefined,
    kinematics_json: string,
  ): string;
  export function import_gcode_to_ir(gcode_text: string): string;
  export function tpms_ops_json(options_json: string): string;
  export function check_machine_compatibility(ops_json: string, params_json: string, capabilities_json: string): string;
}

declare module '*/thumb.js' {
  /** Renders a top-down PNG data URL of the resolved toolpath. */
  export function thumbnail(
    ops: unknown[],
    wasm: { resolve_ir: (opsJson: string, paramsJson: string) => string },
    params: unknown,
    size?: number,
  ): string;
}
