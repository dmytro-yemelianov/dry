import React, { useRef, useState, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useStudioStore } from '../../store/useStudioStore';
import type { GroupingMode } from '../../types/domain';

const CMD_DESC: Record<string, string> = {
  G0: 'rapid travel — reposition without extruding',
  G1: 'linear move — extrude in a straight line',
  G2: 'clockwise circular arc',
  G3: 'counter-clockwise circular arc',
  G4: 'dwell — pause in place',
  M3: 'spindle on (clockwise) / laser active',
  M4: 'spindle on (counter-clockwise)',
  M5: 'spindle / laser off',
  M104: 'set extruder temperature',
  M109: 'wait for extruder temperature',
  M140: 'set bed temperature',
  M190: 'wait for bed temperature',
  M106: 'set cooling fan speed',
  M107: 'turn cooling fan off',
  T: 'tool change command',
};

const PARAM_DESC: Record<string, [string, string]> = {
  F: ['feedrate', 'mm/min'],
  X: ['target X coordinate', 'mm'],
  Y: ['target Y coordinate', 'mm'],
  Z: ['target Z coordinate / layer height', 'mm'],
  E: ['extrusion amount', 'mm of filament'],
  I: ['arc center ΔX offset', 'mm'],
  J: ['arc center ΔY offset', 'mm'],
  A: ['rotary A axis angle', 'deg'],
  B: ['rotary B axis angle', 'deg'],
  C: ['rotary C axis angle', 'deg'],
  S: ['dwell time / spindle RPM / laser PWM', 's / RPM / PWM'],
  P: ['dwell time in ms', 'ms'],
};

