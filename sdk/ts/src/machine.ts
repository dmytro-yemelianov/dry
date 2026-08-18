/**
 * Machine properties & hardware capabilities catalog (dry.machine/1).
 */

import { MachineCapabilities } from './design.js';

export type MachineCategory =
  | '3d_printer'
  | 'cnc_mill'
  | 'laser_cutter'
  | 'plasma_waterjet'
  | 'robot_arm';

export type FirmwareFlavor = 'marlin' | 'klipper' | 'reprap' | 'rs274' | 'grbl' | 'krl';
export type KinematicsType = 'cartesian' | 'corexy' | 'delta' | 'five_axis' | 'robot_6dof';

export interface MachineEnvelope {
  bounds: [number, number, number, number, number, number]; // [minX, maxX, minY, maxY, minZ, maxZ]
  origin?: 'front_left' | 'center' | 'custom';
  safeTraverseZ?: number;
}

export interface MachineKinematicsConfig {
  type: KinematicsType;
  maxFeedrateMmMin: { x: number; y: number; z: number; e?: number };
  maxAccelerationMmS2?: { x: number; y: number; z: number; e?: number };
  maxJunctionVelocityMmS?: number;
}

export interface MachineToolheadConfig {
  index: number;
  kind: 'extruder_nozzle' | 'spindle' | 'laser_diode' | 'plasma_torch';
  nozzleDiameterMm?: number;
  maxTempC?: number;
  maxVolumetricFlowMm3S?: number;
  maxSpindleRpm?: number;
  offsetXyz?: [number, number, number];
}

export interface MachineProfileData {
  id: string;
  version?: number;
  name: string;
  vendor: string;
  category: MachineCategory;
  envelope: MachineEnvelope;
  firmware: {
    flavor: FirmwareFlavor;
    relativeE?: boolean;
    cannedCycles?: boolean;
  };
  kinematics: MachineKinematicsConfig;
  toolheads?: MachineToolheadConfig[];
  capabilities?: {
    heatedBed?: { maxTempC: number };
    laserPowerW?: number;
    spindleMaxRpm?: number;
    multiHeadSyncModes?: string[];
  };
}

/**
 * Machine Profile Model with validation and pre-flight mapping.
 */
export class MachineProfile {
  constructor(public readonly data: MachineProfileData) {}

  get id(): string {
    return this.data.id;
  }

  get name(): string {
    return this.data.name;
  }

  get vendor(): string {
    return this.data.vendor;
  }

  get category(): MachineCategory {
    return this.data.category;
  }

  get bounds(): [number, number, number, number, number, number] {
    return this.data.envelope.bounds;
  }

  /**
   * Convert machine profile into runtime capability requirements for pre-flight checking.
   */
  toCapabilities(): MachineCapabilities {
    const [minX, maxX, minY, maxY, minZ, maxZ] = this.data.envelope.bounds;
    return {
      name: this.data.name,
      xRange: { min: minX, max: maxX },
      yRange: { min: minY, max: maxY },
      zRange: { min: minZ, max: maxZ },
      maxFeedrate: this.data.kinematics.maxFeedrateMmMin.x,
      maxSpindleRpm: this.data.capabilities?.spindleMaxRpm,
    };
  }

  /**
   * Check if a 3D coordinate point resides safely within the machine envelope.
   */
  isWithinBounds(x: number, y: number, z: number): boolean {
    const [minX, maxX, minY, maxY, minZ, maxZ] = this.data.envelope.bounds;
    return x >= minX && x <= maxX && y >= minY && y <= maxY && z >= minZ && z <= maxZ;
  }
}

/**
 * Built-in official machine presets across all manufacturing domains.
 */
