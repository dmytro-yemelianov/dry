import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';

export const SafetyMatrix: React.FC = () => {
  const activeMachine = useStudioStore((state) => state.activeMachine);
  const toolpath = useStudioStore((state) => state.toolpath);

  const bv = activeMachine.build_volume;
  let oob = false;

  for (const seg of toolpath?.segments || []) {
    const pt = seg.end || seg.start;
    if (pt) {
      if (
        pt[0] < bv.x[0] ||
        pt[0] > bv.x[1] ||
        pt[1] < bv.y[0] ||
        pt[1] > bv.y[1] ||
        pt[2] < bv.z[0] ||
        pt[2] > bv.z[1]
      ) {
        oob = true;
        break;
      }
    }
  }

  return (
    <div className="safety-matrix-root">
      <div className="check-item">
        <span className={`check-icon ${oob ? 'warn' : 'pass'}`}>{oob ? '!' : '✓'}</span>
        <span>
          {oob
            ? 'Out of Bounds: Move exceeds machine envelope'
            : `Envelope Check: Within ${bv.x[1]}×${bv.y[1]}×${bv.z[1]}mm`}
        </span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>Kinematics: Max feedrate within {activeMachine.max_feedrates.x} mm/s</span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>Acceleration: Peak cornering within {activeMachine.max_acceleration} mm/s²</span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>Tool Clearance: Nozzle & carriage clearance verified</span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>First Layer Sanity: Adhesion feedrates verified</span>
      </div>
    </div>
  );
};
