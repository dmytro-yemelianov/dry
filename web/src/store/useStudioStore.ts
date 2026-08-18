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
  GcodeViewFormat,
  RowGroupTags,
  GcodeRowMeta,
  HierarchyGroupNode,
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

/** Multi-Tag Auto-Grouping Engine: simultaneously tags moves with layers, figures, turns, and features */
function computeMultiTagGrouping(
  gcodeLines: string[],
  toolpath: Toolpath | null,
  requestedMode: GroupingMode
): {
  sections: GcodeSection[];
  effectiveKind: GroupingKind;
  segmentSections: number[];
  multiTagRows: GcodeRowMeta[];
  hierarchyTree: HierarchyGroupNode[];
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
    return {
      sections: [defaultSec],
      effectiveKind: 'routine',
      segmentSections: [],
      multiTagRows: [],
      hierarchyTree: [],
    };
  }

  // 1. Calculate centroid and bounding box for polar unrolling
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  let zChanges = 0;
  let continuousZChanges = 0;
  let lastZ: number | null = null;
  let totalExtrusionDistance = 0;

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
    if (seg.travel !== true && seg.kind !== 'travel') {
      totalExtrusionDistance += seg.length || 0;
    }
  }

  const cx = isFinite(minX) ? (minX + maxX) / 2 : 50;
  const cy = isFinite(minY) ? (minY + maxY) / 2 : 50;
  const isContinuousSpiral = continuousZChanges > 15 && continuousZChanges > zChanges * 0.7;

  // 2. Determine Primary Strategy for Main Section List
  let strategy: GroupingKind = 'layer';
  if (requestedMode === 'revolutions') strategy = 'revolution';
  else if (requestedMode === 'figures') strategy = 'figure';
  else if (requestedMode === 'layers') strategy = 'layer';
  else {
    // Auto-selection
    if (isContinuousSpiral) strategy = 'revolution';
    else if (zChanges > 1) strategy = 'layer';
    else strategy = 'figure';
  }

  // 3. Simultaneous Multi-Tagging of Segments
  const segmentTags: RowGroupTags[] = [];
  let currentLayer = 1;
  let currentLayerZ: number | null = null;
  let currentFigure = 0;
  let inFigureExtrusion = false;
  let cumulativeAngle = 0;
  let lastAngle: number | null = null;

  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i];
    const pt = seg.end || seg.start || [cx, cy, 0];
    const px = (pt[0] !== null ? (pt[0] as number) : cx) - cx;
    const py = (pt[1] !== null ? (pt[1] as number) : cy) - cy;
    const pz = pt[2] !== null ? (pt[2] as number) : 0;
    const roundedZ = Math.round(pz * 1000) / 1000;

    // Layer tag
    if (currentLayerZ === null || Math.abs(roundedZ - currentLayerZ) > 1e-4) {
      if (currentLayerZ !== null) currentLayer++;
      currentLayerZ = roundedZ;
    }

    // Figure tag
    const isTravel = seg.travel === true || seg.kind === 'travel';
    if (!isTravel) {
      if (!inFigureExtrusion) {
        currentFigure++;
        inFigureExtrusion = true;
      }
    } else {
      inFigureExtrusion = false;
    }

    // Revolution / Turn tag
    const angle = Math.atan2(py, px);
    if (lastAngle !== null) {
      let diff = angle - lastAngle;
      if (diff > Math.PI) diff -= TAU;
      if (diff < -Math.PI) diff += TAU;
      cumulativeAngle += Math.abs(diff);
    }
    lastAngle = angle;
    const currentTurn = Math.floor(cumulativeAngle / TAU) + 1;

    // Feature classification
    let featureType: 'perimeter' | 'infill' | 'travel' | 'skirt' | 'bridge' = 'infill';
    if (isTravel) featureType = 'travel';
    else if (seg.speed && seg.speed < 1200) featureType = 'perimeter';
    else featureType = 'infill';

    const tags: RowGroupTags = {
      layer: currentLayer,
      layerZ: currentLayerZ,
      figure: isTravel ? undefined : currentFigure,
      figureType: featureType,
      turn: currentTurn,
      turnAngleDeg: Math.round((cumulativeAngle * 180) / Math.PI),
      feature: isTravel ? 'Travel Move' : `${featureType.toUpperCase()} Move`,
    };

    seg.tags = tags;
    segmentTags.push(tags);
  }

  // 4. Map G-Code Lines to Multi-Tag Rows
  const lineFactor = gcodeLines.length > 0 && segments.length > 0 ? segments.length / gcodeLines.length : 1;
  const multiTagRows: GcodeRowMeta[] = gcodeLines.map((line, idx) => {
    const words = line.trim().split(/\s+/).filter(Boolean);
    const cmd = words[0] || '';
    const args: Record<string, string> = {};
    for (const tok of words.slice(1)) {
      const k = tok[0].toUpperCase();
      const v = tok.slice(1);
      if (k && v !== undefined) args[k] = v;
    }

    const segIdx = Math.min(segments.length - 1, Math.floor(idx * lineFactor));
    const tags = segmentTags[segIdx] || { layer: 1, figure: 1, turn: 1 };
    return {
      index: idx,
      raw: line,
      cmd,
      args,
      tags,
    };
  });

  // 5. Build Standard Sections based on strategy
  const sections: GcodeSection[] = [];
  const segmentSections: number[] = [];

  if (strategy === 'revolution') {
    let currentTurn = 1;
    for (let i = 0; i < segments.length; i++) {
      const turn = segmentTags[i].turn || 1;
      if (turn >= currentTurn || i === 0) {
        const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i / lineFactor));
        sections.push({
          index: turn,
          line: lineIdx,
          segmentIndex: i,
          kind: 'revolution',
          label: `Turn ${turn} (${(turn * 360 - 360)}°–${turn * 360}°)`,
          subLabel: `Z: ${segmentTags[i].layerZ?.toFixed(2)}mm`,
        });
        currentTurn = turn + 1;
      }
      segmentSections.push(sections.length);
    }
  } else if (strategy === 'figure') {
    let currentFig = 0;
    for (let i = 0; i < segments.length; i++) {
      const fig = segmentTags[i].figure || 0;
      if (fig > currentFig) {
        currentFig = fig;
        const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i / lineFactor));
        sections.push({
          index: currentFig,
          line: lineIdx,
          segmentIndex: i,
          kind: 'figure',
          label: `Figure ${currentFig} (${segmentTags[i].figureType || 'Extrusion'})`,
          subLabel: `Z: ${segmentTags[i].layerZ?.toFixed(2)}mm`,
        });
      }
      segmentSections.push(Math.max(1, sections.length));
    }
  } else {
    let curLayer = 0;
    for (let i = 0; i < segments.length; i++) {
      const layer = segmentTags[i].layer || 1;
      if (layer > curLayer) {
        curLayer = layer;
        const lineIdx = Math.min(gcodeLines.length - 1, Math.floor(i / lineFactor));
        sections.push({
          index: curLayer,
          line: lineIdx,
          segmentIndex: i,
          kind: 'layer',
          label: `Layer ${curLayer} (Z=${segmentTags[i].layerZ?.toFixed(2)}mm)`,
          subLabel: `${segmentTags[i].layerZ?.toFixed(3)} mm`,
        });
      }
      segmentSections.push(curLayer);
    }
  }

  // 6. Build Hierarchical Grouping Tree (Layer -> Figure -> Moves)
  const layerMap = new Map<number, HierarchyGroupNode>();
  for (let idx = 0; idx < multiTagRows.length; idx++) {
    const row = multiTagRows[idx];
    const lNum = row.tags.layer || 1;
    if (!layerMap.has(lNum)) {
      layerMap.set(lNum, {
        id: `layer-${lNum}`,
        kind: 'layer',
        label: `Layer ${lNum} (Z=${row.tags.layerZ?.toFixed(2) || '0.20'}mm)`,
        badge: `L${lNum}`,
        startLine: idx,
        endLine: idx,
        startSeg: Math.floor(idx * lineFactor),
        endSeg: Math.floor(idx * lineFactor),
        lineCount: 0,
        z: row.tags.layerZ,
        children: [],
      });
    }
    const lNode = layerMap.get(lNum)!;
    lNode.endLine = idx;
    lNode.lineCount++;
  }

  const hierarchyTree = Array.from(layerMap.values());

  return {
    sections: sections.length ? sections : [{ index: 1, line: 0, segmentIndex: 0, kind: 'layer', label: 'Layer 1' }],
    effectiveKind: strategy,
    segmentSections,
    multiTagRows,
    hierarchyTree,
  };
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
  multiTagRows: GcodeRowMeta[];
  hierarchyTree: HierarchyGroupNode[];
  groupingMode: GroupingMode;
  effectiveGroupingKind: GroupingKind;
  activeFilterLayers: number[];
  activeFilterFigures: number[];
  activeFilterTurns: number[];
  metrics: Metrics | null;
  optimizedToolpath: Toolpath | null;
  colorMode: 'type' | 'height' | 'speed';
  renderStyle: RenderStyle;
  plasticMaterial: PlasticMaterial;
  slicingFilterMode: SlicingFilterMode;
  targetSectionIndex: number;
  gcodeViewFormat: GcodeViewFormat;
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
  setGcodeViewFormat: (format: GcodeViewFormat) => void;
  toggleFilterLayer: (layer: number) => void;
  toggleFilterFigure: (figure: number) => void;
  toggleFilterTurn: (turn: number) => void;
  clearMultiFilters: () => void;
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
  multiTagRows: [],
  hierarchyTree: [],
  groupingMode: 'auto',
  effectiveGroupingKind: 'revolution',
  activeFilterLayers: [],
  activeFilterFigures: [],
  activeFilterTurns: [],
  metrics: null,
  optimizedToolpath: null,
  colorMode: 'type',
  renderStyle: 'beads',
  plasticMaterial: 'cyan',
  slicingFilterMode: 'all',
  targetSectionIndex: 1,
  gcodeViewFormat: 'stream',
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
      activeFilterLayers: [],
      activeFilterFigures: [],
      activeFilterTurns: [],
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
    const { sections, effectiveKind, segmentSections, multiTagRows, hierarchyTree } = computeMultiTagGrouping(
      gcodeLines,
      toolpath,
      mode
    );
    set({
      gcodeSections: sections,
      effectiveGroupingKind: effectiveKind,
      segmentSections,
      multiTagRows,
      hierarchyTree,
      activeSectionIndex: 1,
      targetSectionIndex: 1,
    });
  },
  setSlicingFilterMode: (mode) => set({ slicingFilterMode: mode }),
  setTargetSectionIndex: (idx) => set({ targetSectionIndex: idx }),
  setGcodeViewFormat: (format) => set({ gcodeViewFormat: format }),

  toggleFilterLayer: (layer) => {
    set((state) => {
      const exists = state.activeFilterLayers.includes(layer);
      const next = exists
        ? state.activeFilterLayers.filter((l) => l !== layer)
        : [...state.activeFilterLayers, layer];
      return {
        activeFilterLayers: next,
        slicingFilterMode: next.length > 0 || state.activeFilterFigures.length > 0 || state.activeFilterTurns.length > 0 ? 'multiFilter' : 'all',
      };
    });
  },

  toggleFilterFigure: (fig) => {
    set((state) => {
      const exists = state.activeFilterFigures.includes(fig);
      const next = exists
        ? state.activeFilterFigures.filter((f) => f !== fig)
        : [...state.activeFilterFigures, fig];
      return {
        activeFilterFigures: next,
        slicingFilterMode: next.length > 0 || state.activeFilterLayers.length > 0 || state.activeFilterTurns.length > 0 ? 'multiFilter' : 'all',
      };
    });
  },

  toggleFilterTurn: (turn) => {
    set((state) => {
      const exists = state.activeFilterTurns.includes(turn);
      const next = exists
        ? state.activeFilterTurns.filter((t) => t !== turn)
        : [...state.activeFilterTurns, turn];
      return {
        activeFilterTurns: next,
        slicingFilterMode: next.length > 0 || state.activeFilterLayers.length > 0 || state.activeFilterFigures.length > 0 ? 'multiFilter' : 'all',
      };
    });
  },

  clearMultiFilters: () => {
    set({
      activeFilterLayers: [],
      activeFilterFigures: [],
      activeFilterTurns: [],
      slicingFilterMode: 'all',
    });
  },

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
      const { sections, effectiveKind, segmentSections, multiTagRows, hierarchyTree } = computeMultiTagGrouping(
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
        multiTagRows,
        hierarchyTree,
        activeDesignKey: 'custom_import',
        currentTime: 0,
        maxTime: 10,
        focusedLineIndex: null,
        activeSectionIndex: 1,
        targetSectionIndex: 1,
        activeFilterLayers: [],
        activeFilterFigures: [],
        activeFilterTurns: [],
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
      const { sections, effectiveKind, segmentSections, multiTagRows, hierarchyTree } = computeMultiTagGrouping(
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
        multiTagRows,
        hierarchyTree,
        metrics: m,
        optimizedToolpath: optIr,
        maxTime: m.total_time_s || 10,
      });
    } catch (err) {
      console.error('Compilation error:', err);
    }
  },
}));
