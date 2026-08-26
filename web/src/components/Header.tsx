import React from 'react';
import { useStudioStore } from '../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY, RESOLVE_PARAMS } from '../data/designs';
import { buildExportText, exportFilename } from '../data/exporters';
import type { DesignDef } from '../types/domain';

/** The /web/ portal pages ship with that deploy only; other mounts (e.g. /gallery/) lack them. */
const portalIsReachable = import.meta.env.BASE_URL === '/web/';

export const Header: React.FC = () => {
  const machines = useStudioStore((state) => state.machines);
  const activeMachine = useStudioStore((state) => state.activeMachine);
  const setActiveMachine = useStudioStore((state) => state.setActiveMachine);
  const gcodeLines = useStudioStore((state) => state.gcodeLines);
  const exportFormat = useStudioStore((state) => state.exportFormat);
  const macroOptions = useStudioStore((state) => state.macroOptions);
  const metrics = useStudioStore((state) => state.metrics);
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const activeParams = useStudioStore((state) => state.activeParams);

  // The button follows the format chosen in the Export panel. Emitting G-code regardless would
  // quietly ignore that choice and hand back the wrong file type.
  const exportCurrent = () => {
    if (!gcodeLines || !gcodeLines.length) return;
    const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
    const def = allDefs[activeDesignKey];
    const designLabel = def?.title ?? def?.label ?? activeDesignKey;
    const ops = def?.ops ?? (def?.build ? def.build(activeParams) : []);
    const text = buildExportText({
      format: exportFormat,
      gcodeLines,
      ops,
      metrics,
      macros: macroOptions,
      machine: activeMachine,
      params: RESOLVE_PARAMS,
      designLabel,
    });
    if (!text.trim()) return;
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = exportFilename(exportFormat, designLabel);
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <header className="studio-header">
      <div className="brand-section">
        <a href="/" className="brand-logo">
          <span className="core-dot"></span>
          Dry Machina
        </a>
        <span className="badge-version">Studio 2.3 Pro</span>
      </div>

      <div className="machine-select-wrapper">
        <label htmlFor="machineSelect">Machine:</label>
        <select
          id="machineSelect"
          className="machine-select"
          value={activeMachine.id}
          onChange={(e) => setActiveMachine(e.target.value)}
        >
          {machines.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name} ({m.manufacturer})
            </option>
          ))}
        </select>
      </div>

      <div className="process-tabs">
        <button className="process-tab active" data-process="fdm">3D Printing</button>
        <button className="process-tab" data-process="cnc">CNC Mill</button>
        <button className="process-tab" data-process="laser">Laser</button>
        <button className="process-tab" data-process="plasma">Plasma</button>
        <button className="process-tab" data-process="robotics">5-Axis RTCP</button>
      </div>

      <div className="header-actions">
        {/* These are product-site portal pages, hardwired to /web/ and served only by that deploy.
            The same app is also mounted at /gallery/ inside the docs site, where they 404 — so
            offer them only where they actually resolve rather than shipping dead links. */}
        {portalIsReachable && (
          <>
            <a href="/web/machines.html" className="btn-action">Machines</a>
            <a href="/web/docs.html" className="btn-action" target="_blank" rel="noreferrer">Docs &amp; Specs</a>
          </>
        )}
        <button onClick={exportCurrent} className="btn-action primary">Export G-Code</button>
      </div>
    </header>
  );
};
