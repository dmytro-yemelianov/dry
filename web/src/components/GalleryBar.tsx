import React from 'react';
import { useStudioStore } from '../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY } from '../data/designs';
import type { DesignDef } from '../types/domain';

/**
 * Identity and provenance for the design on screen.
 *
 * The gallery ships reconstructions of published work, so naming the design and crediting its
 * origin is part of what the page owes its sources — it is not decoration. The bounds field is the
 * verifier's build-volume constraint, and its parse error surfaces here rather than in a console
 * nobody reads.
 */
export const GalleryBar: React.FC = () => {
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const boundsInput = useStudioStore((state) => state.boundsInput);
  const sourceError = useStudioStore((state) => state.sourceError);
  const setBoundsInput = useStudioStore((state) => state.setBoundsInput);

  const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
  const def = allDefs[activeDesignKey];
  const title = def?.title ?? def?.label ?? activeDesignKey;
  const links = def?.links ?? [];

  return (
    <div className="gallery-bar">
      <div className="gallery-bar-identity">
        <span id="designTitle" className="gallery-title">
          {title}
        </span>
        <span id="designLinks" className="gallery-links">
          {links.map(([label, href]) => (
            <a key={href} href={href} target="_blank" rel="noreferrer noopener">
              {label}
            </a>
          ))}
        </span>
      </div>

      <label className="gallery-bounds">
        <span>bounds</span>
        <input
          type="text"
          aria-label="bounds"
          placeholder="x0,x1,y0,y1,z0,z1"
          value={boundsInput}
          onChange={(event) => setBoundsInput(event.target.value)}
        />
      </label>

      <div id="sourceError" className="gallery-error" hidden={!sourceError} role="alert">
        {sourceError}
      </div>
    </div>
  );
};
