import React from 'react';
import { useStudioStore } from '../store/useStudioStore';

/** The /web/ portal pages ship with that deploy only; other mounts (e.g. /gallery/) lack them. */
const portalIsReachable = import.meta.env.BASE_URL === '/web/';

export const Header: React.FC = () => {
  const machines = useStudioStore((state) => state.machines);
  const activeMachine = useStudioStore((state) => state.activeMachine);
  const setActiveMachine = useStudioStore((state) => state.setActiveMachine);
  const gcodeLines = useStudioStore((state) => state.gcodeLines);

  const exportGcode = () => {
    if (!gcodeLines || !gcodeLines.length) return;
    const blob = new Blob([gcodeLines.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `dry_machina_${activeMachine.id}.gcode`;
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
        <button onClick={exportGcode} className="btn-action primary">Export G-Code</button>
      </div>
    </header>
  );
};