export const BUILTIN_MACHINES: Record<string, MachineProfileData> = {
  'bambu-x1c': {
    id: 'bambu-x1c',
    name: 'Bambu Lab X1 Carbon',
    vendor: 'Bambu Lab',
    category: '3d_printer',
    envelope: { bounds: [0, 256, 0, 256, 0, 256], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 3600 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 32 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'voron-v24-350': {
    id: 'voron-v24-350',
    name: 'Voron 2.4 350',
    vendor: 'Voron Design',
    category: '3d_printer',
    envelope: { bounds: [0, 350, 0, 350, 0, 330], origin: 'front_left', safeTraverseZ: 20 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 2400, e: 4800 },
      maxAccelerationMmS2: { x: 10000, y: 10000, z: 800, e: 6000 },
      maxJunctionVelocityMmS: 8,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'shapeoko-4': {
    id: 'shapeoko-4',
    name: 'Shapeoko 4 Standard',
    vendor: 'Carbide 3D',
    category: 'cnc_mill',
    envelope: { bounds: [0, 444, 0, 444, 0, 101], origin: 'front_left', safeTraverseZ: 15 },
    firmware: { flavor: 'grbl', cannedCycles: false },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 5000, y: 5000, z: 2000 },
      maxAccelerationMmS2: { x: 500, y: 500, z: 300 },
    },
    toolheads: [{ index: 0, kind: 'spindle', maxSpindleRpm: 24000 }],
    capabilities: { spindleMaxRpm: 24000 },
  },
  'haas-vf2': {
    id: 'haas-vf2',
    name: 'Haas VF-2 Vertical Machining Center',
    vendor: 'Haas Automation',
    category: 'cnc_mill',
    envelope: { bounds: [0, 762, 0, 406, 0, 508], origin: 'custom', safeTraverseZ: 50 },
    firmware: { flavor: 'rs274', cannedCycles: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 25400, y: 25400, z: 25400 },
      maxAccelerationMmS2: { x: 4900, y: 4900, z: 4900 },
    },
    toolheads: [{ index: 1, kind: 'spindle', maxSpindleRpm: 10000 }],
    capabilities: { spindleMaxRpm: 10000 },
  },
  'ortur-lm2': {
    id: 'ortur-lm2',
    name: 'Ortur Laser Master 2',
    vendor: 'Ortur',
    category: 'laser_cutter',
    envelope: { bounds: [0, 400, 0, 400, 0, 0], origin: 'front_left' },
    firmware: { flavor: 'grbl' },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 10000, y: 10000, z: 0 },
    },
    toolheads: [{ index: 0, kind: 'laser_diode' }],
    capabilities: { laserPowerW: 20 },
  },
  'crossfire-pro': {
    id: 'crossfire-pro',
    name: 'CrossFire PRO Plasma Table',
    vendor: 'Langmuir Systems',
    category: 'plasma_waterjet',
    envelope: { bounds: [0, 845, 0, 1225, 0, 100], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'grbl' },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 7600, y: 7600, z: 2500 },
    },
    toolheads: [{ index: 0, kind: 'plasma_torch' }],
  },
};

/**
 * Universal Machine Catalog client with offline built-in fallbacks.
 */
export class MachineCatalog {
  constructor(private readonly baseUrl = 'https://api.dry.yemelianov.dev') {}

  /**
   * Get machine profile by ID, checking built-in presets first.
   */
  async get(id: string): Promise<MachineProfile> {
    if (BUILTIN_MACHINES[id]) {
      return new MachineProfile(BUILTIN_MACHINES[id]);
    }

    try {
      const res = await fetch(`${this.baseUrl}/v1/machines/${id}`);
      if (!res.ok) {
        throw new Error(`Machine '${id}' not found`);
      }
      const data: MachineProfileData = await res.json();
      return new MachineProfile(data);
    } catch {
      throw new Error(`Machine '${id}' not found in catalog`);
    }
  }

  /**
   * Search machines with filter criteria.
   */
  search(filter?: { vendor?: string; category?: MachineCategory }): MachineProfile[] {
    const list = Object.values(BUILTIN_MACHINES);
    return list
      .filter((m) => {
        if (filter?.vendor && !m.vendor.toLowerCase().includes(filter.vendor.toLowerCase())) {
          return false;
        }
        if (filter?.category && m.category !== filter.category) {
          return false;
        }
        return true;
      })
      .map((m) => new MachineProfile(m));
  }
}
