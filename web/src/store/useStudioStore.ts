import { create } from 'zustand';
import type {
  MachineProfile,
  Toolpath,
  Metrics,
  DesignDef,
  GcodeSection,
  RenderStyle,
  PlasticMaterial,
} from '../types/domain';
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

function buildAutomatedSections(lines: string[]): GcodeSection[] {
  const sections: GcodeSection[] = [];
  let currentZ: number | null = null;
  let layer = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const words = line.trim().split(/\s+/);
    const zToken = words.find((w) => w.startsWith('Z') && !isNaN(parseFloat(w.slice(1))));
    if (!zToken) {
      if (i === 0 && !sections.length) {
        sections.push({ line: 0, layer: 0, z: 0, label: 'Start Routine' });
      }
      continue;
    }
    const zVal = parseFloat(zToken.slice(1));
    const roundedZ = Math.round(zVal * 1000) / 1000;
    if (currentZ === null || Math.abs(roundedZ - currentZ) > 1e-5) {
      currentZ = roundedZ;
      layer += 1;
      sections.push({
        line: i,
        layer,
        z: roundedZ,
        label: `Layer ${layer} (Z=${roundedZ.toFixed(2)}mm)`,
      });
    }
  }

  if (!sections.length) {
    sections.push({ line: 0, layer: 0, z: 0, label: 'Start Routine' });
  }

  return sections;
}

interface StudioState {
  isWasmReady: boolean;
  machines: MachineProfile[];
  activeMachine: MachineProfile;
  activeDesignKey: string;
  activeParams: Record<string, number>;
  toolpath: Toolpath | null;
  gcodeLines: string[];
  gcodeSections: GcodeSection[];
  metrics: Metrics | null;
  optimizedToolpath: Toolpath | null;
  colorMode: 'type' | 'height' | 'speed';
  renderStyle: RenderStyle;
  plasticMaterial: PlasticMaterial;
  activeCategory: string;
  searchQuery: string;
  isPlaying: boolean;
  currentTime: number;
  maxTime: number;
  playSpeed: number;
  focusedLineIndex: number | null;
  activeLayerNumber: number;

  // Actions
  initStudio: () => Promise<void>;
  setActiveMachine: (id: string) => void;
  selectDesign: (key: string) => void;
  updateParam: (paramId: string, value: number) => void;
  resetParams: () => void;
  setColorMode: (mode: 'type' | 'height' | 'speed') => void;
  setRenderStyle: (style: RenderStyle) => void;
  setPlasticMaterial: (mat: PlasticMaterial) => void;
  setActiveCategory: (cat: string) => void;
  setSearchQuery: (q: string) => void;
  togglePlay: () => void;
  setPlaySpeed: (speed: number) => void;
  seekTime: (time: number) => void;
  setFocusedLine: (index: number | null) => void;
  jumpToLayer: (layerNum: number) => void;
  nextLayer: () => void;
  prevLayer: () => void;
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
  gcodeSections: [],
  metrics: null,
  optimizedToolpath: null,
  colorMode: 'type',
  renderStyle: 'beads',
  plasticMaterial: 'cyan',
  activeCategory: 'all',
  searchQuery: '',
  isPlaying: false,
  currentTime: 0,
  maxTime: 10,
  playSpeed: 1.0,
  focusedLineIndex: null,
  activeLayerNumber: 1,

  initStudio: async () => {
    await ensureWasmInitialized();
    set({ isWasmReady: true });

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
      activeLayerNumber: 1,
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
  setRenderStyle: (style) => set({ renderStyle: style }),
  setPlasticMaterial: (mat) => set({ plasticMaterial: mat }),
  setActiveCategory: (cat) => set({ activeCategory: cat }),
  setSearchQuery: (q) => set({ searchQuery: q }),

  togglePlay: () => set((state) => ({ isPlaying: !state.isPlaying })),
  setPlaySpeed: (speed) => set({ playSpeed: speed }),
  seekTime: (time) => {
    const { gcodeSections, gcodeLines, maxTime } = get();
    const frac = maxTime > 0 ? Math.min(1.0, Math.max(0.0, time / maxTime)) : 0;
    const approxLine = Math.floor(frac * (gcodeLines.length - 1));

    let activeL = 1;
    for (const sec of gcodeSections) {
      if (approxLine >= sec.line) {
        activeL = sec.layer;
      } else {
        break;
      }
    }

    set({ currentTime: time, activeLayerNumber: activeL });
  },

  setFocusedLine: (index) => {
    if (index === null) {
      set({ focusedLineIndex: null });
      return;
    }
    const { gcodeSections } = get();
    let activeL = 1;
    for (const sec of gcodeSections) {
      if (index >= sec.line) {
        activeL = sec.layer;
      } else {
        break;
      }
    }
    set({ focusedLineIndex: index, activeLayerNumber: activeL });
  },

  jumpToLayer: (layerNum) => {
    const { gcodeSections, gcodeLines, maxTime } = get();
    const sec = gcodeSections.find((s) => s.layer === layerNum);
    if (!sec) return;
    const frac = gcodeLines.length > 0 ? sec.line / gcodeLines.length : 0;
    set({
      activeLayerNumber: layerNum,
      focusedLineIndex: sec.line,
      currentTime: frac * maxTime,
    });
  },

  nextLayer: () => {
    const { activeLayerNumber, gcodeSections } = get();
    const maxLayer = gcodeSections[gcodeSections.length - 1]?.layer || 1;
    if (activeLayerNumber < maxLayer) {
      get().jumpToLayer(activeLayerNumber + 1);
    }
  },

  prevLayer: () => {
    const { activeLayerNumber } = get();
    if (activeLayerNumber > 1) {
      get().jumpToLayer(activeLayerNumber - 1);
    }
  },

  importCustomGcode: (text: string) => {
    try {
      const tp = importGcode(text);
      const lines = text.split('\n').filter((l) => l.trim().length > 0);
      const sections = buildAutomatedSections(lines);
      set({
        toolpath: tp,
        gcodeLines: lines,
        gcodeSections: sections,
        activeDesignKey: 'custom_import',
        currentTime: 0,
        maxTime: 10,
        focusedLineIndex: null,
        activeLayerNumber: 1,
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
      const sections = buildAutomatedSections(gcode);

      let optIr: Toolpath | null = null;
      try {
        optIr = compileOptimizedIR(ops, RESOLVE_PARAMS);
      } catch {
        // Optional
      }

      set({
        toolpath: ir,
        gcodeLines: gcode,
        gcodeSections: sections,
        metrics: m,
        optimizedToolpath: optIr,
        maxTime: m.total_time_s || 10,
      });
    } catch (err) {
      console.error('Compilation error:', err);
    }
  },
}));
