import { create } from 'zustand';
import type { MachineProfile, Toolpath, Metrics, DesignDef } from '../types/domain';
import {
  ensureWasmInitialized,
  compileGcode,
  compileIR,
  compileMetrics,
  compileOptimizedIR,
  importGcode,
} from '../wasm/engine';
import { DESIGN_DEFS, FULLCONTROL_GALLERY, RESOLVE_PARAMS } from '../data/designs';

const DEFAULT_MACHINES: MachineProfile[] = [
  {
    id: 'bambu-x1-carbon',
    name: 'Bambu Lab X1-Carbon',
    manufacturer: 'Bambu Lab',
    build_volume: { x: [0, 256], y: [0, 256], z: [0, 256] },
    max_feedrates: { x: 500, y: 500, z: 30, e: 60 },
    max_acceleration: 20000,
    firmware: 'bambu',
  },
  {
    id: 'voron-2.4-350',
    name: 'Voron 2.4 (350mm)',
    manufacturer: 'Voron Design',
    build_volume: { x: [0, 350], y: [0, 350], z: [0, 340] },
    max_feedrates: { x: 600, y: 600, z: 50, e: 120 },
    max_acceleration: 15000,
    firmware: 'klipper',
  },
  {
    id: 'prusa-mk4s',
    name: 'Prusa MK4S',
    manufacturer: 'Prusa Research',
    build_volume: { x: [0, 250], y: [0, 210], z: [0, 220] },
    max_feedrates: { x: 300, y: 300, z: 30, e: 100 },
    max_acceleration: 4000,
    firmware: 'marlin',
  },
];

interface StudioState {
  isWasmReady: boolean;
  machines: MachineProfile[];
  activeMachine: MachineProfile;
  activeDesignKey: string;
  activeParams: Record<string, number>;
  toolpath: Toolpath | null;
  gcodeLines: string[];
  metrics: Metrics | null;
  optimizedToolpath: Toolpath | null;
  colorMode: 'type' | 'height' | 'speed';
  activeCategory: string;
  searchQuery: string;
  isPlaying: boolean;
  currentTime: number;
  maxTime: number;
  playSpeed: number;
  focusedLineIndex: number | null;

  // Actions
  initStudio: () => Promise<void>;
  setActiveMachine: (id: string) => void;
  selectDesign: (key: string) => void;
  updateParam: (paramId: string, value: number) => void;
  resetParams: () => void;
  setColorMode: (mode: 'type' | 'height' | 'speed') => void;
  setActiveCategory: (cat: string) => void;
  setSearchQuery: (q: string) => void;
  togglePlay: () => void;
  setPlaySpeed: (speed: number) => void;
  seekTime: (time: number) => void;
  setFocusedLine: (index: number | null) => void;
  importCustomGcode: (text: string, filename: string) => void;
  recompileCurrentDesign: () => void;
}

export const useStudioStore = create<StudioState>((set, get) => ({
  isWasmReady: false,
  machines: DEFAULT_MACHINES,
  activeMachine: DEFAULT_MACHINES[0],
  activeDesignKey: 'spiral_vase',
  activeParams: {},
  toolpath: null,
  gcodeLines: [],
  metrics: null,
  optimizedToolpath: null,
  colorMode: 'type',
  activeCategory: 'all',
  searchQuery: '',
  isPlaying: false,
  currentTime: 0,
  maxTime: 10,
  playSpeed: 1.0,
  focusedLineIndex: null,

  initStudio: async () => {
    await ensureWasmInitialized();
    set({ isWasmReady: true });

    // Load machine profiles
    try {
      const res = await fetch('./machines.json');
      if (res.ok) {
        const data = await res.json();
        if (data.machines && data.machines.length) {
          set({ machines: data.machines, activeMachine: data.machines[0] });
        }
      }
    } catch {
      // Keep defaults
    }

    get().selectDesign('spiral_vase');
  },

  setActiveMachine: (id: string) => {
    const m = get().machines.find((x) => x.id === id);
    if (m) set({ activeMachine: m });
  },

  selectDesign: (key: string) => {
    const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
    const def = allDefs[key];
    if (!def) return;

    const initialParams = Object.fromEntries(
      (def.params || []).map((p) => [p.id, p.defaultValue])
    );

    set({
      activeDesignKey: key,
      activeParams: initialParams,
      currentTime: 0,
      focusedLineIndex: null,
    });

    get().recompileCurrentDesign();
  },

  updateParam: (paramId: string, value: number) => {
    set((state) => ({
      activeParams: { ...state.activeParams, [paramId]: value },
    }));
    get().recompileCurrentDesign();
  },

  resetParams: () => {
    const { activeDesignKey } = get();
    const def = DESIGN_DEFS[activeDesignKey];
    if (!def) return;
    const initialParams = Object.fromEntries(
      (def.params || []).map((p) => [p.id, p.defaultValue])
    );
    set({ activeParams: initialParams });
    get().recompileCurrentDesign();
  },

  setColorMode: (mode) => set({ colorMode: mode }),
  setActiveCategory: (cat) => set({ activeCategory: cat }),
  setSearchQuery: (q) => set({ searchQuery: q }),

  togglePlay: () => set((state) => ({ isPlaying: !state.isPlaying })),
  setPlaySpeed: (speed) => set({ playSpeed: speed }),
  seekTime: (time) => set({ currentTime: time }),
  setFocusedLine: (index) => set({ focusedLineIndex: index }),

  importCustomGcode: (text: string) => {
    try {
      const tp = importGcode(text);
      const lines = text.split('\n').filter((l) => l.trim().length > 0);
      set({
        toolpath: tp,
        gcodeLines: lines,
        activeDesignKey: 'custom_import',
        currentTime: 0,
        maxTime: 10,
        focusedLineIndex: null,
      });
    } catch (err) {
      console.error('Import failed:', err);
    }
  },

  recompileCurrentDesign: () => {
    const { activeDesignKey, activeParams } = get();
    const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
    const def = allDefs[activeDesignKey];
    if (!def) return;

    const ops = typeof def.build === 'function' ? def.build(activeParams) : (def.ops || []);
    if (!ops || !ops.length) return;

    try {
      const gcode = compileGcode(ops, RESOLVE_PARAMS);
      const ir = compileIR(ops, RESOLVE_PARAMS);
      const m = compileMetrics(ops, RESOLVE_PARAMS);

      let optIr: Toolpath | null = null;
      try {
        optIr = compileOptimizedIR(ops, RESOLVE_PARAMS);
      } catch {
        // Optional
      }

      set({
        toolpath: ir,
        gcodeLines: gcode,
        metrics: m,
        optimizedToolpath: optIr,
        maxTime: m.total_time_s || 10,
      });
    } catch (err) {
      console.error('Compilation error:', err);
    }
  },
}));
