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
  const groupingMode = useStudioStore((state) => state.groupingMode);
  const setGroupingMode = useStudioStore((state) => state.setGroupingMode);
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

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
      {/* Grouping Strategy Selector */}
      <div className="grouping-mode-bar">
        <span className="grouping-label">Group By:</span>
        <div className="grouping-pills">
          {(['auto', 'revolutions', 'figures', 'layers'] as GroupingMode[]).map((mode) => (
            <button
              key={mode}
              className={`grouping-pill ${groupingMode === mode ? 'active' : ''}`}
              onClick={() => setGroupingMode(mode)}
            >
              {mode === 'auto'
                ? `Auto (${sectionTitle}s)`
                : mode === 'revolutions'
                ? 'Turns / Helix'
                : mode === 'figures'
                ? 'Figures'
                : 'Layers'}
            </button>
          ))}
        </div>
      </div>

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

      {/* Virtualized Line Stream */}
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
            const line = gcodeLines[index] || '';
            const lineWords = line.trim().split(/\s+/);
            const lineCmd = lineWords[0] || '';
            const isG0 = lineCmd === 'G0';
            const isG1 = lineCmd === 'G1';
            const isArc = lineCmd === 'G2' || lineCmd === 'G3';
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
                  className={`gcode-row ${isActive ? 'active' : ''}`}
                  onMouseEnter={() => setHoveredLineIndex(index)}
                  onMouseLeave={() => setHoveredLineIndex(null)}
                  onClick={() => setFocusedLine(index)}
                >
                  <span className="gcode-lineno">{index + 1}</span>
                  <span className={`gcode-cmd ${cmdClass}`}>{lineCmd}</span>
                  <span className="gcode-args">{lineWords.slice(1).join(' ')}</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
