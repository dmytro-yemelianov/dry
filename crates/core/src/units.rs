//! Typed physical quantities — units are *types*, so mixed-unit arithmetic is a compile error
//! (`docs/01-architecture.md` §3). The cross-dimensional operators below encode the physics **once**:
//! `Length × Length = Area`, `Area × Length = Volume`, `Volume ÷ Area = Length` (filament feedstock),
//! `Length ÷ Feedrate = Time`, `Volume ÷ Time = Flow`, `Length × Angle = Length` (arc length). There is
//! deliberately no operator that mixes incompatible units (e.g. `Length + Feedrate`), so a unit
//! confusion in `resolve`/`simulate`/`emit` is a *compile* error, not a silent wrong number.
//!
//! Each quantity is `#[serde(transparent)]`, so the Dry IR (de)serialises as bare numbers — the wire
//! format and the emitted g-code stay byte-identical to the conformance corpora.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

/// Define a physical quantity newtype over `f64` with the within-dimension scalar algebra
/// (add/sub/negate, scale by a scalar). Cross-dimensional products/quotients are defined explicitly.
macro_rules! quantity {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub f64);

        impl $name {
            /// The additive identity.
            pub const ZERO: $name = $name(0.0);
            /// The underlying scalar (for formatting / boundary code only).
            #[inline]
            pub fn value(self) -> f64 {
                self.0
            }
        }
        impl Add for $name {
            type Output = $name;
            #[inline]
            fn add(self, rhs: $name) -> $name {
                $name(self.0 + rhs.0)
            }
        }
        impl Sub for $name {
            type Output = $name;
            #[inline]
            fn sub(self, rhs: $name) -> $name {
                $name(self.0 - rhs.0)
            }
        }
        impl Neg for $name {
            type Output = $name;
            #[inline]
            fn neg(self) -> $name {
                $name(-self.0)
            }
        }
        // scale by a dimensionless scalar (both orders) and divide by one.
        impl Mul<f64> for $name {
            type Output = $name;
            #[inline]
            fn mul(self, k: f64) -> $name {
                $name(self.0 * k)
            }
        }
        impl Mul<$name> for f64 {
            type Output = $name;
            #[inline]
            fn mul(self, q: $name) -> $name {
                $name(self * q.0)
            }
        }
        impl Div<f64> for $name {
            type Output = $name;
            #[inline]
            fn div(self, k: f64) -> $name {
                $name(self.0 / k)
            }
        }
    };
}

quantity!(
    /// A length, in millimetres.
    Length
);
quantity!(
    /// An area, in mm².
    Area
);
quantity!(
    /// A volume, in mm³.
    Volume
);
quantity!(
    /// A feedrate (machine speed), in mm/min — the g-code `F` value.
    Feedrate
);
quantity!(
    /// A duration, in seconds.
    Time
);
quantity!(
    /// A volumetric flow rate, in mm³/s.
    Flow
);
quantity!(
    /// A plane angle, in radians.
    Angle
);

impl Length {
    #[inline]
    pub fn mm(value: f64) -> Length {
        Length(value)
    }
    /// Euclidean hypotenuse of two lengths (a length).
    #[inline]
    pub fn hypot(self, other: Length) -> Length {
        Length(self.0.hypot(other.0))
    }
    /// The angle of the vector `(other, self)` from the +x axis (`atan2(self, other)`).
    #[inline]
    pub fn atan2(self, other: Length) -> Angle {
        Angle(self.0.atan2(other.0))
    }
}

impl Area {
    /// The side length of the square with this area (a length).
    #[inline]
    pub fn sqrt(self) -> Length {
        Length(self.0.sqrt())
    }
}

// ---- the cross-dimensional physics (defined once) ----

