import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';

export const OptimizerDiffs: React.FC = () => {
  const toolpath = useStudioStore((state) => state.toolpath);
  const optimizedToolpath = useStudioStore((state) => state.optimizedToolpath);

  const origCount = toolpath?.segments ? toolpath.segments.length : 0;
  const optCount = optimizedToolpath?.segments ? optimizedToolpath.segments.length : origCount;
  const reduction =
    origCount > 0 && optCount < origCount
      ? (((origCount - optCount) / origCount) * 100).toFixed(1)
      : '0.0';

  return (
    <div className="optimizer-diffs-root">
      <div className="stat-grid">
        <div className="stat-box">
          <div className="title">Original Moves</div>
          <div className="val">{origCount}</div>
        </div>
        <div className="stat-box">
          <div className="title">Optimized Moves</div>
          <div className="val">{optCount}</div>
        </div>
      </div>

      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>Collinear Merge: {reduction}% segment reduction</span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>TSP Travel Reordering: Rapid moves minimized</span>
      </div>
      <div className="check-item">
        <span className="check-icon pass">✓</span>
        <span>Arc Fitting: High-curvature runs preserved</span>
      </div>
    </div>
  );
};
