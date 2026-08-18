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
  'bambu-x1-carbon': {
    id: 'bambu-x1-carbon',
    name: 'Bambu Lab X1-Carbon',
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
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
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
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'voron-v24-350': {
    id: 'voron-v24-350',
    name: 'Voron 2.4 350',
    vendor: 'Voron Design',
    category: '3d_printer',
    envelope: { bounds: [0, 350, 0, 350, 0, 340], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 3000, e: 7200 },
      maxAccelerationMmS2: { x: 15000, y: 15000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 130 } },
  },
  'bambu-p1s': {
    id: 'bambu-p1s',
    name: 'Bambu Lab P1S',
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
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'bambu-p1p': {
    id: 'bambu-p1p',
    name: 'Bambu Lab P1P',
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
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'bambu-a1': {
    id: 'bambu-a1',
    name: 'Bambu Lab A1',
    vendor: 'Bambu Lab',
    category: '3d_printer',
    envelope: { bounds: [0, 256, 0, 256, 0, 256], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 3600 },
      maxAccelerationMmS2: { x: 10000, y: 10000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'bambu-a1-mini': {
    id: 'bambu-a1-mini',
    name: 'Bambu Lab A1 Mini',
    vendor: 'Bambu Lab',
    category: '3d_printer',
    envelope: { bounds: [0, 180, 0, 180, 0, 180], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 3600 },
      maxAccelerationMmS2: { x: 10000, y: 10000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 80 } },
  },
  'creality-k1': {
    id: 'creality-k1',
    name: 'Creality K1',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 220, 0, 220, 0, 250], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 1800, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'creality-k1-max': {
    id: 'creality-k1-max',
    name: 'Creality K1 Max',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 300], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 1800, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'creality-ender-3-v3-ke': {
    id: 'creality-ender-3-v3-ke',
    name: 'Creality Ender-3 V3 KE',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 220, 0, 220, 0, 240], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 8000, y: 8000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'creality-ender-3-v3-plus': {
    id: 'creality-ender-3-v3-plus',
    name: 'Creality Ender-3 V3 Plus',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 330], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 1800, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'creality-ender-3-v2': {
    id: 'creality-ender-3-v2',
    name: 'Creality Ender-3 V2',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 220, 0, 220, 0, 250], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 9000, y: 9000, z: 900, e: 3000 },
      maxAccelerationMmS2: { x: 1000, y: 1000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 260, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'creality-cr-10-smart-pro': {
    id: 'creality-cr-10-smart-pro',
    name: 'Creality CR-10 Smart Pro',
    vendor: 'Creality',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 400], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 10800, y: 10800, z: 1200, e: 3600 },
      maxAccelerationMmS2: { x: 1500, y: 1500, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'prusa-mk4s': {
    id: 'prusa-mk4s',
    name: 'Prusa MK4S',
    vendor: 'Prusa Research',
    category: '3d_printer',
    envelope: { bounds: [0, 250, 0, 210, 0, 220], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 18000, y: 18000, z: 1800, e: 6000 },
      maxAccelerationMmS2: { x: 4000, y: 4000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'prusa-mk3s-plus': {
    id: 'prusa-mk3s-plus',
    name: 'Prusa i3 MK3S+',
    vendor: 'Prusa Research',
    category: '3d_printer',
    envelope: { bounds: [0, 250, 0, 210, 0, 210], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 12000, y: 12000, z: 1200, e: 3000 },
      maxAccelerationMmS2: { x: 1500, y: 1500, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'prusa-mini-plus': {
    id: 'prusa-mini-plus',
    name: 'Prusa MINI+',
    vendor: 'Prusa Research',
    category: '3d_printer',
    envelope: { bounds: [0, 180, 0, 180, 0, 180], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 12000, y: 12000, z: 1200, e: 3600 },
      maxAccelerationMmS2: { x: 2000, y: 2000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 280, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'prusa-xl-5tool': {
    id: 'prusa-xl-5tool',
    name: 'Prusa XL (5-Tool Toolchanger)',
    vendor: 'Prusa Research',
    category: '3d_printer',
    envelope: { bounds: [0, 360, 0, 360, 0, 360], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 24000, y: 24000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 6000, y: 6000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 115 } },
  },
  'voron-2.4-350': {
    id: 'voron-2.4-350',
    name: 'Voron 2.4 (350mm)',
    vendor: 'Voron Design',
    category: '3d_printer',
    envelope: { bounds: [0, 350, 0, 350, 0, 340], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 3000, e: 7200 },
      maxAccelerationMmS2: { x: 15000, y: 15000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 130 } },
  },
  'voron-trident-300': {
    id: 'voron-trident-300',
    name: 'Voron Trident (300mm)',
    vendor: 'Voron Design',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 250], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 2400, e: 6000 },
      maxAccelerationMmS2: { x: 12000, y: 12000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 130 } },
  },
  'voron-v0.2': {
    id: 'voron-v0.2',
    name: 'Voron V0.2',
    vendor: 'Voron Design',
    category: '3d_printer',
    envelope: { bounds: [0, 120, 0, 120, 0, 120], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 3000, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'elegoo-neptune-4-pro': {
    id: 'elegoo-neptune-4-pro',
    name: 'Elegoo Neptune 4 Pro',
    vendor: 'Elegoo',
    category: '3d_printer',
    envelope: { bounds: [0, 225, 0, 225, 0, 265], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 8000, y: 8000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 110 } },
  },
  'elegoo-neptune-4-max': {
    id: 'elegoo-neptune-4-max',
    name: 'Elegoo Neptune 4 Max',
    vendor: 'Elegoo',
    category: '3d_printer',
    envelope: { bounds: [0, 420, 0, 420, 0, 480], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1500, e: 4800 },
      maxAccelerationMmS2: { x: 6000, y: 6000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 90 } },
  },
  'anycubic-kobra-2-pro': {
    id: 'anycubic-kobra-2-pro',
    name: 'Anycubic Kobra 2 Pro',
    vendor: 'Anycubic',
    category: '3d_printer',
    envelope: { bounds: [0, 220, 0, 220, 0, 250], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 10000, y: 10000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 260, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 110 } },
  },
  'anycubic-kobra-2-max': {
    id: 'anycubic-kobra-2-max',
    name: 'Anycubic Kobra 2 Max',
    vendor: 'Anycubic',
    category: '3d_printer',
    envelope: { bounds: [0, 420, 0, 420, 0, 500], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1500, e: 4800 },
      maxAccelerationMmS2: { x: 8000, y: 8000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 260, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 90 } },
  },
  'qidi-x-max-3': {
    id: 'qidi-x-max-3',
    name: 'Qidi Tech X-Max 3',
    vendor: 'Qidi Tech',
    category: '3d_printer',
    envelope: { bounds: [0, 325, 0, 325, 0, 315], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 2400, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'qidi-q1-pro': {
    id: 'qidi-q1-pro',
    name: 'Qidi Tech Q1 Pro',
    vendor: 'Qidi Tech',
    category: '3d_printer',
    envelope: { bounds: [0, 245, 0, 245, 0, 245], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 2400, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'sovol-sv07-plus': {
    id: 'sovol-sv07-plus',
    name: 'Sovol SV07 Plus',
    vendor: 'Sovol',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 350], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 30000, y: 30000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 8000, y: 8000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'sovol-sv06-plus': {
    id: 'sovol-sv06-plus',
    name: 'Sovol SV06 Plus',
    vendor: 'Sovol',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 300, 0, 340], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 10800, y: 10800, z: 1200, e: 3600 },
      maxAccelerationMmS2: { x: 1500, y: 1500, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'ratrig-v-core-3-500': {
    id: 'ratrig-v-core-3-500',
    name: 'RatRig V-Core 3.1 (500mm)',
    vendor: 'RatRig',
    category: '3d_printer',
    envelope: { bounds: [0, 500, 0, 500, 0, 500], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 2400, e: 7200 },
      maxAccelerationMmS2: { x: 15000, y: 15000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 350, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 120 } },
  },
  'flashforge-adventurer-5m-pro': {
    id: 'flashforge-adventurer-5m-pro',
    name: 'FlashForge Adventurer 5M Pro',
    vendor: 'FlashForge',
    category: '3d_printer',
    envelope: { bounds: [0, 220, 0, 220, 0, 220], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 36000, y: 36000, z: 1800, e: 4800 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 280, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 110 } },
  },
  'snapmaker-j1s-idex': {
    id: 'snapmaker-j1s-idex',
    name: 'Snapmaker J1s (IDEX)',
    vendor: 'Snapmaker',
    category: '3d_printer',
    envelope: { bounds: [0, 300, 0, 200, 0, 200], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'marlin', relativeE: true },
    kinematics: {
      type: 'cartesian',
      maxFeedrateMmMin: { x: 21000, y: 21000, z: 1200, e: 3600 },
      maxAccelerationMmS2: { x: 10000, y: 10000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
  },
  'two-trees-sk1': {
    id: 'two-trees-sk1',
    name: 'Two Trees SK1',
    vendor: 'Two Trees',
    category: '3d_printer',
    envelope: { bounds: [0, 256, 0, 256, 0, 256], origin: 'front_left', safeTraverseZ: 25 },
    firmware: { flavor: 'klipper', relativeE: true },
    kinematics: {
      type: 'corexy',
      maxFeedrateMmMin: { x: 42000, y: 42000, z: 1800, e: 6000 },
      maxAccelerationMmS2: { x: 20000, y: 20000, z: 500, e: 5000 },
      maxJunctionVelocityMmS: 10,
    },
    toolheads: [{ index: 0, kind: 'extruder_nozzle', nozzleDiameterMm: 0.4, maxTempC: 300, maxVolumetricFlowMm3S: 30 }],
    capabilities: { heatedBed: { maxTempC: 100 } },
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