/// `Length × Length = Area`.
impl Mul<Length> for Length {
    type Output = Area;
    #[inline]
    fn mul(self, rhs: Length) -> Area {
        Area(self.0 * rhs.0)
    }
}
/// `Area × Length = Volume`.
impl Mul<Length> for Area {
    type Output = Volume;
    #[inline]
    fn mul(self, rhs: Length) -> Volume {
        Volume(self.0 * rhs.0)
    }
}
/// `Length × Area = Volume`.
impl Mul<Area> for Length {
    type Output = Volume;
    #[inline]
    fn mul(self, rhs: Area) -> Volume {
        Volume(self.0 * rhs.0)
    }
}
/// `Volume ÷ Area = Length` — the feedstock length consumed for a deposited volume.
impl Div<Area> for Volume {
    type Output = Length;
    #[inline]
    fn div(self, rhs: Area) -> Length {
        Length(self.0 / rhs.0)
    }
}
/// `Length × Angle = Length` — arc length is radius times swept angle.
impl Mul<Angle> for Length {
    type Output = Length;
    #[inline]
    fn mul(self, rhs: Angle) -> Length {
        Length(self.0 * rhs.0)
    }
}
/// `Length ÷ Feedrate = Time` (seconds). Feedrate is mm/min, so the quotient (minutes) is ×60.
impl Div<Feedrate> for Length {
    type Output = Time;
    #[inline]
    fn div(self, rhs: Feedrate) -> Time {
        Time(self.0 / rhs.0 * 60.0)
    }
}
/// `Volume ÷ Time = Flow` (mm³/s).
impl Div<Time> for Volume {
    type Output = Flow;
    #[inline]
    fn div(self, rhs: Time) -> Flow {
        Flow(self.0 / rhs.0)
    }
}
/// Angle wraps modulo a scalar (e.g. `% TAU`).
impl Rem<f64> for Angle {
    type Output = Angle;
    #[inline]
    fn rem(self, k: f64) -> Angle {
        Angle(self.0 % k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_arithmetic_is_typed() {
        assert_eq!(Length::mm(2.0) + Length::mm(3.0), Length::mm(5.0));
        assert_eq!(Length::mm(5.0) - Length::mm(3.0), Length::mm(2.0));
        assert_eq!(Length::ZERO, Length::mm(0.0));
    }

    #[test]
    fn the_dimensional_algebra_encodes_the_physics() {
        // Length × Length = Area; Area × Length = Volume
        assert_eq!(Length::mm(2.0) * Length::mm(3.0), Area(6.0));
        assert_eq!(Area(6.0) * Length::mm(2.0), Volume(12.0));
        assert_eq!(Length::mm(2.0) * Area(6.0), Volume(12.0));
        // Volume ÷ Area = Length (the filament feedstock length)
        assert_eq!(Volume(12.0) / Area(6.0), Length::mm(2.0));
        // Area square-root is a Length
        assert_eq!(Area(9.0).sqrt(), Length::mm(3.0));
        // Length ÷ Feedrate = Time in SECONDS (feedrate is mm/min, so ×60)
        assert_eq!(Length::mm(100.0) / Feedrate(1000.0), Time(6.0));
        // Volume ÷ Time = Flow (mm³/s)
        assert_eq!(Volume(12.0) / Time(6.0), Flow(2.0));
        // arc length = radius × swept angle (Length × Angle = Length)
        assert_eq!(Length::mm(10.0) * Angle(2.0), Length::mm(20.0));
        // hypot / atan2 carry the right dimensions
        assert_eq!(Length::mm(3.0).hypot(Length::mm(4.0)), Length::mm(5.0));
        assert_eq!(Length::mm(0.0).atan2(Length::mm(1.0)), Angle(0.0));
    }

    #[test]
    fn quantities_serialise_as_bare_numbers() {
        // #[serde(transparent)] keeps the wire format byte-identical (no `{"0": ..}` wrapping).
        assert_eq!(serde_json::to_string(&Length::mm(0.2)).unwrap(), "0.2");
        assert_eq!(serde_json::to_string(&Volume(12.0)).unwrap(), "12.0");
        assert_eq!(
            serde_json::from_str::<Length>("0.2").unwrap(),
            Length::mm(0.2)
        );
    }
}
