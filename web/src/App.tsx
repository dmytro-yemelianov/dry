import React, { useEffect, useState } from 'react';
import { useStudioStore } from './store/useStudioStore';
import { Header } from './components/Header';
import { CatalogAccordions } from './components/LeftExplorer/CatalogAccordions';
import { ParametricForm } from './components/LeftExplorer/ParametricForm';
import { ThreeViewport } from './viewport/ThreeViewport';
import { GcodeInspector } from './components/RightInspector/GcodeInspector';
import { SafetyMatrix } from './components/RightInspector/SafetyMatrix';
import { TelemetryCard } from './components/RightInspector/TelemetryCard';
import { OptimizerDiffs } from './components/RightInspector/OptimizerDiffs';
import { PlaybackController } from './components/PlaybackController';

export const App: React.FC = () => {
  const initStudio = useStudioStore((state) => state.initStudio);
  const [leftTab, setLeftTab] = useState<'catalog' | 'params'>('catalog');
  const [rightTab, setRightTab] = useState<'gcode' | 'safety' | 'telemetry' | 'optimizer'>('gcode');

  useEffect(() => {
    initStudio();
  }, [initStudio]);

  return (
    <div className="studio-app-container">
      <Header />

      <main className="studio-main">
        {/* Left Sidebar: Catalog & Live Parameters */}
        <aside className="sidebar-left">
          <div className="panel-header" style={{ padding: 0 }}>
            <div className="left-tabs">
              <button
                className={`left-tab-btn ${leftTab === 'catalog' ? 'active' : ''}`}
                onClick={() => setLeftTab('catalog')}
              >
                Catalog
              </button>
              <button
                className={`left-tab-btn ${leftTab === 'params' ? 'active' : ''}`}
                onClick={() => setLeftTab('params')}
              >
                Parameters
              </button>
            </div>
          </div>

          <div className="panel-content">
            {leftTab === 'catalog' ? (
              <>
                <CatalogAccordions />
                <div style={{ marginTop: '20px', borderTop: '1px solid var(--border)', paddingTop: '14px' }}>
                  <div className="panel-header" style={{ background: 'transparent', padding: '0 0 8px', height: 'auto' }}>
                    <span>Custom G-Code / STEP-NC</span>
                  </div>
                  <div style={{ border: '1px dashed var(--border)', borderRadius: '6px', padding: '12px', textAlign: 'center', fontSize: '11px', color: 'var(--fg-muted)' }}>
                    Drag & drop arbitrary <code>.gcode</code>, <code>.nc</code>, or <code>.stepnc</code> file to inspect in 3D.
                  </div>
                </div>
              </>
            ) : (
              <ParametricForm />
            )}
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
              <button
                className={`panel-tab-btn ${rightTab === 'optimizer' ? 'active' : ''}`}
                onClick={() => setRightTab('optimizer')}
              >
                Optimizer
              </button>
            </div>
          </div>

          <div className="panel-content" style={{ height: 'calc(100% - 40px)', padding: '12px' }}>
            {rightTab === 'gcode' && <GcodeInspector />}
            {rightTab === 'safety' && <SafetyMatrix />}
            {rightTab === 'telemetry' && <TelemetryCard />}
            {rightTab === 'optimizer' && <OptimizerDiffs />}
          </div>
        </aside>
      </main>

      <PlaybackController />
    </div>
  );
};
