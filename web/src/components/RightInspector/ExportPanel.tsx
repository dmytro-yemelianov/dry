import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY, RESOLVE_PARAMS } from '../../data/designs';
import {
  EXPORT_FORMATS,
  buildExportText,
  exportFilename,
  type MacroOptions,
} from '../../data/exporters';
import type { DesignDef } from '../../types/domain';

const MACRO_TOGGLES: Array<{ key: keyof MacroOptions; label: string; hint: string }> = [
  { key: 'header', label: 'Header', hint: 'Units, absolute XYZ, and the extrusion mode.' },
  { key: 'heat', label: 'Heat & wait', hint: 'M140/M104 then M190/M109.' },
  { key: 'home', label: 'Home axes', hint: 'G28 before the first move.' },
  { key: 'primeLine', label: 'Prime line', hint: 'Purge along the front edge of the bed.' },
  { key: 'primeBlob', label: 'Prime blob', hint: 'Purge in place, then retract.' },
  { key: 'present', label: 'Present print', hint: 'Lift and park so the part is reachable.' },
  { key: 'cooldown', label: 'Cooldown', hint: 'Nozzle, bed and fan to zero.' },
  { key: 'motorsOff', label: 'Motors off', hint: 'M84 at the end.' },
  { key: 'relativeE', label: 'Relative E', hint: 'M83 instead of M82.' },
];

export const ExportPanel: React.FC = () => {
  const exportFormat = useStudioStore((state) => state.exportFormat);
  const setExportFormat = useStudioStore((state) => state.setExportFormat);
  const macroOptions = useStudioStore((state) => state.macroOptions);
  const setMacroOption = useStudioStore((state) => state.setMacroOption);
  const gcodeLines = useStudioStore((state) => state.gcodeLines);
  const metrics = useStudioStore((state) => state.metrics);
  const activeMachine = useStudioStore((state) => state.activeMachine);
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const activeParams = useStudioStore((state) => state.activeParams);
  const [status, setStatus] = React.useState('');

  const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
  const def = allDefs[activeDesignKey];
  const designLabel = def?.title ?? def?.label ?? activeDesignKey;
  const ops = React.useMemo(() => {
    if (!def) return [];
    return def.ops ?? (def.build ? def.build(activeParams) : []);
  }, [def, activeParams]);

  const text = React.useMemo(
    () =>
      buildExportText({
        format: exportFormat,
        gcodeLines,
        ops,
        metrics,
        macros: macroOptions,
        machine: activeMachine,
        params: RESOLVE_PARAMS,
        designLabel,
      }),
    [exportFormat, gcodeLines, ops, metrics, macroOptions, activeMachine, designLabel],
  );

  const lineCount = text ? text.trimEnd().split('\n').length : 0;
  const showMacros = exportFormat === 'gcode-macros';

  const download = () => {
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = exportFilename(exportFormat, designLabel);
    a.click();
    URL.revokeObjectURL(url);
    setStatus(`saved ${exportFilename(exportFormat, designLabel)}`);
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setStatus('copied to clipboard');
    } catch {
      setStatus('clipboard unavailable');
    }
  };

  return (
    <div className="export-panel-root">
      <div className="param-row">
        <div className="param-label-wrapper">
          <label className="param-label param-name" htmlFor="exportFormat">
            Format
          </label>
        </div>
        <select
          id="exportFormat"
          className="machine-select"
          value={exportFormat}
          onChange={(e) => setExportFormat(e.target.value as typeof exportFormat)}
        >
          {EXPORT_FORMATS.map((f) => (
            <option key={f.id} value={f.id}>
              {f.label}
            </option>
          ))}
        </select>
      </div>

      {showMacros && (
        <div className="macro-options">
          <div className="panel-subhead">Start / finish macros</div>
          {MACRO_TOGGLES.map((t) => (
            <label key={t.key} className="macro-toggle" title={t.hint}>
              <input
                type="checkbox"
                id={`macro-${String(t.key)}`}
                checked={Boolean(macroOptions[t.key])}
                onChange={(e) => setMacroOption(t.key, e.target.checked as never)}
              />
              <span>{t.label}</span>
            </label>
          ))}
          <div className="macro-temps">
            <label>
              <span>Nozzle °C</span>
              <input
                type="number"
                min={0}
                max={activeMachine.max_hotend_temp ?? 500}
                value={macroOptions.nozzleTemp}
                onChange={(e) => setMacroOption('nozzleTemp', Number(e.target.value) as never)}
              />
            </label>
            <label>
              <span>Bed °C</span>
              <input
                type="number"
                min={0}
                max={activeMachine.max_bed_temp ?? 200}
                value={macroOptions.bedTemp}
                onChange={(e) => setMacroOption('bedTemp', Number(e.target.value) as never)}
              />
            </label>
          </div>
        </div>
      )}

      <div className="export-actions">
        <button className="btn-action primary" onClick={download} disabled={!text.trim()}>
          Download
        </button>
        <button className="btn-action" onClick={copy} disabled={!text.trim()}>
          Copy
        </button>
        <span id="exportStatus" className="export-status">
          {status || `${lineCount.toLocaleString()} lines ready`}
        </span>
      </div>

      <pre className="export-preview">{text.split('\n').slice(0, 40).join('\n')}</pre>
    </div>
  );
};
