//! # dry-core — the Dry IR + engine (foundations)
//!
//! This is the dependency-free core of Dry (no PyO3, no numpy), the seed of the architecture in
//! `docs/01-architecture.md`. It is intentionally a skeleton at Phase 0: the module layout encodes the
//! design (units, the multi-level IR dialects, the pass framework, the engine surface), and the pieces
//! are filled in against the conformance corpora ported from the FullControl fork (`docs/03-conformance.md`).
//!
//! Status: **P0** — scaffold only. See `docs/04-tasks.md`.

#![forbid(unsafe_code)]

/// Typed physical quantities. Units are *types*, so mixed-unit arithmetic is a compile error
/// (`docs/01-architecture.md` §3). Seeded with `Length`; the rest land in P0.2.
pub mod units {
    /// A length in millimetres. A distinct type from a bare `f64` so it cannot be confused with a
    /// speed, a volume or an angle. (Full quantity set + dimensional checking: task P0.2.)
    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
    pub struct Length(pub f64);

    impl Length {
        pub const ZERO: Length = Length(0.0);
        #[inline]
        pub fn mm(value: f64) -> Length {
            Length(value)
        }
    }

    impl std::ops::Add for Length {
        type Output = Length;
        #[inline]
        fn add(self, rhs: Length) -> Length {
            Length(self.0 + rhs.0)
        }
    }
    impl std::ops::Sub for Length {
        type Output = Length;
        #[inline]
        fn sub(self, rhs: Length) -> Length {
            Length(self.0 - rhs.0)
        }
    }
    // Note: there is deliberately no `impl Add<Speed> for Length` — mixing units must not compile.
}

/// The multi-level Dry IR: L0 design → L1 path → L2 motion → L3 target (`docs/01-architecture.md` §1).
/// Each dialect gets its node set + a verifier; lowering passes move between levels. Defined in P0.2.
pub mod ir {
    // TODO P0.2: dialect node types (toolframe, channels), columnar L2 storage, JSON + binary encodings.
}

/// The pass framework: lowering (L0→L1→L2→L3) + optimisation, each declaring preconditions and the
/// invariants it preserves (`docs/01-architecture.md` §4). Defined from P2/P3.
pub mod pass {
    // TODO: pass trait + pass manager; ports of the fork's ir/passes.py + gcode_engine/passes/.
}

/// The engine surface: `lower` / `simulate` / `verify` / `optimise` / `emit` / `parse` / `reverse`
/// (`docs/01-architecture.md` §7). Filled in across P1–P3, gated on the conformance corpora.
pub mod engine {
    // TODO P1+: the analyses and lowerings, byte-identical to the FullControl fork on the goldens.
}

#[cfg(test)]
mod tests {
    use super::units::Length;

    #[test]
    fn length_arithmetic_is_typed() {
        assert_eq!(Length::mm(2.0) + Length::mm(3.0), Length::mm(5.0));
        assert_eq!(Length::mm(5.0) - Length::mm(3.0), Length::mm(2.0));
        assert_eq!(Length::ZERO, Length::mm(0.0));
    }
}
