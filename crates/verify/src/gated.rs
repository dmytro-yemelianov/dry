//! The verification-gated rewrite wrappers.
//!
//! These are `drymachina_kernel::optimize::apply_gated_with` bound to `verify` as its error-rule policy.
//! The mechanism is kernel code; the policy is not, because a kernel that could call the verifier
//! would reinstate the cycle the crate split exists to break. So they sit here, in the crate that
//! owns `verify` — the lowest layer that can name both halves (plan Tasks 4 and 5).

use crate::Contracts;
use drymachina_kernel::ir::Toolpath;
use drymachina_kernel::optimize::{apply_gated_with, GatedResult, OptimizeMode};
use drymachina_kernel::profile::MachineKinematics;

/// The verification-gated rewrite: [`apply_gated_with`] with `verify` as the policy, so a rewrite is
/// accepted only if it introduces no **new** error rule relative to the input under `contracts`.
/// Pre-existing input errors do not block; new warning-only findings do not block. On rejection the
/// input is returned verbatim, with the offending rule ids in `new_error_rules`. Apply this per motion
/// span so a rejected span passes through while its neighbours are still rewritten.
pub fn apply_gated(
    tp: &Toolpath,
    contracts: &Contracts,
    mode: OptimizeMode,
    kinematics: Option<&MachineKinematics>,
) -> GatedResult {
    use crate::{verify, Severity};

    apply_gated_with(tp, mode, kinematics, |candidate| {
        verify(candidate, contracts)
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.rule.clone())
            .collect()
    })
}

/// The `safe`-mode gate: [`apply_gated`] with [`OptimizeMode::Safe`]. Kept as a thin wrapper for the
/// existing callers/tests. `safe` ignores kinematics, so this passes `None`.
pub fn apply_safe_gated(tp: &Toolpath, contracts: &Contracts) -> GatedResult {
    apply_gated(tp, contracts, OptimizeMode::Safe, None)
}
