// The first bounded L0 feature surface. Geometry expansion stays in Rust; these helpers only build the
// versionless P2.3 feature document and pass it to the wasm engine.
import { Design } from './design';
import { expandFeatures } from './engine';
import type { Op } from './ops';

export interface FeaturePose {
  x?: number;
  y?: number;
  z?: number;
  rotate_z_deg?: number;
  rotation?: {
    x: number;
    y: number;
    z: number;
    w: number;
  };
  frame?: string;
}

export type FeatureNode =
  | {
      kind: 'feature';
      name?: string;
      pose?: FeaturePose;
      ops: Op[];
    }
  | {
      kind: 'group';
      children: FeatureNode[];
    }
  | {
      kind: 'repeat';
      count: number;
      step?: FeaturePose;
      child: FeatureNode;
    };

export interface FeatureProgramDocument {
  features: FeatureNode[];
}

/** Wrap a coordinate-local L1 design/op list as a feature at a planar pose. */
export function feature(
  design: Design | readonly Op[],
  pose: FeaturePose = {},
  name?: string
): FeatureNode {
  const ops = design instanceof Design ? design.ops : design;
  return {
    kind: 'feature',
    ...(name === undefined ? {} : { name }),
    ...(Object.keys(pose).length === 0 ? {} : { pose }),
    ops: [...ops],
  };
}

/** Preserve source order while composing feature nodes. */
export function group(...children: FeatureNode[]): FeatureNode {
  return { kind: 'group', children };
}

/** Repeat a child; instance zero is unchanged and each later instance composes one `step`. */
export function repeat(child: FeatureNode, count: number, step: FeaturePose = {}): FeatureNode {
  return {
    kind: 'repeat',
    count,
    ...(Object.keys(step).length === 0 ? {} : { step }),
    child,
  };
}

export class FeatureProgram {
  readonly features: FeatureNode[] = [];

  add(...nodes: FeatureNode[]): this {
    this.features.push(...nodes);
    return this;
  }

  /** Expand through the Rust engine and return the canonical L1 `Design`. */
  expand(): Design {
    return Design.fromOps(expandFeatures({ features: this.features }));
  }
}
