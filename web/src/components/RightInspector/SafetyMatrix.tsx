import React from 'react';
import { useStudioStore } from '../../store/useStudioStore';
import { DESIGN_DEFS, FULLCONTROL_GALLERY, RESOLVE_PARAMS } from '../../data/designs';
import { compileVerify, type VerifyContracts, type VerifyReport } from '../../wasm/engine';
import { boundsParseError } from '../../store/useStudioStore';
import type { DesignDef } from '../../types/domain';

/**
 * Real verification results.
 *
 * This panel used to show five rows: one hand-rolled envelope test in JavaScript, and four
 * hardcoded green ticks — kinematics, acceleration, tool clearance, first layer — that were never
 * computed. The engine's own verifier was reachable the whole time but its wrapper was dead code
 * calling a thirteen-argument function with four arguments.
 *
 * The Report deliberately carries `segments_inspected` and `rules_evaluated` so that a vacuous pass
 * is not byte-identical to a real one (H1.3 design §3.5). That is exactly the distinction the ticks
 * erased, so both are shown: a rule that was not evaluated is reported as not evaluated, never as
 * passing.
 */
export const SafetyMatrix: React.FC = () => {
  const activeMachine = useStudioStore((state) => state.activeMachine);
  const activeDesignKey = useStudioStore((state) => state.activeDesignKey);
  const activeParams = useStudioStore((state) => state.activeParams);
  const boundsInput = useStudioStore((state) => state.boundsInput);

  const allDefs: Record<string, DesignDef> = { ...DESIGN_DEFS, ...FULLCONTROL_GALLERY };
  const def = allDefs[activeDesignKey];

  const { report, error } = React.useMemo(() => {
    if (!def) return { report: null, error: 'No design selected.' };
    const ops = def.ops ?? (def.build ? def.build(activeParams) : []);
    if (!ops.length) return { report: null, error: 'This design produced no operations.' };

    const bv = activeMachine.build_volume;
    // The machine envelope is a contract the engine can check, rather than something to re-derive
    // in JavaScript. A bounds string typed in the gallery bar overrides it, when it parses.
    let bounds = [bv.x[0], bv.x[1], bv.y[0], bv.y[1], bv.z[0], bv.z[1]];
    const typed = boundsInput.trim();
    if (typed && !boundsParseError(typed)) {
      const parsed = typed.split(',').map((token) => Number(token.trim()));
      if (parsed.length === 6) bounds = parsed;
    }

    const contracts: VerifyContracts = {
      bounds,
      kinematics: { max_acceleration_mm_s2: activeMachine.max_acceleration },
    };

    try {
      return { report: compileVerify(ops, RESOLVE_PARAMS, contracts), error: '' };
    } catch (e) {
      return { report: null, error: e instanceof Error ? e.message : String(e) };
    }
  }, [def, activeParams, activeMachine, boundsInput]);

  if (error || !report) {
    return (
      <div className="safety-matrix-root">
        <div className="check-item">
          <span className="check-icon warn">!</span>
          <span>{error || 'Verification unavailable.'}</span>
        </div>
      </div>
    );
  }

  const inspected = report.segments_inspected ?? 0;
  const rules = report.rules_evaluated ?? [];

  // One bad contract can put every segment in breach — a 1mm envelope produces a finding per
  // segment. Listing them all buries the fact that it is one problem and renders thousands of rows,
  // so group by rule and severity and show the count with a couple of examples.
  const groups = new Map<string, { rule: string; severity: string; count: number; samples: string[] }>();
  for (const finding of report.findings) {
    const key = `${finding.severity}:${finding.rule}`;
    const group = groups.get(key) ?? { rule: finding.rule, severity: finding.severity, count: 0, samples: [] };
    group.count += 1;
    if (group.samples.length < 2) {
      group.samples.push(
        finding.segment != null ? `${finding.message} (segment ${finding.segment})` : finding.message,
      );
    }
    groups.set(key, group);
  }
  const ordered = [...groups.values()].sort((a, b) =>
    a.severity === b.severity ? b.count - a.count : a.severity === 'error' ? -1 : 1,
  );

  return (
    <div className="safety-matrix-root">
      {/* Coverage first: a clean report over zero segments proves nothing, and saying so up front
          is the difference between this panel and the ticks it replaces. */}
      <div className="verify-coverage">
        {inspected > 0 ? (
          <>
            <strong>{inspected.toLocaleString()}</strong> segments inspected against{' '}
            <strong>{rules.length}</strong> rules
          </>
        ) : (
          <>No segments were inspected — this report proves nothing.</>
        )}
      </div>

      {report.findings.length === 0 ? (
        <div className="check-item">
          <span className="check-icon pass">✓</span>
          <span>
            {inspected > 0
              ? 'No findings against the rules listed below.'
              : 'No findings, but nothing was checked.'}
          </span>
        </div>
      ) : (
        <>
          {ordered.map((group) => (
            <div className="check-item" key={`${group.severity}:${group.rule}`}>
              <span className={`check-icon ${group.severity === 'error' ? 'fail' : 'warn'}`}>
                {group.severity === 'error' ? '✕' : '!'}
              </span>
              <span>
                <code className="verify-rule">{group.rule}</code>
                {group.count > 1 ? <span className="finding-count">×{group.count.toLocaleString()}</span> : null}
                <div className="finding-samples">
                  {group.samples.map((sample, i) => (
                    <div key={i}>{sample}</div>
                  ))}
                  {group.count > group.samples.length ? (
                    <div className="finding-more">
                      and {(group.count - group.samples.length).toLocaleString()} more
                    </div>
                  ) : null}
                </div>
              </span>
            </div>
          ))}
        </>
      )}

      <div className="verify-rules">
        <div className="panel-subhead">Rules evaluated</div>
        {rules.length ? (
          <div className="verify-rule-list">
            {rules.map((rule) => (
              <code className="verify-rule" key={rule}>
                {rule}
              </code>
            ))}
          </div>
        ) : (
          <div className="verify-coverage">None — no contract was supplied to check against.</div>
        )}
      </div>
    </div>
  );
};
