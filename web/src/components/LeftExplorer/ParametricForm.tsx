import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY } from '../../data/designs';

export const ParametricForm: React.FC = () => {
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const activeParams = useStudioStore((state) => state.activeParams);
  const updateParam = useStudioStore((state) => state.updateParam);
  const resetParams = useStudioStore((state) => state.resetParams);

  const allDefs = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
  const def = allDefs[activeDesignKey];

  if (!def) {
    return <div style={{ fontSize: '12px', color: 'var(--fg-muted)', padding: '14px' }}>Select a design to inspect.</div>;
  }

  const isParametric = (def.params && def.params.length > 0);

  return (
    <div className="parametric-form-root">
      <div style={{ marginBottom: '14px', paddingBottom: '10px', borderBottom: '1px solid var(--border)' }}>
        <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--fg-bright)' }}>{def.label}</div>
        <div style={{ fontSize: '11px', color: 'var(--fg-muted)', marginTop: '2px' }}>
          {def.group} · {isParametric ? `${def.params.length} adjustable parameters` : 'Fixed reference geometry'}
        </div>
      </div>

      {isParametric ? (
        <div className="param-fields">
          {def.params.map((p) => {
            const val = activeParams[p.id] ?? p.defaultValue;
            return (
              <div key={p.id} className="param-row">
                <div className="param-label-wrapper">
                  <label className="param-label">{p.label}</label>
                  <span className="param-val-badge">
                    {val} {p.unit}
                  </span>
                </div>
                <div className="param-input-wrapper">
                  <input
                    type="range"
                    className="param-slider"
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    value={val}
                    onChange={(e) => updateParam(p.id, parseFloat(e.target.value))}
                  />
                  <input
                    type="number"
                    className="param-num-input"
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    value={val}
                    onChange={(e) => updateParam(p.id, parseFloat(e.target.value))}
                  />
                </div>
              </div>
            );
          })}

          <div style={{ marginTop: '16px', paddingTop: '12px', borderTop: '1px solid var(--border)' }}>
            <button onClick={resetParams} className="btn-action" style={{ width: '100%', justifyContent: 'center' }}>
              Reset to Defaults
            </button>
          </div>
        </div>
      ) : (
        <div style={{ fontSize: '12px', color: 'var(--fg-muted)', padding: '12px 0' }}>
          This FullControl paper reference model uses canonical fixed toolpath operations.
        </div>
      )}
    </div>
  );
};
