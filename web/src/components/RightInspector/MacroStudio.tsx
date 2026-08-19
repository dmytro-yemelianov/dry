import React, { useState } from 'react';
import { useStudioStore } from '../../store/useStudioStore';
import { MACRO_LIBRARY, type MacroTarget, type MacroDef } from '../../data/macros';

export const MacroStudio: React.FC = () => {
  const [selectedMacroId, setSelectedMacroId] = useState<string>(MACRO_LIBRARY[0].id);
  const [targetFirmware, setTargetFirmware] = useState<MacroTarget>('klipper');
  const [macroParams, setMacroParams] = useState<Record<string, number>>({});
  const gcodeLines = useStudioStore((state) => state.gcodeLines);
  const importCustomGcode = useStudioStore((state) => state.importCustomGcode);

  const activeMacro = MACRO_LIBRARY.find((m) => m.id === selectedMacroId) || MACRO_LIBRARY[0];

  const getParamVal = (pId: string, defVal: number) => {
    return macroParams[pId] !== undefined ? macroParams[pId] : defVal;
  };

  const currentParamValues: Record<string, number> = {};
  activeMacro.params.forEach((p) => {
    currentParamValues[p.id] = getParamVal(p.id, p.defaultValue);
  });

  const generatedGcode = activeMacro.generateGcode(currentParamValues, targetFirmware);

  const handleInjectMacro = (position: 'start' | 'end') => {
    const injectedLines = generatedGcode.split('\n');
    let combined: string[];
    if (position === 'start') {
      combined = [...injectedLines, '', ...gcodeLines];
    } else {
      combined = [...gcodeLines, '', ...injectedLines];
    }
    importCustomGcode(combined.join('\n'), `injected_${activeMacro.id}.gcode`);
  };

  return (
    <div className="macro-studio-root" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      {/* Macro Category / Selector */}
      <div className="optimizer-card">
        <div className="opt-card-title">Production Macro Library</div>
        <div style={{ fontSize: '11px', color: 'var(--fg-muted)', marginBottom: '8px' }}>
          Select parameterized machine routines portable across all major firmwares:
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '6px' }}>
          {MACRO_LIBRARY.map((macro) => (
            <button
              key={macro.id}
              className={`sample-select-btn ${selectedMacroId === macro.id ? 'active' : ''}`}
              onClick={() => setSelectedMacroId(macro.id)}
            >
              <div style={{ fontWeight: 700, fontSize: '11px' }}>{macro.name}</div>
              <div style={{ fontSize: '9.5px', opacity: 0.7 }}>{macro.category}</div>
            </button>
          ))}
        </div>
      </div>

      {/* Target Firmware Selector */}
      <div className="optimizer-card">
        <div className="opt-card-title">Target Firmware / Output Dialect</div>
        <div className="opt-mode-pills">
          {(['klipper', 'marlin', 'bambu', 'dry_ir'] as MacroTarget[]).map((tgt) => (
            <button
              key={tgt}
              className={`opt-mode-btn ${targetFirmware === tgt ? 'active' : ''}`}
              onClick={() => setTargetFirmware(tgt)}
            >
              <span style={{ fontWeight: 700, textTransform: 'capitalize' }}>{tgt}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Parameter Sliders for Selected Macro */}
      <div className="optimizer-card">
        <div className="opt-card-title">Macro Parameters ({activeMacro.name})</div>
        <div style={{ fontSize: '11px', color: 'var(--fg-muted)', marginBottom: '8px' }}>
          {activeMacro.description}
        </div>
        <div className="param-fields-compact">
          {activeMacro.params.map((p) => {
            const val = getParamVal(p.id, p.defaultValue);
            return (
              <div key={p.id} className="param-row-compact">
                <div className="param-label-wrapper-compact">
                  <span className="param-label-compact">{p.label}</span>
                  <span className="param-val-badge-compact">
                    {val} {p.unit}
                  </span>
                </div>
                <div className="param-input-wrapper-compact">
                  <input
                    type="range"
                    className="param-slider-compact"
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    value={val}
                    onChange={(e) =>
                      setMacroParams({ ...macroParams, [p.id]: parseFloat(e.target.value) })
                    }
                  />
                  <input
                    type="number"
                    className="param-num-input-compact"
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    value={val}
                    onChange={(e) =>
                      setMacroParams({ ...macroParams, [p.id]: parseFloat(e.target.value) })
                    }
                  />
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Generated Macro Code Preview */}
      <div className="optimizer-card">
        <div className="opt-card-title">Compiled Macro Output ({targetFirmware.toUpperCase()})</div>
        <pre
          style={{
            background: 'var(--bg-app)',
            border: '1px solid var(--border)',
            borderRadius: '4px',
            padding: '8px',
            fontSize: '10.5px',
            fontFamily: 'ui-monospace, monospace',
            color: 'var(--fg-bright)',
            overflowX: 'auto',
            maxHeight: '130px',
          }}
        >
          {generatedGcode}
        </pre>
      </div>

      {/* Injection Actions */}
      <div style={{ display: 'flex', gap: '8px' }}>
        <button
          className="btn-action primary-cta"
          style={{ flex: 1 }}
          onClick={() => handleInjectMacro('start')}
        >
          ➕ Inject as Start G-Code
        </button>
        <button
          className="btn-action"
          style={{ flex: 1 }}
          onClick={() => handleInjectMacro('end')}
        >
          ➕ Inject as End G-Code
        </button>
      </div>
    </div>
  );
};
