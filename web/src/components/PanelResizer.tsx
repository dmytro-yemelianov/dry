import React from 'react';

type Side = 'left' | 'right';

const LIMITS: Record<Side, { min: number; max: number }> = {
  left: { min: 220, max: 560 },
  right: { min: 260, max: 640 },
};

/**
 * Drag handle between a sidebar and the viewport.
 *
 * Studio 2.0 fixed both sidebars at a constant width, which makes the centre pane as narrow as the
 * widest panel demands — on a laptop the 3D view ends up a letterbox. Width is written to a CSS
 * custom property on the layout root so only two rules depend on it, and it is clamped so a panel
 * can never be dragged to nothing and stranded.
 */
export const PanelResizer: React.FC<{ side: Side }> = ({ side }) => {
  const dragging = React.useRef(false);

  const apply = (px: number) => {
    const { min, max } = LIMITS[side];
    const width = Math.round(Math.min(Math.max(px, min), max));
    document.documentElement.style.setProperty(`--${side}-panel-w`, `${width}px`);
    try {
      localStorage.setItem(`dry.panel.${side}`, String(width));
    } catch {
      // Private windows and blocked site data are fine; the width just does not persist.
    }
  };

  React.useEffect(() => {
    try {
      const saved = Number(localStorage.getItem(`dry.panel.${side}`));
      if (Number.isFinite(saved) && saved > 0) apply(saved);
    } catch {
      // No stored width: the stylesheet default stands.
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [side]);

  React.useEffect(() => {
    const onMove = (event: PointerEvent) => {
      if (!dragging.current) return;
      event.preventDefault();
      apply(side === 'left' ? event.clientX : window.innerWidth - event.clientX);
    };
    const onUp = () => {
      dragging.current = false;
      document.body.classList.remove('is-resizing-panels');
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [side]);

  return (
    <div
      className="panel-resizer"
      data-resize-panel={side}
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${side} panel`}
      onPointerDown={(e) => {
        e.preventDefault();
        dragging.current = true;
        document.body.classList.add('is-resizing-panels');
      }}
      onDoubleClick={() => apply(side === 'left' ? 340 : 400)}
      title="Drag to resize, double-click to reset"
    />
  );
};
