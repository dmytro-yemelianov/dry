use serde::{Deserialize, Serialize};

/// The rotary kinematics of the 5-axis machine: which two rotary axes carry the toolframe orientation,
/// and how the tool-direction unit vector maps onto them. Supports mechanical TCP (Tool Center Point)
/// translation offsets and rotary joint rotation offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kinematics {
    /// Tilting head: `A` about X then `B` about Y. Words `A`,`B`.
    Ab {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `A` about X, `C` about Z (e.g. table/trunnion). Words `A`,`C`.
    Ac {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `B` about Y, `C` about Z. Words `B`,`C`.
    Bc {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
}

/// Reference machine model used for the 5-axis task: B/C rotary axes with zero offsets.
pub const REFERENCE_FIVE_AXIS_MACHINE: Kinematics = Kinematics::Bc {
    pivot_offset: [0.0, 0.0, 0.0],
    rotary_offset: [0.0, 0.0],
};

impl Default for Kinematics {
    fn default() -> Self {
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        }
    }
}

/// Recover the unit tool-direction vector from a toolframe orientation.
///
/// `None` is the identity toolframe (`[0, 0, 1]`).
///
/// Both transforms below assume ‖v‖ = 1: `Ac`/`Bc` recover the tilt as `acos(k)`, which is the true
/// tilt only for a unit vector, so an un-normalised orientation silently lands the **linear** axes
/// on the wrong point *and* reports the wrong angle — while `Ab`, which uses `atan2`, is
/// scale-invariant and disagrees with them on identical input. Normalising once, here, is what makes
/// the three models agree. A zero or non-finite vector carries no direction at all and is refused.
fn unit_orientation(orientation: Option<[f64; 3]>) -> Result<[f64; 3], String> {
    let v = orientation.unwrap_or([0.0, 0.0, 1.0]);
    let magnitude = libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if !(magnitude.is_finite() && magnitude > 0.0) {
        return Err(format!(
            "orientation [{}, {}, {}] must have a finite non-zero magnitude",
            v[0], v[1], v[2]
        ));
    }
    Ok([v[0] / magnitude, v[1] / magnitude, v[2] / magnitude])
}

impl Kinematics {
    /// Convert a toolframe orientation into the two rotary words used by this model.
    ///
    /// The orientation is normalised first; see [`unit_orientation`].
    pub(crate) fn rotary_words(
        &self,
        orientation: Option<[f64; 3]>,
    ) -> Result<[Rotary; 2], String> {
        let [i, j, k] = unit_orientation(orientation)?;
        Ok(match *self {
            Self::Ab {
                rotary_offset,
                pivot_offset: _,
            } => {
                let a = libm::atan2(j, libm::hypot(i, k)).to_degrees() + rotary_offset[0];
                let b = libm::atan2(i, k).to_degrees() + rotary_offset[1];
                [
                    Rotary {
                        letter: 'A',
                        value: a,
                    },
                    Rotary {
                        letter: 'B',
                        value: b,
                    },
                ]
            }
            Self::Ac {
                rotary_offset,
                pivot_offset: _,
            } => {
                let c = libm::atan2(j, i).to_degrees() + rotary_offset[1];
                let a = libm::acos(k.clamp(-1.0, 1.0)).to_degrees() + rotary_offset[0];
                [
                    Rotary {
                        letter: 'C',
                        value: c,
                    },
                    Rotary {
                        letter: 'A',
                        value: a,
                    },
                ]
            }
            Self::Bc {
                rotary_offset,
                pivot_offset: _,
            } => {
                let c = libm::atan2(j, i).to_degrees() + rotary_offset[1];
                let b = libm::acos(k.clamp(-1.0, 1.0)).to_degrees() + rotary_offset[0];
                [
                    Rotary {
                        letter: 'C',
                        value: c,
                    },
                    Rotary {
                        letter: 'B',
                        value: b,
                    },
                ]
            }
        })
    }

    /// The letters of this model's two rotary words, in the order [`Self::rotary_words`] returns
    /// them — `Ab` writes `A` then `B`, `Ac` and `Bc` write `C` then their tilt axis.
    pub(crate) fn rotary_letters(&self) -> [char; 2] {
        match self {
            Self::Ab { .. } => ['A', 'B'],
            Self::Ac { .. } => ['C', 'A'],
            Self::Bc { .. } => ['C', 'B'],
        }
    }

    /// Invert [`Self::rotary_words`]: recover the tool-direction vector from two rotary word values
    /// in **degrees**, given in the order [`Self::rotary_letters`] reports.
    ///
    /// This is the import side of the forward map. `Ab` writes `a = atan2(j, hypot(i, k))` and
    /// `b = atan2(i, k)`, so on a unit vector `hypot(i, k) = cos a` and the inverse is
    /// `[cos a · sin b, sin a, cos a · cos b]`; `Ac`/`Bc` write `acos(k)` for the tilt and
    /// `atan2(j, i)` for `C`, so the inverse is `[sin t · cos c, sin t · sin c, cos t]`. Each per-axis
    /// `rotary_offset` is subtracted before the trig, mirroring the addition on the way out.
    ///
    /// The result is a unit vector by construction (`sin² + cos² = 1`), so it needs no normalisation
    /// and cannot trip `verify`'s `orientation-not-unit`. It is finite whenever the words and the
    /// model's offsets are — the words are finite by [`crate::gcode::GcodeParser`]'s word scanner and
    /// the offsets by [`Self::validate`], which the import path calls once up front exactly as
    /// `emit_stream` does. That is why this map is infallible where the forward one is not.
    ///
    /// `inverse ∘ forward` is the identity on any unit vector. `forward ∘ inverse` is the identity
    /// only on the branch the forward map can produce (`|a| ≤ 90°` for `Ab`, tilt in `[0°, 180°]`
    /// for `Ac`/`Bc`): a program stating a tilt outside it still yields the tool direction that pose
    /// points in, which is the honest reading of the words.
    pub(crate) fn orientation_from_rotary_words(&self, values: [f64; 2]) -> [f64; 3] {
        match *self {
            Self::Ab { rotary_offset, .. } => {
                let a = (values[0] - rotary_offset[0]).to_radians();
                let b = (values[1] - rotary_offset[1]).to_radians();
                let ca = libm::cos(a);
                [ca * libm::sin(b), libm::sin(a), ca * libm::cos(b)]
            }
            Self::Ac { rotary_offset, .. } => {
                let c = (values[0] - rotary_offset[1]).to_radians();
                let a = (values[1] - rotary_offset[0]).to_radians();
                let sa = libm::sin(a);
                [sa * libm::cos(c), sa * libm::sin(c), libm::cos(a)]
            }
            Self::Bc { rotary_offset, .. } => {
                let c = (values[0] - rotary_offset[1]).to_radians();
                let b = (values[1] - rotary_offset[0]).to_radians();
                let sb = libm::sin(b);
                [sb * libm::cos(c), sb * libm::sin(c), libm::cos(b)]
            }
        }
    }

    /// Convert machine workpoint `p` in WCS to MCS machine coordinates for this kinematic model.
    ///
    /// The orientation is normalised first; see [`unit_orientation`].
    pub(crate) fn machine_position(
        &self,
        p: [f64; 3],
        orientation: Option<[f64; 3]>,
    ) -> Result<[f64; 3], String> {
        let [i, j, k] = unit_orientation(orientation)?;
        Ok(match *self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            } => {
                let a_nom = libm::atan2(j, libm::hypot(i, k));
                let b_nom = libm::atan2(i, k);
                let a = a_nom + rotary_offset[0].to_radians();
                let b = b_nom + rotary_offset[1].to_radians();

                let sa = libm::sin(a);
                let ca = libm::cos(a);
                let sb = libm::sin(b);
                let cb = libm::cos(b);

                // R = R_y(b) * R_x(a)
                let lx = pivot_offset[0];
                let ly = pivot_offset[1];
                let lz = pivot_offset[2];

                let rx = cb * lx - sb * sa * ly + sb * ca * lz;
                let ry = ca * ly + sa * lz;
                let rz = -sb * lx - cb * sa * ly + cb * ca * lz;

                [p[0] - rx, p[1] - ry, p[2] - rz]
            }
            Self::Ac {
                pivot_offset,
                rotary_offset,
            } => {
                let c_nom = libm::atan2(j, i);
                let a_nom = libm::acos(k.clamp(-1.0, 1.0));
                let a = a_nom + rotary_offset[0].to_radians();
                let c = c_nom + rotary_offset[1].to_radians();

                let sa = libm::sin(a);
                let ca = libm::cos(a);
                let sc = libm::sin(c);
                let cc = libm::cos(c);

                // R_table = R_x(a) * R_z(c)
                let lx = pivot_offset[0];
                let ly = pivot_offset[1];
                let lz = pivot_offset[2];

                let px = p[0] + lx;
                let py = p[1] + ly;
                let pz = p[2] + lz;

                let rx = cc * px - sc * py;
                let ry = ca * sc * px + ca * cc * py - sa * pz;
                let rz = sa * sc * px + sa * cc * py + ca * pz;

                [rx - lx, ry - ly, rz - lz]
            }
            Self::Bc {
                pivot_offset,
                rotary_offset,
            } => {
                let c_nom = libm::atan2(j, i);
                let b_nom = libm::acos(k.clamp(-1.0, 1.0));
                let b = b_nom + rotary_offset[0].to_radians();
                let c = c_nom + rotary_offset[1].to_radians();

                let sb = libm::sin(b);
                let cb = libm::cos(b);
                let sc = libm::sin(c);
                let cc = libm::cos(c);

                // R_table = R_y(b) * R_z(c)
                let lx = pivot_offset[0];
                let ly = pivot_offset[1];
                let lz = pivot_offset[2];

                let px = p[0] + lx;
                let py = p[1] + ly;
                let pz = p[2] + lz;

                let rx = cb * cc * px - cb * sc * py + sb * pz;
                let ry = sc * px + cc * py;
                let rz = -sb * cc * px + sb * sc * py + cb * pz;

                [rx - lx, ry - ly, rz - lz]
            }
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            }
            | Self::Ac {
                pivot_offset,
                rotary_offset,
            }
            | Self::Bc {
                pivot_offset,
                rotary_offset,
            } => {
                for (axis, value) in ["x", "y", "z"].iter().zip(*pivot_offset) {
                    if !value.is_finite() {
                        return Err(format!("pivot_offset[{axis}] must be finite"));
                    }
                }
                for (axis, value) in ["0", "1"].iter().zip(*rotary_offset) {
                    if !value.is_finite() {
                        return Err(format!("rotary_offset[{axis}] must be finite"));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "ab" => Ok(Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            "ac" => Ok(Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            "bc" => Ok(Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            other => Err(format!("unknown kinematics: {other}")),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Kinematics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawKinematics {
            String(String),
            Struct(RawKinematicsStruct),
        }

        #[derive(Deserialize)]
        struct RawKinematicsStruct {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default)]
            pivot_offset: [f64; 3],
            #[serde(default)]
            rotary_offset: [f64; 2],
        }

        let raw = RawKinematics::deserialize(deserializer)?;
        match raw {
            RawKinematics::String(s) => match s.as_str() {
                "ab" | "ac" | "bc" => Kinematics::named(&s).map_err(D::Error::custom),
                other => Err(D::Error::custom(format!("unknown kinematics: {other}"))),
            },
            RawKinematics::Struct(s) => match s.kind.as_str() {
                "ab" => Ok(Kinematics::Ab {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "ac" => Ok(Kinematics::Ac {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "bc" => Ok(Kinematics::Bc {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                other => Err(D::Error::custom(format!(
                    "unknown kinematics type: {other}"
                ))),
            },
        }
    }
}

impl Serialize for Kinematics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Raw {
            #[serde(rename = "type")]
            kind: &'static str,
            #[serde(default)]
            pivot_offset: [f64; 3],
            #[serde(default)]
            rotary_offset: [f64; 2],
        }

        match self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "ab",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
            Self::Ac {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "ac",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
            Self::Bc {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "bc",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
        }
    }
}

/// One emitted rotary word: its letter and its value in **degrees**.
pub(crate) struct Rotary {
    pub(super) letter: char,
    pub(super) value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_vec(v: [f64; 3]) -> [f64; 3] {
        let norm = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / norm, v[1] / norm, v[2] / norm]
    }

    fn norm(v: [f64; 3]) -> f64 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    fn assert_point_within_epsilon(a: [f64; 3], b: [f64; 3], eps: f64) {
        assert!((a[0] - b[0]).abs() < eps);
        assert!((a[1] - b[1]).abs() < eps);
        assert!((a[2] - b[2]).abs() < eps);
    }

    #[test]
    fn machine_position_preserves_reference_radius_for_zero_pivot_models() {
        let point = [10.0, -7.0, 4.25];
        let orientations = [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            unit_vec([0.2, 0.6, 0.76]),
            unit_vec([0.9, -0.3, 0.316227766016838]),
        ];
        let reference_radius = norm(point);

        let models = [
            Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
        ];

        for model in models {
            for orientation in orientations {
                let projected = model
                    .machine_position(point, Some(unit_vec(orientation)))
                    .unwrap();
                let projected_radius = norm(projected);
                assert!(
                    (projected_radius - reference_radius).abs() < 1e-10,
                    "machine-position should preserve distance-to-origin for zero-pivot models"
                );
            }
        }
    }

    /// `orientation_from_rotary_words` is the exact inverse of `rotary_words`, with no g-code
    /// formatting in between: this isolates the trig from the emitter's 6-decimal word rounding,
    /// which is what dominates the end-to-end round-trip error measured in
    /// `tests/five_axis_import.rs`.
    ///
    /// Every vector here is off the singular cone (`|k| < 1`, `hypot(i, j) > 0`). On the cone the
    /// second word is not recoverable from the *forward* map — `atan2(0, 0)` and `hypot(i, k) = 0`
    /// throw away the axis the tool is symmetric about — so a round-trip there would be measuring the
    /// forward map's loss, not this inverse. (The recovered *vector* is still correct on the cone,
    /// because the lost word multiplies a zero sine; that is not what this test is for.)
    #[test]
    fn rotary_words_invert_back_to_the_orientation_they_came_from() {
        let orientations = [
            unit_vec([0.36, 0.48, 0.8]),
            unit_vec([0.6, 0.0, 0.8]),
            unit_vec([0.0, 0.6, 0.8]),
            unit_vec([-0.36, 0.48, 0.8]),
            unit_vec([0.48, 0.36, -0.8]),
            unit_vec([1.0, 1.0, 0.0]),
            unit_vec([-0.2, -0.7, 0.3]),
        ];
        let models = [
            Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            // a machine whose rotary joints are not zeroed: the offset must be subtracted on the way
            // back in, not added a second time.
            Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [10.0, -5.0],
            },
            Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [-7.5, 21.0],
            },
            Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [3.25, -90.0],
            },
        ];
        for model in models {
            for orientation in orientations {
                let words = model.rotary_words(Some(orientation)).unwrap();
                let back = model.orientation_from_rotary_words([words[0].value, words[1].value]);
                // measured worst case over this matrix: 1.67e-16 per component, i.e. rounding.
                for axis in 0..3 {
                    assert!(
                        (back[axis] - orientation[axis]).abs() < 1e-15,
                        "{model:?}: {orientation:?} -> {} {} / {} {} -> {back:?}",
                        words[0].letter,
                        words[0].value,
                        words[1].letter,
                        words[1].value,
                    );
                }
                // and the recovered vector is unit *by construction*, so it can never trip
                // `verify`'s `orientation-not-unit`. Measured `|‖v‖ - 1|` over this matrix: exactly 0.
                assert!((norm(back) - 1.0).abs() < 1e-15, "{back:?} is not unit");
            }
        }
    }

    #[test]
    fn machine_position_ab_model_identity_with_zero_pivot_offset() {
        let point = [2.5, -6.0, 11.75];
        let model = Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        };
        let orientations = [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            unit_vec([0.2, 0.6, 0.76]),
            unit_vec([0.9, -0.3, 0.316227766016838]),
        ];

        for orientation in orientations {
            let projected = model
                .machine_position(point, Some(unit_vec(orientation)))
                .unwrap();
            assert_point_within_epsilon(projected, point, 1e-12);
        }
    }
}