export const GcodeInspector: React.FC = () => {
  const gcodeLines = useStudioStore((state) => state.gcodeLines);
  const gcodeSections = useStudioStore((state) => state.gcodeSections);
  const multiTagRows = useStudioStore((state) => state.multiTagRows);
  const groupingMode = useStudioStore((state) => state.groupingMode);
  const setGroupingMode = useStudioStore((state) => state.setGroupingMode);
  const gcodeViewFormat = useStudioStore((state) => state.gcodeViewFormat);
  const setGcodeViewFormat = useStudioStore((state) => state.setGcodeViewFormat);
  const effectiveGroupingKind = useStudioStore((state) => state.effectiveGroupingKind);
  const focusedLineIndex = useStudioStore((state) => state.focusedLineIndex);
  const setFocusedLine = useStudioStore((state) => state.setFocusedLine);
  const currentTime = useStudioStore((state) => state.currentTime);
  const maxTime = useStudioStore((state) => state.maxTime);
  const isPlaying = useStudioStore((state) => state.isPlaying);
  const activeSectionIndex = useStudioStore((state) => state.activeSectionIndex);
  const jumpToSection = useStudioStore((state) => state.jumpToSection);
  const nextSection = useStudioStore((state) => state.nextSection);
  const prevSection = useStudioStore((state) => state.prevSection);

  const activeFilterLayers = useStudioStore((state) => state.activeFilterLayers);
  const activeFilterFigures = useStudioStore((state) => state.activeFilterFigures);
  const activeFilterTurns = useStudioStore((state) => state.activeFilterTurns);
  const toggleFilterLayer = useStudioStore((state) => state.toggleFilterLayer);
  const toggleFilterFigure = useStudioStore((state) => state.toggleFilterFigure);
  const toggleFilterTurn = useStudioStore((state) => state.toggleFilterTurn);
  const clearMultiFilters = useStudioStore((state) => state.clearMultiFilters);

  const [hoveredLineIndex, setHoveredLineIndex] = useState<number | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);

  // Map each line index to its section header if it starts a section
  const sectionStartMap = useRef<Map<number, { label: string; kind: string }>>(new Map());
  useEffect(() => {
    const map = new Map<number, { label: string; kind: string }>();
    gcodeSections.forEach((sec) => {
      map.set(sec.line, { label: sec.label, kind: sec.kind });
    });
    sectionStartMap.current = map;
  }, [gcodeSections]);

  const activeLineIndex =
    focusedLineIndex !== null
      ? focusedLineIndex
      : gcodeLines.length > 0
      ? Math.min(gcodeLines.length - 1, Math.floor((currentTime / (maxTime || 1)) * gcodeLines.length))
      : 0;

  const currentDisplayIndex = hoveredLineIndex !== null ? hoveredLineIndex : activeLineIndex;
  const currentDisplayLine = gcodeLines[currentDisplayIndex] || gcodeLines[0] || '';
  const currentRowMeta = multiTagRows[currentDisplayIndex] || {
    index: currentDisplayIndex,
    raw: currentDisplayLine,
    cmd: '',
    args: {},
    tags: {},
  };

  const virtualizer = useVirtualizer({
    count: gcodeLines.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 24,
    overscan: 25,
  });

  useEffect(() => {
    if (isPlaying && activeLineIndex >= 0 && activeLineIndex < gcodeLines.length) {
      virtualizer.scrollToIndex(activeLineIndex, { align: 'auto' });
    }
  }, [isPlaying, activeLineIndex, virtualizer, gcodeLines.length]);

  const words = currentDisplayLine.trim().split(/\s+/).filter(Boolean);
  const cmd = words[0] || '';
  const cmdDesc = CMD_DESC[cmd] || 'G-code command';

  const maxSection = gcodeSections[gcodeSections.length - 1]?.index || 1;
  const sectionTitle =
    effectiveGroupingKind === 'revolution'
      ? 'Turn'
      : effectiveGroupingKind === 'figure'
      ? 'Figure'
      : 'Layer';

  const hasActiveFilters =
    activeFilterLayers.length > 0 || activeFilterFigures.length > 0 || activeFilterTurns.length > 0;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
      {/* Grouping Strategy & View Format Switcher Bar */}
      <div className="grouping-mode-bar">
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <span className="grouping-label">Group:</span>
          <div className="grouping-pills">
            {(['auto', 'revolutions', 'figures', 'layers'] as GroupingMode[]).map((mode) => (
              <button
                key={mode}
                className={`grouping-pill ${groupingMode === mode ? 'active' : ''}`}
                onClick={() => setGroupingMode(mode)}
              >
                {mode === 'auto'
                  ? `Auto Gantt`
                  : mode === 'revolutions'
                  ? 'Turns'
                  : mode === 'figures'
                  ? 'Figures'
                  : 'Layers'}
              </button>
            ))}
          </div>
        </div>

        <div className="format-pills">
          <button
            className={`format-pill ${gcodeViewFormat === 'stream' ? 'active' : ''}`}
            onClick={() => setGcodeViewFormat('stream')}
            title="Raw G-code stream view with Vertical Gantt"
          >
            Stream
          </button>
          <button
            className={`format-pill ${gcodeViewFormat === 'table' ? 'active' : ''}`}
            onClick={() => setGcodeViewFormat('table')}
            title="Tabular coordinate matrix view with Vertical Gantt"
          >
            Table Matrix
          </button>
        </div>
      </div>

      {/* Breadcrumbs Trail */}
      <div className="breadcrumb-trail-bar">
        <span className="bc-item">
          L{currentRowMeta.tags.layer || 1} (Z={currentRowMeta.tags.layerZ?.toFixed(2) || '0.20'}mm)
        </span>
        {currentRowMeta.tags.figure && (
          <>
            <span className="bc-sep">›</span>
            <span className="bc-item">
              Fig {currentRowMeta.tags.figure} ({currentRowMeta.tags.figureType || 'extrude'})
            </span>
          </>
        )}
        {currentRowMeta.tags.turn && (
          <>
            <span className="bc-sep">›</span>
            <span className="bc-item">Turn {currentRowMeta.tags.turn}</span>
          </>
        )}
        <span className="bc-sep">›</span>
        <span className="bc-active">Move #{currentDisplayIndex + 1}</span>
      </div>

      {/* Multi-Filter Active Indicators */}
      {hasActiveFilters && (
        <div className="active-filters-bar">
          <span className="filter-badge-label">Active Filters:</span>
          {activeFilterLayers.map((l) => (
            <span key={`l-${l}`} className="filter-chip" onClick={() => toggleFilterLayer(l)}>
              Layer {l} ✕
            </span>
          ))}
          {activeFilterFigures.map((f) => (
            <span key={`f-${f}`} className="filter-chip" onClick={() => toggleFilterFigure(f)}>
              Fig {f} ✕
            </span>
          ))}
          {activeFilterTurns.map((t) => (
            <span key={`t-${t}`} className="filter-chip" onClick={() => toggleFilterTurn(t)}>
              Turn {t} ✕
            </span>
          ))}
          <button className="clear-filter-btn" onClick={clearMultiFilters}>
            Clear All
          </button>
        </div>
      )}

      {/* Multi-Modal Section Navigation Bar */}
      <div className="layer-nav-bar">
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <button className="layer-step-btn" onClick={prevSection} disabled={activeSectionIndex <= 1}>
            ⏮
          </button>
          <span className="layer-badge">
            {sectionTitle} {activeSectionIndex} / {maxSection}
          </span>
          <button className="layer-step-btn" onClick={nextSection} disabled={activeSectionIndex >= maxSection}>
            ⏭
          </button>
        </div>

        <select
          className="layer-select-dropdown"
          value={activeSectionIndex}
          onChange={(e) => jumpToSection(parseInt(e.target.value, 10))}
        >
          {gcodeSections.map((sec) => (
            <option key={sec.index} value={sec.index}>
              {sec.label}
            </option>
          ))}
        </select>
      </div>

      {/* Top Decoded Token Explanation Card */}
      <div className="gcode-explainer-card">
        <div className="exp-header">
          <span className="exp-cmd">{cmd}</span> — <span>{cmdDesc}</span>
        </div>
        {words.length > 1 && (
          <table className="exp-table">
            <tbody>
              {words.slice(1).map((tok) => {
                const k = tok[0];
                const v = tok.slice(1);
                const pInfo = PARAM_DESC[k];
                return (
                  <tr key={tok}>
                    <td className="exp-k">{tok}</td>
                    <td className="exp-desc">
                      {pInfo ? `${pInfo[0]} (${pInfo[1]})` : 'parameter'} = <b>{v}</b>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Dedicated Gantt Header Bar with Horizontally Aligned Column Names */}
      <div className="gcode-table-header">
        <div className="gantt-header-gutter">
          <span className="gantt-hdr-col layer-hdr" title="Layer Schedule">Layer</span>
          <span className="gantt-hdr-col fig-hdr" title="Figure & Loop Schedule">Figure</span>
          <span className="gantt-hdr-col turn-hdr" title="Revolution / Turn Schedule">Turn</span>
        </div>
        <span className="col-no">Line</span>
        {gcodeViewFormat === 'table' ? (
          <>
            <span className="col-cmd">Op</span>
            <span className="col-x">X (mm)</span>
            <span className="col-y">Y (mm)</span>
            <span className="col-z">Z (mm)</span>
            <span className="col-e">E (mm)</span>
            <span className="col-f">F (mm/min)</span>
          </>
        ) : (
          <span className="col-stream-hdr">Command & Coordinates</span>
        )}
      </div>

      {/* Virtualized G-Code Stream or Table Matrix with Vertical Gantt Chart */}
      <div
        ref={parentRef}
        className="gcode-viewer-table"
        style={{ flex: 1, minHeight: 0, overflowY: 'auto' }}
      >
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: '100%',
            position: 'relative',
          }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const index = virtualRow.index;
            const rowMeta = multiTagRows[index] || {
              index,
              raw: gcodeLines[index] || '',
              cmd: '',
              args: {},
              tags: {},
            };
            const prevRow = index > 0 ? multiTagRows[index - 1] : null;
            const nextRow = index < multiTagRows.length - 1 ? multiTagRows[index + 1] : null;

            // Track 1: Layer
            const layerVal = rowMeta.tags.layer || 1;
            const lStart = !prevRow || prevRow.tags.layer !== layerVal;
            const lEnd = !nextRow || nextRow.tags.layer !== layerVal;

            // Track 2: Figure
            const figVal = rowMeta.tags.figure;
            const isTravel = figVal === undefined;
            const figStart = !isTravel && (!prevRow || prevRow.tags.figure !== figVal);
            const figEnd = !isTravel && (!nextRow || nextRow.tags.figure !== figVal);

            // Track 3: Turn
            const turnVal = rowMeta.tags.turn || 1;
            const turnStart = !prevRow || prevRow.tags.turn !== turnVal;
            const turnEnd = !nextRow || nextRow.tags.turn !== turnVal;

            const isG0 = rowMeta.cmd === 'G0';
            const isG1 = rowMeta.cmd === 'G1';
            const isArc = rowMeta.cmd === 'G2' || rowMeta.cmd === 'G3';
            const cmdClass = isG0 ? 'cmd-g0' : isG1 ? 'cmd-g1' : isArc ? 'cmd-arc' : 'cmd-other';
            const isActive = index === activeLineIndex;
            const sectionInfo = sectionStartMap.current.get(index);

            const icon =
              sectionInfo?.kind === 'revolution'
                ? '🔄'
                : sectionInfo?.kind === 'figure'
                ? '🔷'
                : '📌';

            return (
              <div
                key={index}
                data-index={index}
                ref={virtualizer.measureElement}
                className="gcode-row-container"
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                {sectionInfo && (
                  <div className="gcode-section-header">
                    <span>{icon} {sectionInfo.label}</span>
                  </div>
                )}

                <div
                  className={`gcode-row-wrapper ${gcodeViewFormat === 'table' ? 'table-matrix-row' : 'gcode-row'} ${isActive ? 'active' : ''}`}
                  onMouseEnter={() => setHoveredLineIndex(index)}
                  onMouseLeave={() => setHoveredLineIndex(null)}
                  onClick={() => setFocusedLine(index)}
                >
                  {/* Vertical Gantt Chart Gutter */}
                  <div className="vertical-gantt-gutter">
                    {/* Layer Track */}
                    <div
                      className={`gantt-track-cell layer-track ${lStart ? 'is-start' : ''} ${lEnd ? 'is-end' : ''} ${activeFilterLayers.includes(layerVal) ? 'active-filter' : ''}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFilterLayer(layerVal);
                      }}
                      title={`Layer ${layerVal} (Z=${rowMeta.tags.layerZ?.toFixed(2)}mm)`}
                    >
                      <div className="gantt-rail-stem" />
                      {lStart && <span className="gantt-block-badge">L{layerVal}</span>}
                      {lEnd && !lStart && <span className="gantt-end-node" />}
                    </div>

                    {/* Figure Track */}
                    <div
                      className={`gantt-track-cell fig-track ${figStart ? 'is-start' : ''} ${figEnd ? 'is-end' : ''} ${isTravel ? 'is-travel' : ''} ${figVal && activeFilterFigures.includes(figVal) ? 'active-filter' : ''}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        if (figVal) toggleFilterFigure(figVal);
                      }}
                      title={isTravel ? 'Travel Move (Rapid)' : `Figure ${figVal} (${rowMeta.tags.figureType || 'extrude'})`}
                    >
                      {isTravel ? (
                        <div className="gantt-travel-dash" />
                      ) : (
                        <>
                          <div className="gantt-rail-stem" />
                          {figStart && <span className="gantt-block-badge">F{figVal}</span>}
                          {figEnd && !figStart && <span className="gantt-end-node" />}
                        </>
                      )}
                    </div>

                    {/* Turn Track */}
                    <div
                      className={`gantt-track-cell turn-track ${turnStart ? 'is-start' : ''} ${turnEnd ? 'is-end' : ''} ${activeFilterTurns.includes(turnVal) ? 'active-filter' : ''}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFilterTurn(turnVal);
                      }}
                      title={`Turn ${turnVal}`}
                    >
                      <div className="gantt-rail-stem" />
                      {turnStart && <span className="gantt-block-badge">T{turnVal}</span>}
                      {turnEnd && !turnStart && <span className="gantt-end-node" />}
                    </div>
                  </div>

                  <span className="col-no">{index + 1}</span>

                  {gcodeViewFormat === 'stream' ? (
                    /* ---- Stream Mode ---- */
                    <>
                      <span className={`gcode-cmd ${cmdClass}`}>{rowMeta.cmd}</span>
                      <span className="gcode-args">{rowMeta.raw.split(/\s+/).slice(1).join(' ')}</span>
                    </>
                  ) : (
                    /* ---- Tabular Coordinate Matrix Mode ---- */
                    <>
                      <span className={`col-cmd ${cmdClass}`}>{rowMeta.cmd}</span>
                      <span className="col-x">{rowMeta.args.X || '—'}</span>
                      <span className="col-y">{rowMeta.args.Y || '—'}</span>
                      <span className="col-z">{rowMeta.args.Z || '—'}</span>
                      <span className="col-e">{rowMeta.args.E || '—'}</span>
                      <span className="col-f">{rowMeta.args.F || '—'}</span>
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
