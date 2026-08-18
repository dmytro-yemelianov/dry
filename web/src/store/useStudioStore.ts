import { create } from 'zustand';
import type {
  MachineProfile,
  Toolpath,
  Metrics,
  DesignDef,
  GcodeSection,
  GroupingMode,
  GroupingKind,
  RenderStyle,
  PlasticMaterial,
  SlicingFilterMode,
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

const TAU = Math.PI * 2;

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

/** Intelligent Multi-Modal Sectioning and Grouping Engine */
function computeIntelligentGrouping(
  gcodeLines: string[],
  toolpath: Toolpath | null,
  requestedMode: GroupingMode
): {
  sections: GcodeSection[];
  effectiveKind: GroupingKind;
  segmentSections: number[];
} {
  const segments = toolpath?.segments || [];
  if (!segments.length) {
    const defaultSec: GcodeSection = {
      index: 1,
      line: 0,
      segmentIndex: 0,
      kind: 'routine',
      label: 'Start Routine',
    };
    return { sections: [defaultSec], effectiveKind: 'routine', segmentSections: [] };
  }

  // 1. Analyze Geometry & Toolpath Characteristics
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  let zChanges = 0;
  let continuousZChanges = 0;
  let lastZ: number | null = null;
  let islands = 0;
  let inExtrusion = false;

  for (const seg of segments) {
    const pt = seg.end || seg.start;
    if (pt && pt[0] !== null && pt[1] !== null) {
      const x = pt[0] as number, y = pt[1] as number;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
    const z = pt && pt[2] !== null ? (pt[2] as number) : 0;
    if (lastZ === null) {
      lastZ = z;
    } else if (Math.abs(z - lastZ) > 1e-4) {
      zChanges++;
      if (Math.abs(z - lastZ) < 0.1) continuousZChanges++;
      lastZ = z;
    }

    const isTravel = seg.travel === true || seg.kind === 'travel';
    if (!isTravel) {
      if (!inExtrusion) {
        islands++;
        inExtrusion = true;
      }
    } else {
      inExtrusion = false;
    }
  }

  const cx = isFinite(minX) ? (minX + maxX) / 2 : 50;
  const cy = isFinite(minY) ? (minY + maxY) / 2 : 50;
  const isContinuousSpiral = continuousZChanges > 15 && continuousZChanges > zChanges * 0.7;

  // 2. Decide Effective Strategy
  let strategy: GroupingKind = 'layer';
  if (requestedMode === 'revolutions') strategy = 'revolution';
  else if (requestedMode === 'figures') strategy = 'figure';
  else if (requestedMode === 'layers') strategy = 'layer';
  else {
    // Auto Detection
    if (isContinuousSpiral) strategy = 'revolution';
    else if (islands > 1 && zChanges <= 3) strategy = 'figure';
    else if (zChanges > 1) strategy = 'layer';
    else if (islands > 1) strategy = 'figure';
    else strategy = 'revolution';
  }

  const sections: GcodeSection[] = [];
  const segmentSections: number[] = [];

  // Approximate mapping of segment index to G-code line index
  const lineFactor = gcodeLines.length > 0 && segments.length > 0 ? gcodeLines.length / segments.length : 1;

  if (strategy === 'revolution') {
    // ---- Revolutions / Turns Grouping ----
    let cumulativeAngle = 0;
    let lastAngle: number | null = null;
    let currentTurn = 1;
    let turnStartSeg = 0;
    let zStart = 0;

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      const pt = seg.end || seg.start || [cx, cy, 0];
      const px = (pt[0] !== null ? (pt[0] as number) : cx) - cx;
      const py = (pt[1] !== null ? (pt[1] as number) : cy) - cy;
      const pz = pt[2] !== null ? (pt[2] as number) : 0;

      if (i === 0) zStart = pz;

      const angle = Math.atan2(py, px);
      if (lastAngle !== null) {
        let diff = angle - lastAngle;
        if (diff > Math.PI) diff -= TAU;
        if (diff < -Math.PI) diff += TAU;
        cumulativeAngle += Math.abs(diff);
      }
      lastAngle = angle;

      const completedTurns = Math.floor(cumulativeAngle / TAU);
      if (completedTurns >= currentTurn || i === 0) {
        if (i > 0) {
          const prevSec = sections[sections.length - 1];
          if (prevSec) {
            prevSec.moveCount = i - turnStartSeg;
            prevSec.zRange = [zStart, pz];
          }
          currentTurn = completedTurns + 1;
        }

        turnStartSeg = i;
        zStart = pz;
        const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i * lineFactor));
        sections.push({
          index: currentTurn,
          line: lineIdx,
          segmentIndex: i,
          kind: 'revolution',
          label: `Turn ${currentTurn} (${(currentTurn * 360 - 360)}°–${currentTurn * 360}°)`,
          subLabel: `Z: ${pz.toFixed(2)}mm`,
          zRange: [pz, pz],
          angleRangeDeg: [currentTurn * 360 - 360, currentTurn * 360],
        });
      }

      segmentSections.push(sections.length);
    }
  } else if (strategy === 'figure') {
    // ---- Discrete Geometric Figures / Islands Grouping ----
    let currentFig = 0;
    let figStartSeg = 0;
    let inFig = false;

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      const isTravel = seg.travel === true || seg.kind === 'travel';

      if (!isTravel) {
        if (!inFig) {
          currentFig++;
          inFig = true;
          figStartSeg = i;
          const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i * lineFactor));
          const pt = seg.end || seg.start;
          const pz = pt && pt[2] !== null ? (pt[2] as number) : 0;
          sections.push({
            index: currentFig,
            line: lineIdx,
            segmentIndex: i,
            kind: 'figure',
            label: `Figure ${currentFig} (Extrusion Loop)`,
            subLabel: `Z: ${pz.toFixed(2)}mm`,
            zRange: [pz, pz],
          });
        }
      } else {
        if (inFig) {
          inFig = false;
          const prevSec = sections[sections.length - 1];
          if (prevSec) prevSec.moveCount = i - figStartSeg;
        }
      }

      segmentSections.push(Math.max(1, sections.length));
    }
  } else {
    // ---- Discrete Layers Grouping ----
    let currentLayer = 0;
    let currentZ: number | null = null;
    let layerStartSeg = 0;

    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      const pt = seg.end || seg.start;
      const pz = pt && pt[2] !== null ? (pt[2] as number) : 0;
      const roundedZ = Math.round(pz * 1000) / 1000;

      if (currentZ === null || Math.abs(roundedZ - currentZ) > 1e-4) {
        if (currentZ !== null) {
          const prevSec = sections[sections.length - 1];
          if (prevSec) prevSec.moveCount = i - layerStartSeg;
        }
        currentZ = roundedZ;
        currentLayer++;
        layerStartSeg = i;
        const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i * lineFactor));
        sections.push({
          index: currentLayer,
          line: lineIdx,
          segmentIndex: i,
          kind: 'layer',
          label: `Layer ${currentLayer} (Z=${roundedZ.toFixed(2)}mm)`,
          subLabel: `${roundedZ.toFixed(3)} mm`,
          zRange: [roundedZ, roundedZ],
        });
      }

      segmentSections.push(currentLayer);
    }
  }

  if (!sections.length) {
    sections.push({
      index: 1,
      line: 0,
      segmentIndex: 0,
      kind: strategy,
      label: 'Section 1',
    });
  }

  return { sections, effectiveKind: strategy, segmentSections };
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
  segmentSections: number[];
  groupingMode: GroupingMode;
  effectiveGroupingKind: GroupingKind;
  metrics: Metrics | null;
  optimizedToolpath: Toolpath | null;
  colorMode: 'type' | 'height' | 'speed';
  renderStyle: RenderStyle;
  plasticMaterial: PlasticMaterial;
  slicingFilterMode: SlicingFilterMode;
  targetSectionIndex: number;
  activeCategory: string;
  searchQuery: string;
  isPlaying: boolean;
  currentTime: number;
  maxTime: number;
  playSpeed: number;
  focusedLineIndex: number | null;
  activeSectionIndex: number;

  // Actions
  initStudio: () => Promise<void>;
  setActiveMachine: (id: string) => void;
  selectDesign: (key: string) => void;
  updateParam: (paramId: string, value: number) => void;
  resetParams: () => void;
  setColorMode: (mode: 'type' | 'height' | 'speed') => void;
  setRenderStyle: (style: RenderStyle) => void;
  setPlasticMaterial: (mat: PlasticMaterial) => void;
  setGroupingMode: (mode: GroupingMode) => void;
  setSlicingFilterMode: (mode: SlicingFilterMode) => void;
  setTargetSectionIndex: (idx: number) => void;
  setActiveCategory: (cat: string) => void;
  setSearchQuery: (q: string) => void;
  togglePlay: () => void;
  setPlaySpeed: (speed: number) => void;
  seekTime: (time: number) => void;
  setFocusedLine: (index: number | null) => void;
  jumpToSection: (sectionIndex: number) => void;
  nextSection: () => void;
  prevSection: () => void;
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
  segmentSections: [],
  groupingMode: 'auto',
  effectiveGroupingKind: 'revolution',
  metrics: null,
  optimizedToolpath: null,
  colorMode: 'type',
  renderStyle: 'beads',
  plasticMaterial: 'cyan',
  slicingFilterMode: 'all',
  targetSectionIndex: 1,
  activeCategory: 'all',
  searchQuery: '',
  isPlaying: false,
  currentTime: 0,
  maxTime: 10,
  playSpeed: 1.0,
  focusedLineIndex: null,
  activeSectionIndex: 1,

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
      activeSectionIndex: 1,
      targetSectionIndex: 1,
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
  setGroupingMode: (mode) => {
    set({ groupingMode: mode });
    const { gcodeLines, toolpath } = get();
    const { sections, effectiveKind, segmentSections } = computeIntelligentGrouping(
      gcodeLines,
      toolpath,
      mode
    );
    set({
      gcodeSections: sections,
      effectiveGroupingKind: effectiveKind,
      segmentSections,
      activeSectionIndex: 1,
      targetSectionIndex: 1,
    });
  },
  setSlicingFilterMode: (mode) => set({ slicingFilterMode: mode }),
  setTargetSectionIndex: (idx) => set({ targetSectionIndex: idx }),
  setActiveCategory: (cat) => set({ activeCategory: cat }),
  setSearchQuery: (q) => set({ searchQuery: q }),

  togglePlay: () => set((state) => ({ isPlaying: !state.isPlaying })),
  setPlaySpeed: (speed) => set({ playSpeed: speed }),
  seekTime: (time) => {
    const { gcodeSections, gcodeLines, maxTime } = get();
    const frac = maxTime > 0 ? Math.min(1.0, Math.max(0.0, time / maxTime)) : 0;
    const approxLine = Math.floor(frac * (gcodeLines.length - 1));

    let activeSec = 1;
    for (const sec of gcodeSections) {
      if (approxLine >= sec.line) {
        activeSec = sec.index;
      } else {
        break;
      }
    }

    set({ currentTime: time, activeSectionIndex: activeSec, targetSectionIndex: activeSec });
  },

  setFocusedLine: (index) => {
    if (index === null) {
      set({ focusedLineIndex: null });
      return;
    }
    const { gcodeSections } = get();
    let activeSec = 1;
    for (const sec of gcodeSections) {
      if (index >= sec.line) {
        activeSec = sec.index;
      } else {
        break;
      }
    }
    set({ focusedLineIndex: index, activeSectionIndex: activeSec, targetSectionIndex: activeSec });
  },

  jumpToSection: (sectionIndex) => {
    const { gcodeSections, gcodeLines, maxTime } = get();
    const sec = gcodeSections.find((s) => s.index === sectionIndex);
    if (!sec) return;
    const frac = gcodeLines.length > 0 ? sec.line / gcodeLines.length : 0;
    set({
      activeSectionIndex: sectionIndex,
      targetSectionIndex: sectionIndex,
      focusedLineIndex: sec.line,
      currentTime: frac * maxTime,
    });
  },

  nextSection: () => {
    const { activeSectionIndex, gcodeSections } = get();
    const maxSec = gcodeSections[gcodeSections.length - 1]?.index || 1;
    if (activeSectionIndex < maxSec) {
      get().jumpToSection(activeSectionIndex + 1);
    }
  },

  prevSection: () => {
    const { activeSectionIndex } = get();
    if (activeSectionIndex > 1) {
      get().jumpToSection(activeSectionIndex - 1);
    }
  },

  importCustomGcode: (text: string) => {
    try {
      const tp = importGcode(text);
      const lines = text.split('\n').filter((l) => l.trim().length > 0);
      const { sections, effectiveKind, segmentSections } = computeIntelligentGrouping(
        lines,
        tp,
        get().groupingMode
      );
      set({
        toolpath: tp,
        gcodeLines: lines,
        gcodeSections: sections,
        effectiveGroupingKind: effectiveKind,
        segmentSections,
        activeDesignKey: 'custom_import',
        currentTime: 0,
        maxTime: 10,
        focusedLineIndex: null,
        activeSectionIndex: 1,
        targetSectionIndex: 1,
      });
    } catch (err) {
      console.error('Import failed:', err);
    }
  },

  recompileCurrentDesign: () => {
    const { activeDesignKey, activeParams, groupingMode } = get();
    const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
    const def = allDefs[activeDesignKey];
    if (!def) return;

    const ops = typeof def.build === 'function' ? def.build(activeParams) : (def.ops || []);
    if (!ops || !ops.length) return;

    try {
      const gcode = compileGcode(ops, RESOLVE_PARAMS);
      const ir = compileIR(ops, RESOLVE_PARAMS);
      const m = compileMetrics(ops, RESOLVE_PARAMS);
      const { sections, effectiveKind, segmentSections } = computeIntelligentGrouping(
        gcode,
        ir,
        groupingMode
      );

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
        effectiveGroupingKind: effectiveKind,
        segmentSections,
        metrics: m,
        optimizedToolpath: optIr,
        maxTime: m.total_time_s || 10,
      });
    } catch (err) {
      console.error('Compilation error:', err);
    }
  },
}));
