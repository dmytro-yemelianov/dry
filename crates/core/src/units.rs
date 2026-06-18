//! Typed physical quantities — units are *types*, so mixed-unit arithmetic is a compile error
//! (`docs/01-architecture.md` §3). Seeded with `Length`; the full set (Speed/Volume/Flow/Temperature/
//! Angle) and the integration into the IR fields land in P0.2.

/// A length in millimetres — a distinct type from a bare `f64`, so it cannot be confused with a speed,
/// a volume or an angle.
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

#[cfg(test)]
mod tests {
    use super::Length;

    #[test]
    fn length_arithmetic_is_typed() {
        assert_eq!(Length::mm(2.0) + Length::mm(3.0), Length::mm(5.0));
        assert_eq!(Length::mm(5.0) - Length::mm(3.0), Length::mm(2.0));
        assert_eq!(Length::ZERO, Length::mm(0.0));
    }
}
