import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';

export const TelemetryCard: React.FC = () => {
  const metrics = useStudioStore((state) => state.metrics);

  const duration = metrics ? (metrics.total_time_s ?? metrics.print_time_s ?? 0) : 0;
  const length = metrics ? (metrics.extruding_distance ?? metrics.travel_distance ?? 0) : 0;
  const volume = metrics ? (metrics.extruded_volume ?? 0) : 0;
  const mass = volume * 0.00124; // PLA density ~1.24g/cm3

  return (
    <div className="telemetry-card-root">
      <div className="stat-grid">
        <div className="stat-box">
          <div className="title">Duration</div>
          <div>
            <span className="val">{Number(duration).toFixed(1)}</span>
            <span className="unit">s</span>
          </div>
        </div>
        <div className="stat-box">
          <div className="title">Total Path</div>
          <div>
            <span className="val">{Number(length).toFixed(0)}</span>
            <span className="unit">mm</span>
          </div>
        </div>
        <div className="stat-box">
          <div className="title">Extruded Vol</div>
          <div>
            <span className="val">{Number(volume).toFixed(1)}</span>
            <span className="unit">mm³</span>
          </div>
        </div>
        <div className="stat-box">
          <div className="title">Material Mass</div>
          <div>
            <span className="val">{Number(mass).toFixed(2)}</span>
            <span className="unit">g</span>
          </div>
        </div>
      </div>

      <div style={{ marginTop: '12px', fontSize: '11.5px', color: 'var(--fg-muted)', lineHeight: '1.5' }}>
        <div>• <b>Segment Count</b>: {metrics?.segment_count ?? 0} moves</div>
        <div>• <b>Peak Flow Rate</b>: {Number(metrics?.max_flow_rate ?? 0).toFixed(2)} mm³/s</div>
        <div>• <b>Travel Time</b>: {Number(metrics?.travel_time_s ?? 0).toFixed(1)} s</div>
      </div>
    </div>
  );
};
