import React, { useRef, useState, useEffect } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useStudioStore } from '../../store/useStudioStore';

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
  const focusedLineIndex = useStudioStore((state) => state.focusedLineIndex);
  const setFocusedLine = useStudioStore((state) => state.setFocusedLine);
  const currentTime = useStudioStore((state) => state.currentTime);
  const maxTime = useStudioStore((state) => state.maxTime);
  const isPlaying = useStudioStore((state) => state.isPlaying);

  const [hoveredLineIndex, setHoveredLineIndex] = useState<number | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);

  // Active line index from playback or focus
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
    overscan: 20,
  });

  // Auto-scroll on playback
  useEffect(() => {
    if (isPlaying && activeLineIndex >= 0 && activeLineIndex < gcodeLines.length) {
      virtualizer.scrollToIndex(activeLineIndex, { align: 'auto' });
    }
  }, [isPlaying, activeLineIndex, virtualizer, gcodeLines.length]);

  // Decode Active G-Code Line
  const words = currentDisplayLine.trim().split(/\s+/).filter(Boolean);
  const cmd = words[0] || '';
  const cmdDesc = CMD_DESC[cmd] || 'G-code command';

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }}>
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
        style={{ flex: 1, overflowY: 'auto' }}
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

            return (
              <div
                key={index}
                className={`gcode-row ${isActive ? 'active' : ''}`}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
                onMouseEnter={() => setHoveredLineIndex(index)}
                onMouseLeave={() => setHoveredLineIndex(null)}
                onClick={() => setFocusedLine(index)}
              >
                <span className="gcode-lineno">{index + 1}</span>
                <span className={`gcode-cmd ${cmdClass}`}>{lineCmd}</span>
                <span className="gcode-args">{lineWords.slice(1).join(' ')}</span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
