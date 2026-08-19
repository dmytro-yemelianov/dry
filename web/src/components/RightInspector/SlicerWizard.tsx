import React, { useState } from 'react';

export const SlicerWizard: React.FC = () => {
  const [selectedSlicer, setSelectedSlicer] = useState<'orca' | 'bambu' | 'prusa' | 'cura'>('orca');
  const [selectedOS, setSelectedOS] = useState<'mac' | 'linux' | 'win'>('mac');

  const getCliCommand = () => {
    if (selectedOS === 'win') {
      return `"C:\\Program Files\\DryMachina\\dry.exe" optimize --mode balanced`;
    }
    return `"/usr/local/bin/dry" optimize --mode balanced`;
  };

  return (
    <div className="slicer-wizard-root" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      {/* Slicer Selector */}
      <div className="optimizer-card">
        <div className="opt-card-title">Select Your 3D Slicer</div>
        <div className="opt-mode-pills">
          <button
            className={`opt-mode-btn ${selectedSlicer === 'orca' ? 'active' : ''}`}
            onClick={() => setSelectedSlicer('orca')}
          >
            <span style={{ fontWeight: 700 }}>OrcaSlicer</span>
          </button>
          <button
            className={`opt-mode-btn ${selectedSlicer === 'bambu' ? 'active' : ''}`}
            onClick={() => setSelectedSlicer('bambu')}
          >
            <span style={{ fontWeight: 700 }}>Bambu Studio</span>
          </button>
          <button
            className={`opt-mode-btn ${selectedSlicer === 'prusa' ? 'active' : ''}`}
            onClick={() => setSelectedSlicer('prusa')}
          >
            <span style={{ fontWeight: 700 }}>PrusaSlicer</span>
          </button>
          <button
            className={`opt-mode-btn ${selectedSlicer === 'cura' ? 'active' : ''}`}
            onClick={() => setSelectedSlicer('cura')}
          >
            <span style={{ fontWeight: 700 }}>UltiMaker Cura</span>
          </button>
        </div>
      </div>

      {/* Operating System Selector */}
      <div className="optimizer-card">
        <div className="opt-card-title">Operating System</div>
        <div className="opt-mode-pills">
          <button
            className={`opt-mode-btn ${selectedOS === 'mac' ? 'active' : ''}`}
            onClick={() => setSelectedOS('mac')}
          >
            macOS (Apple Silicon / Intel)
          </button>
          <button
            className={`opt-mode-btn ${selectedOS === 'linux' ? 'active' : ''}`}
            onClick={() => setSelectedOS('linux')}
          >
            Linux (x86_64 / ARM64)
          </button>
          <button
            className={`opt-mode-btn ${selectedOS === 'win' ? 'active' : ''}`}
            onClick={() => setSelectedOS('win')}
          >
            Windows (10 / 11)
          </button>
        </div>
      </div>

      {/* Step-by-Step Drop-in Setup Instructions */}
      <div className="optimizer-card">
        <div className="opt-card-title">
          {selectedSlicer === 'cura'
            ? 'Cura Post-Processing Plugin Setup'
            : `Drop-in Post-Processing Hook (${selectedSlicer.toUpperCase()})`}
        </div>

        {selectedSlicer === 'cura' ? (
          <div style={{ fontSize: '11px', color: 'var(--fg)', lineHeight: '1.6' }}>
            <p>1. Copy <code>DryOptimizer.py</code> to your Cura scripts directory:</p>
            <pre
              style={{
                background: 'var(--bg-app)',
                border: '1px solid var(--border)',
                borderRadius: '4px',
                padding: '6px 8px',
                fontSize: '10.5px',
                fontFamily: 'ui-monospace, monospace',
                color: 'var(--accent)',
              }}
            >
              {selectedOS === 'mac'
                ? '~/Library/Application Support/cura/<version>/scripts/DryOptimizer.py'
                : selectedOS === 'win'
                ? '%APPDATA%\\cura\\<version>\\scripts\\DryOptimizer.py'
                : '~/.local/share/cura/<version>/scripts/DryOptimizer.py'}
            </pre>
            <p style={{ marginTop: '8px' }}>
              2. In Cura, navigate to <b>Extensions › Post Processing › Modify G-Code › Add a script › Dry Machina Optimizer</b>.
            </p>
            <p>3. Select your desired optimization level (<b>Safe</b>, <b>Balanced</b>, or <b>Max</b>).</p>
          </div>
        ) : (
          <div style={{ fontSize: '11px', color: 'var(--fg)', lineHeight: '1.6' }}>
            <p>
              1. Open <b>{selectedSlicer === 'bambu' ? 'Bambu Studio' : selectedSlicer === 'orca' ? 'OrcaSlicer' : 'PrusaSlicer'}</b>.
            </p>
            <p>
              2. Go to <b>Print Settings › {selectedSlicer === 'prusa' ? 'Output options' : 'Others'} › Post-processing scripts</b>.
            </p>
            <p>3. Paste the following executable command line:</p>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                background: 'var(--bg-app)',
                border: '1px solid var(--border)',
                borderRadius: '4px',
                padding: '6px 8px',
                marginTop: '4px',
              }}
            >
              <code style={{ flex: 1, fontSize: '10.5px', color: 'var(--accent)', fontFamily: 'ui-monospace, monospace' }}>
                {getCliCommand()};
              </code>
              <button
                className="param-reset-btn"
                onClick={() => navigator.clipboard?.writeText(`${getCliCommand()};`)}
              >
                Copy
              </button>
            </div>
            <p style={{ marginTop: '8px', color: 'var(--fg-muted)', fontSize: '10.5px' }}>
              * Every sliced model exported from the slicer will automatically be arc-welded, collinear-merged, and jerk-smoothed!
            </p>
          </div>
        )}
      </div>
    </div>
  );
};
