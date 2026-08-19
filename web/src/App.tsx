import React, { useEffect, useState } from 'react';
import { useStudioStore } from './store/useStudioStore';
import { Header } from './components/Header';
import { CatalogAccordions } from './components/LeftExplorer/CatalogAccordions';
import { ThreeViewport } from './viewport/ThreeViewport';
import { GcodeInspector } from './components/RightInspector/GcodeInspector';
import { SafetyMatrix } from './components/RightInspector/SafetyMatrix';
import { TelemetryCard } from './components/RightInspector/TelemetryCard';
import { OptimizerDiffs } from './components/RightInspector/OptimizerDiffs';
import { MacroStudio } from './components/RightInspector/MacroStudio';
import { SlicerWizard } from './components/RightInspector/SlicerWizard';
import { ApiPortal } from './components/RightInspector/ApiPortal';
import { PlaybackController } from './components/PlaybackController';

export const App: React.FC = () => {
  const initStudio = useStudioStore((state) => state.initStudio);
  const [rightTab, setRightTab] = useState<'gcode' | 'safety' | 'telemetry' | 'optimizer' | 'macros' | 'slicers' | 'api'>('gcode');

  useEffect(() => {
    initStudio();
  }, [initStudio]);

  return (
    <div className="studio-app-container">
      <Header />

      <main className="studio-main">
        {/* Left Sidebar: Catalog & Inline Live Parameters */}
        <aside className="sidebar-left">
          <div className="panel-header">
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontWeight: 700, letterSpacing: '0.03em' }}>DESIGN LIBRARY</span>
              <span className="library-badge">60 Models</span>
            </div>
          </div>

          <div className="panel-content">
            <CatalogAccordions />

            <div style={{ marginTop: '20px', borderTop: '1px solid var(--border)', paddingTop: '14px' }}>
              <div className="panel-header" style={{ background: 'transparent', padding: '0 0 8px', height: 'auto' }}>
                <span style={{ fontSize: '11px', fontWeight: 600, color: 'var(--fg-muted)', textTransform: 'uppercase' }}>
                  Custom G-Code / STEP-NC
                </span>
              </div>
              <div style={{ border: '1px dashed var(--border)', borderRadius: '6px', padding: '12px', textAlign: 'center', fontSize: '11px', color: 'var(--fg-muted)' }}>
                Drag & drop arbitrary <code>.gcode</code>, <code>.nc</code>, or <code>.stepnc</code> file to inspect in 3D.
              </div>
            </div>
          </div>
        </aside>

        {/* Center 3D Viewport */}
        <ThreeViewport />

        {/* Right Sidebar: Inspector Tabs */}
        <aside className="sidebar-right">
          <div className="panel-header" style={{ padding: 0 }}>
            <div className="panel-tabs">
              <button
                className={`panel-tab-btn ${rightTab === 'gcode' ? 'active' : ''}`}
                onClick={() => setRightTab('gcode')}
              >
                G-Code
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'optimizer' ? 'active' : ''}`}
                onClick={() => setRightTab('optimizer')}
              >
                Optimizer
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'macros' ? 'active' : ''}`}
                onClick={() => setRightTab('macros')}
              >
                Macros
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'slicers' ? 'active' : ''}`}
                onClick={() => setRightTab('slicers')}
              >
                Slicers
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'api' ? 'active' : ''}`}
                onClick={() => setRightTab('api')}
              >
                API & MCP
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'safety' ? 'active' : ''}`}
                onClick={() => setRightTab('safety')}
              >
                Safety
              </button>
              <button
                className={`panel-tab-btn ${rightTab === 'telemetry' ? 'active' : ''}`}
                onClick={() => setRightTab('telemetry')}
              >
                Telemetry
              </button>
            </div>
          </div>

          <div className="panel-content">
            {rightTab === 'gcode' && <GcodeInspector />}
            {rightTab === 'optimizer' && <OptimizerDiffs />}
            {rightTab === 'macros' && <MacroStudio />}
            {rightTab === 'slicers' && <SlicerWizard />}
            {rightTab === 'api' && <ApiPortal />}
            {rightTab === 'safety' && <SafetyMatrix />}
            {rightTab === 'telemetry' && <TelemetryCard />}
          </div>
        </aside>
      </main>

      {/* Bottom Transport Timeline */}
      <PlaybackController />
    </div>
  );
};
