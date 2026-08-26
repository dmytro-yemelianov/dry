import React from 'react';
import { thumbnail } from '../../../thumb.js';
import { thumbnailWasm } from '../../wasm/engine';
import { RESOLVE_PARAMS } from '../../data/designs';
import type { DesignDef } from '../../types/domain';

/**
 * Top-down preview of a design, drawn once and reused.
 *
 * Every thumbnail costs a full `resolve_ir`, so 60 of them on mount would stall the catalog. They
 * are rendered only once a card is actually scrolled into view, and cached by design key so
 * scrolling back does not pay again.
 */
const CACHE = new Map<string, string>();

export const DesignThumbnail: React.FC<{ def: DesignDef; size?: number }> = ({ def, size = 72 }) => {
  const [src, setSrc] = React.useState<string | null>(() => CACHE.get(def.key) ?? null);
  const holder = React.useRef<HTMLDivElement | null>(null);

  React.useEffect(() => {
    if (src) return;
    const node = holder.current;
    if (!node) return;

    let cancelled = false;
    const render = () => {
      if (cancelled || CACHE.has(def.key)) {
        if (CACHE.has(def.key)) setSrc(CACHE.get(def.key)!);
        return;
      }
      try {
        // Defaults only: a card should show what the design is, not the sliders' current position.
        const params = Object.fromEntries((def.params ?? []).map((p) => [p.id, p.defaultValue]));
        const ops = def.ops ?? (def.build ? def.build(params) : []);
        if (!ops.length) return;
        const url = thumbnail(ops as unknown[], thumbnailWasm, RESOLVE_PARAMS, size);
        CACHE.set(def.key, url);
        if (!cancelled) setSrc(url);
      } catch {
        // A design that cannot resolve simply shows no preview; the card still works.
      }
    };

    if (typeof IntersectionObserver === 'undefined') {
      render();
      return () => {
        cancelled = true;
      };
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          observer.disconnect();
          // Yield first so scrolling stays smooth while the IR resolves.
          window.requestIdleCallback ? window.requestIdleCallback(render) : setTimeout(render, 0);
        }
      },
      { rootMargin: '120px' },
    );
    observer.observe(node);
    return () => {
      cancelled = true;
      observer.disconnect();
    };
  }, [def, size, src]);

  return (
    <div className="design-thumb" ref={holder} style={{ width: size, height: size }} aria-hidden="true">
      {src ? <img src={src} alt="" width={size} height={size} /> : null}
    </div>
  );
};
