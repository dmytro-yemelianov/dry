import React, { useState } from 'react';
import { DesignThumbnail } from './DesignThumbnail';
import { useStudioStore } from '../../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY } from '../../data/designs';
import type { DesignDef } from '../../types/domain';

export const CatalogAccordions: React.FC = () => {
  const activeCategory = useStudioStore((state) => state.activeCategory);
  const setActiveCategory = useStudioStore((state) => state.setActiveCategory);
  const searchQuery = useStudioStore((state) => state.searchQuery);
  const setSearchQuery = useStudioStore((state) => state.setSearchQuery);
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const selectDesign = useStudioStore((state) => state.selectDesign);
  const activeParams = useStudioStore((state) => state.activeParams);
  const updateParam = useStudioStore((state) => state.updateParam);
  const resetParams = useStudioStore((state) => state.resetParams);

  // Parameters are collapsed by default
  const [expandedCardKey, setExpandedCardKey] = useState<string | null>(null);

  const categories: Record<string, DesignDef[]> = {
    'Vases & Non-Planar': [],
    'TPMS Minimal Surfaces': [],
    'Research Lattices': [],
    'Curves & Geometries': [],
    'Infill & Multi-Layer': [],
    'Basics': [],
    'FullControl Gallery': [],
  };

  Object.values(DESIGN_DEFS).forEach((def) => {
    const group = def.group || 'Basics';
    const catName = group.includes('Vases')
      ? 'Vases & Non-Planar'
      : group.includes('TPMS')
      ? 'TPMS Minimal Surfaces'
      : group.includes('Research')
      ? 'Research Lattices'
      : group.includes('Curves')
      ? 'Curves & Geometries'
      : group.includes('Infill')
      ? 'Infill & Multi-Layer'
      : 'Basics';

    categories[catName].push(def);
  });

  Object.values(FULLCONTROL_GALLERY).forEach((def) => {
    categories['FullControl Gallery'].push(def);
  });

  const query = searchQuery.toLowerCase().trim();

  return (
    <div className="catalog-accordion-root">
      <input
        type="text"
        className="gallery-search-input"
        placeholder="Search 50+ designs, lattices & FullControl..."
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
      />

      <div className="filter-pills">
        <button
          className={`filter-pill ${activeCategory === 'all' ? 'active' : ''}`}
          onClick={() => setActiveCategory('all')}
        >
          All
        </button>
        <button
          className={`filter-pill ${activeCategory === 'vases' ? 'active' : ''}`}
          onClick={() => setActiveCategory('vases')}
        >
          Vases & 3D
        </button>
        <button
          className={`filter-pill ${activeCategory === 'tpms' ? 'active' : ''}`}
          onClick={() => setActiveCategory('tpms')}
        >
          TPMS
        </button>
        <button
          className={`filter-pill ${activeCategory === 'lattices' ? 'active' : ''}`}
          onClick={() => setActiveCategory('lattices')}
        >
          Lattices
        </button>
        <button
          className={`filter-pill ${activeCategory === 'infill' ? 'active' : ''}`}
          onClick={() => setActiveCategory('infill')}
        >
          Infill
        </button>
        <button
          className={`filter-pill ${activeCategory === 'fullcontrol' ? 'active' : ''}`}
          onClick={() => setActiveCategory('fullcontrol')}
        >
          FullControl
        </button>
      </div>

      <div className="gallery-list">
        {Object.entries(categories).map(([catTitle, items]) => {
          const filteredItems = items.filter((item) => {
            const matchCat =
              activeCategory === 'all' ||
              (activeCategory === 'vases' && catTitle.includes('Vases')) ||
              (activeCategory === 'tpms' && catTitle.includes('TPMS')) ||
              (activeCategory === 'lattices' && catTitle.includes('Lattices')) ||
              (activeCategory === 'infill' && catTitle.includes('Infill')) ||
              (activeCategory === 'fullcontrol' && catTitle.includes('FullControl'));

            const matchSearch =
              !query ||
              item.label.toLowerCase().includes(query) ||
              item.tags.some((t) => t.toLowerCase().includes(query));

            return matchCat && matchSearch;
          });

          if (!filteredItems.length) return null;

          return (
            <div key={catTitle} className="category-group">
              <div className="category-header">
                <span>{catTitle}</span>
                <span className="category-count">{filteredItems.length}</span>
              </div>
              <div className="category-items">
                {filteredItems.map((item) => {
                  const isSelected = item.key === activeDesignKey;
                  const isParametric = item.params && item.params.length > 0;
                  const isExpanded = isSelected && expandedCardKey === item.key;

                  return (
                    <div
                      key={item.key}
                      className={`gallery-card ${isSelected ? 'active' : ''}`}
                      onClick={() => {
                        if (!isSelected) {
                          selectDesign(item.key);
                          // Parameters remain collapsed by default on selection
                        }
                      }}
                    >
                      <div className="gallery-card-header">
                        <DesignThumbnail def={item} />
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '3px' }}>
                          <div className="gallery-title">{item.label}</div>
                          {isParametric && (
                            <button
                              className={`param-toggle-pill ${isExpanded ? 'open' : ''}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                if (!isSelected) selectDesign(item.key);
                                setExpandedCardKey(isExpanded ? null : item.key);
                              }}
                              title={isExpanded ? 'Collapse parameters' : 'Expand parameter sliders'}
                            >
                              {isExpanded ? '▼ Params' : `⚙️ ${item.params.length}`}
                            </button>
                          )}
                        </div>

                        <div className="gallery-tags">
                          {item.tags.map((t) => (
                            <span key={t} className="tag-badge">
                              {t}
                            </span>
                          ))}
                        </div>
                      </div>

                      {/* Inline Parameter Drawer when expanded */}
                      {isExpanded && isParametric && (
                        <div
                          className="card-param-drawer"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <div className="param-inline-header">
                            <span>Adjust Parameters</span>
                            <button
                              onClick={resetParams}
                              className="param-reset-btn"
                              title="Reset all sliders to default values"
                            >
                              ↺ Reset
                            </button>
                          </div>

                          {item.note ? <div className="param-note">{item.note}</div> : null}

                          <div className="param-fields-compact">
                            {item.params.map((p) => {
                              const val = activeParams[p.id] ?? p.defaultValue;
                              const modified = val !== p.defaultValue;
                              return (
                                <div key={p.id} className="param-row param-row-compact" title={p.title || undefined}>
                                  <div className="param-label-wrapper-compact">
                                    <span className="param-label-compact param-name">{p.label}</span>
                                    {p.unit ? <span className="param-unit">[{p.unit}]</span> : null}
                                    {p.kind !== 'toggle' && (
                                      <span className="param-val-badge-compact">
                                        {val} {p.unit}
                                      </span>
                                    )}
                                    {/* Undo one slider without discarding every other adjustment. */}
                                    <button
                                      type="button"
                                      className="default-reset"
                                      data-reset-control={p.id}
                                      aria-label={`Reset ${p.label} to default`}
                                      title={`Reset to ${p.defaultValue}`}
                                      disabled={!modified}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        updateParam(p.id, p.defaultValue);
                                      }}
                                    >
                                      ↺
                                    </button>
                                  </div>
                                  {p.kind === 'toggle' ? (
                                    <label className="param-toggle">
                                      <input
                                        type="checkbox"
                                        checked={val === 1}
                                        onChange={(e) => updateParam(p.id, e.target.checked ? 1 : 0)}
                                      />
                                      <span>{val === 1 ? 'on' : 'off'}</span>
                                    </label>
                                  ) : (
                                    <div className="param-input-wrapper-compact">
                                      <input
                                        type="range"
                                        className="param-slider-compact"
                                        min={p.min}
                                        max={p.max}
                                        step={p.step}
                                        value={val}
                                        onChange={(e) =>
                                          updateParam(p.id, parseFloat(e.target.value))
                                        }
                                      />
                                      <input
                                        type="number"
                                        className="param-num-input-compact"
                                        min={p.min}
                                        max={p.max}
                                        step={p.step}
                                        value={val}
                                        onChange={(e) =>
                                          updateParam(p.id, parseFloat(e.target.value))
                                        }
                                      />
                                    </div>
                                  )}
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
