import React from 'react';
import { useStudioStore } from '../store/useStudioStore';

const SPEEDS = [0.25, 0.5, 1.0, 4.0, 16.0, 64.0];

export const PlaybackController: React.FC = () => {
  const isPlaying = useStudioStore((state) => state.isPlaying);
  const togglePlay = useStudioStore((state) => state.togglePlay);
  const currentTime = useStudioStore((state) => state.currentTime);
  const maxTime = useStudioStore((state) => state.maxTime);
  const playSpeed = useStudioStore((state) => state.playSpeed);
  const setPlaySpeed = useStudioStore((state) => state.setPlaySpeed);
  const seekTime = useStudioStore((state) => state.seekTime);
  const activeSectionIndex = useStudioStore((state) => state.activeSectionIndex);
  const gcodeSections = useStudioStore((state) => state.gcodeSections);
  const effectiveGroupingKind = useStudioStore((state) => state.effectiveGroupingKind);
  const nextSection = useStudioStore((state) => state.nextSection);
  const prevSection = useStudioStore((state) => state.prevSection);

  const sliderVal = maxTime > 0 ? (currentTime / maxTime) * 100 : 0;
  const maxSection = gcodeSections[gcodeSections.length - 1]?.index || 1;

  const sectionPrefix =
    effectiveGroupingKind === 'revolution'
      ? 'Turn'
      : effectiveGroupingKind === 'figure'
      ? 'Fig'
      : 'L';

  return (
    <footer className="studio-playback">
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <button onClick={togglePlay} className="playback-play-btn">
          {isPlaying ? '⏸' : '▶'}
        </button>

        <div className="layer-playback-stepper">
          <button className="layer-step-btn" onClick={prevSection} disabled={activeSectionIndex <= 1}>
            ⏮
          </button>
          <span className="layer-chip">
            {sectionPrefix}{activeSectionIndex}
          </span>
          <button className="layer-step-btn" onClick={nextSection} disabled={activeSectionIndex >= maxSection}>
            ⏭
          </button>
        </div>
      </div>

      <div className="timeline-slider-wrapper">
        <input
          type="range"
          className="timeline-slider"
          min="0"
          max="100"
          value={sliderVal}
          onChange={(e) => {
            const frac = parseFloat(e.target.value) / 100;
            seekTime(frac * maxTime);
          }}
        />
        <span className="timeline-label">
          {currentTime.toFixed(1)}s / {maxTime.toFixed(1)}s
        </span>
      </div>

      <div className="speed-selector">
        {SPEEDS.map((s) => (
          <button
            key={s}
            className={`speed-btn ${playSpeed === s ? 'active' : ''}`}
            onClick={() => setPlaySpeed(s)}
          >
            {s}×
          </button>
        ))}
      </div>
    </footer>
  );
};
