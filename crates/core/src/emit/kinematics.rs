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
///
/// The offsets stay zero deliberately: a zero-pivot table is the one configuration whose forward
/// transform is exactly a rotation about the WCS origin, so every emitted 5-axis program is
/// reproducible without a machine-specific calibration. What the model does *not* carry is any
/// limit — see [`REFERENCE_FIVE_AXIS_LIMITS`] for those.
pub const REFERENCE_FIVE_AXIS_MACHINE: Kinematics = Kinematics::Bc {
    pivot_offset: [0.0, 0.0, 0.0],
    rotary_offset: [0.0, 0.0],
};

/// The limits of the reference 5-axis machine: what its rotary axes can reach, how fast they can get
/// there, and where the tool tip is allowed to end up.
///
/// **These numbers are illustrative, not sourced from any real machine's datasheet.** Dry does not
/// ship a model of a specific trunnion mill, and inventing a plausible-looking spec sheet would claim
/// an authority these values do not have. They are chosen to be *representative in shape* — a tilt
/// that runs out before the table can turn the part upside down, a rotary rate slower than the linear
/// axes, a cube-ish envelope — so that the rules gated on them are exercised against something with
/// the right character. Any real deployment supplies its own `machine.rotary` block, and this constant
/// is never applied implicitly: nothing in `Profile::contracts` reads it.
///
/// The specific choices, and what each one is doing:
///
/// - `travel_deg.b = [0, 120]`. Under [`Kinematics::Bc`] the tilt word is `acos(k)`, so `B` is already
///   confined to `[0, 180]` by construction; a limit is only meaningful when it is *tighter* than
///   that. 120° is the honest shape of a trunnion: it can tip the work well past vertical but cannot
///   flip it over, so `B = 180` (tool pointing at −Z) is out of reach.
/// - `travel_deg.c` is absent. `C = atan2(j, i)` already lands in `(−180, 180]`, and a continuously
///   rotating C axis has no travel limit to state — an absent axis is unconstrained, which is the
///   truthful encoding, and a `[−360, 360]` that can never fire would be a vacuous limit.
/// - `max_rotary_feed_deg_min = 3600` (60 °/s). Slower than the linear axes, which is what makes a
///   synchronised reorientation the constraint rather than the linear feed.
/// - `envelope_mm` is the *machine*-coordinate box the tool tip must stay inside once the rotation is
///   applied. Deliberately not symmetric in Z: the head can lift well above the table but only a
///   little below it, which is the geometry that makes tilting a far-out point unreachable.
pub const REFERENCE_FIVE_AXIS_LIMITS: crate::verify::RotaryContracts =
    crate::verify::RotaryContracts {
        model: REFERENCE_FIVE_AXIS_MACHINE,
        travel_deg: Some(crate::verify::RotaryTravelRanges {
            a: None,
            b: Some([0.0, 120.0]),
            c: None,
        }),
        max_rotary_feed_deg_min: Some(3600.0),
        envelope_mm: Some([[-200.0, 200.0], [-200.0, 200.0], [-50.0, 300.0]]),
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

/// Sine of the tool tilt at or below which the `C` axis carries no direction: the singular cone.
///
/// `Ac` and `Bc` both recover `C` as `atan2(j, i)`. As the tool approaches ±Z the tilt `hypot(i, j)`
/// falls to zero and `C` stops being determined by the direction at all; at exactly ±Z `atan2(0, 0)`
/// returns `0`, which is a C library return value rather than a choice. Emitting it swings a real
/// rotary axis mid-cut for no geometric reason — and because the linear axes are expressed in the
/// rotated table frame, they swing with it.
///
/// The threshold is derived from the emitter's own word resolution, not tuned. Substituting any `C`
/// for the computed one moves the tool direction by at most `2·asin(hypot(i, j))` radians (the two
/// directions sit on a circle of radius `hypot(i, j)` about ±Z). At `1e-9` that is `2e-9` rad =
/// `1.15e-7°`, an order of magnitude below the `1e-6°` that [`super::gcode::num`]'s `{v:.6}` can
/// print — so holding `C` can never change a word the program is able to express. Any larger epsilon
/// would let this policy alter emitted geometry silently.
const SINGULAR_CONE_SIN_TILT: f64 = 1e-9;

/// The `C`-axis state carried from one segment to the next.
///
/// [`Kinematics::resolve_joints`] cannot be a pure per-segment function: inside the singular cone
/// `C` is undetermined, and the only defensible answer is where the previous segment left the axis,
/// which is history. `emit_stream_to_writer` threads this the way it already threads `prog_pos`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RotaryState {
    /// Last determined `C`, in **radians**, nominal (before `rotary_offset`).
    ///
    /// Seeded at `0` — the identity — by `Default`. On the first move there is no previous
    /// orientation, and the program cannot know where the operator left the axis, so the identity is
    /// the only value it can assert. It is also what [`unit_orientation`] already substitutes for a
    /// missing orientation (`[0, 0, 1]`), so a program that *starts* inside the cone is byte-identical
    /// to one emitted before this state existed. A program that *enters* the cone differs,
    /// deliberately: entering carries a determined `C`, starting carries none.
    c: f64,
}

/// One segment's rotary joint angles in **radians**, nominal (before `rotary_offset`), in the order
/// this model emits its words: `Ab` ⇒ `(A, B)`, `Ac` ⇒ `(C, A)`, `Bc` ⇒ `(C, B)`.
///
/// Resolving them once per segment is load-bearing, not tidiness. [`Kinematics::rotary_words`] and
/// [`Kinematics::machine_position`] each used to recompute `atan2(j, i)` from the orientation. Once
/// `C` can be *held* rather than computed, a held value reaching only one of them would emit rotary
/// words for one machine state and linear words for another: under `Bc` at `B = 0`, holding
/// `C = 90°` while the linear transform still assumed `C = 0` puts the programmed point a quarter
/// turn about Z away from the metal.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Joints([f64; 2]);

/// Resolve the `C` angle for a tool direction, holding the previous value inside the singular cone.
///
/// Advancing the state *is* the resolution: outside the cone the direction determines `C` and the
/// state records it; inside, the recorded value is reused unchanged.
fn resolve_c(i: f64, j: f64, state: &mut RotaryState) -> f64 {
    if libm::hypot(i, j) > SINGULAR_CONE_SIN_TILT {
        state.c = libm::atan2(j, i);
    }
    state.c
}

impl Kinematics {
    /// Resolve one segment's rotary joint angles, advancing the `C`-axis state.
    ///
    /// The orientation is normalised first; see [`unit_orientation`]. This is the only fallible step
    /// of the mapping — everything downstream is a function of the angles alone.
    ///
    /// `Ab` never touches the state: it has no `C` axis. Its own singularity — `B = atan2(i, k)` with
    /// the tool along ±Y, where `i = k = 0` — is the exact analogue of the one handled here and is
    /// **not** addressed; a tilting head passing through horizontal still swings `B` to zero.
    pub(crate) fn resolve_joints(
        &self,
        orientation: Option<[f64; 3]>,
        state: &mut RotaryState,
    ) -> Result<Joints, String> {
        let [i, j, k] = unit_orientation(orientation)?;
        Ok(match *self {
            Self::Ab { .. } => Joints([libm::atan2(j, libm::hypot(i, k)), libm::atan2(i, k)]),
            Self::Ac { .. } | Self::Bc { .. } => {
                Joints([resolve_c(i, j, state), libm::acos(k.clamp(-1.0, 1.0))])
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

    /// Convert resolved joint angles into the two rotary words used by this model.
    pub(crate) fn rotary_words(&self, joints: Joints) -> [Rotary; 2] {
        let Joints([first, second]) = joints;
        match *self {
            Self::Ab {
                rotary_offset,
                pivot_offset: _,
            } => [
                Rotary {
                    letter: 'A',
                    value: first.to_degrees() + rotary_offset[0],
                },
                Rotary {
                    letter: 'B',
                    value: second.to_degrees() + rotary_offset[1],
                },
            ],
            Self::Ac {
                rotary_offset,
                pivot_offset: _,
            } => [
                Rotary {
                    letter: 'C',
                    value: first.to_degrees() + rotary_offset[1],
                },
                Rotary {
                    letter: 'A',
                    value: second.to_degrees() + rotary_offset[0],
                },
            ],
            Self::Bc {
                rotary_offset,
                pivot_offset: _,
            } => [
                Rotary {
                    letter: 'C',
                    value: first.to_degrees() + rotary_offset[1],
                },
                Rotary {
                    letter: 'B',
                    value: second.to_degrees() + rotary_offset[0],
                },
            ],
        }
    }

    /// Convert machine workpoint `p` in WCS to MCS machine coordinates for this kinematic model.
    ///
    /// Takes the same [`Joints`] the rotary words are rendered from, so the linear and rotary halves
    /// of a line always describe one machine state.
    pub(crate) fn machine_position(&self, p: [f64; 3], joints: Joints) -> [f64; 3] {
        let Joints([first, second]) = joints;
        match *self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            } => {
                let a = first + rotary_offset[0].to_radians();
                let b = second + rotary_offset[1].to_radians();

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
                let c = first + rotary_offset[1].to_radians();
                let a = second + rotary_offset[0].to_radians();

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
                let c = first + rotary_offset[1].to_radians();
                let b = second + rotary_offset[0].to_radians();

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
        }
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
///
/// `pub(crate)` fields rather than `pub(super)`: `verify` reads the same words the emitter writes, so
/// the rotary rules judge the program that will actually be produced rather than a second derivation
/// of it.
pub(crate) struct Rotary {
    pub(crate) letter: char,
    pub(crate) value: f64,
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

    /// Resolve joints from a fresh (identity-seeded) state, as a one-segment program would.
    fn joints_of(model: Kinematics, orientation: [f64; 3]) -> Joints {
        model
            .resolve_joints(Some(orientation), &mut RotaryState::default())
            .unwrap()
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
                let projected =
                    model.machine_position(point, joints_of(model, unit_vec(orientation)));
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
                // A fresh RotaryState per case: this test is about the trig round-tripping, not
                // about the cone-hold, and a held C would make the inverse depend on history.
                let words = model.rotary_words(joints_of(model, orientation));
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
            let projected = model.machine_position(point, joints_of(model, unit_vec(orientation)));
            assert_point_within_epsilon(projected, point, 1e-12);
        }
    }

    /// The C axis stops being determined by the tool direction inside the cone, so the resolver must
    /// return the previous value rather than `atan2(0, 0)`.
    #[test]
    fn c_is_held_inside_the_singular_cone_and_recomputed_outside_it() {
        for model in [
            Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
            Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            },
        ] {
            let mut state = RotaryState::default();
            // Seed: tool tilted toward +Y ⇒ C = 90°.
            let tilted = model
                .resolve_joints(Some(unit_vec([0.0, 1.0, 0.2])), &mut state)
                .unwrap();
            assert!((tilted.0[0].to_degrees() - 90.0).abs() < 1e-12);

            // Straight up, and every direction whose tilt is inside the cone, holds it.
            for inside in [
                [0.0, 0.0, 1.0],
                [-1e-17, 0.0, 1.0],
                [0.0, SINGULAR_CONE_SIN_TILT, 1.0],
                [-SINGULAR_CONE_SIN_TILT, 0.0, 1.0],
            ] {
                let held = model.resolve_joints(Some(inside), &mut state).unwrap();
                assert!(
                    (held.0[0].to_degrees() - 90.0).abs() < 1e-12,
                    "{inside:?} should hold C = 90°, got {}",
                    held.0[0].to_degrees()
                );
            }

            // One decade outside the cone the direction determines C again.
            let outside = model
                .resolve_joints(Some([-1e-8, 0.0, 1.0]), &mut state)
                .unwrap();
            assert!((outside.0[0].to_degrees() - 180.0).abs() < 1e-12);
        }
    }

    /// The bound that makes the epsilon defensible: holding C instead of computing it can move the
    /// tool direction by at most `2·asin(SINGULAR_CONE_SIN_TILT)` radians, which is below the
    /// `1e-6°` the emitter's `{v:.6}` word format can print.
    #[test]
    fn holding_c_inside_the_cone_stays_below_the_emitted_word_resolution() {
        let model = Kinematics::Bc {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        };
        let worst_case_rad = 2.0 * libm::asin(SINGULAR_CONE_SIN_TILT);
        assert!(worst_case_rad.to_degrees() < 1e-6);

        // Sample the cone boundary at the worst possible held value (180° away from the true C) and
        // measure the angle between the direction the program describes and the true direction.
        let mut worst_measured: f64 = 0.0;
        for step in 0..=36 {
            let theta = f64::from(step) * 10.0_f64.to_radians();
            let h = SINGULAR_CONE_SIN_TILT;
            let direction = unit_vec([h * libm::cos(theta), h * libm::sin(theta), 1.0]);
            let mut state = RotaryState {
                c: theta + std::f64::consts::PI,
            };
            let joints = model.resolve_joints(Some(direction), &mut state).unwrap();
            let (c, b) = (joints.0[0], joints.0[1]);
            let described = [
                libm::sin(b) * libm::cos(c),
                libm::sin(b) * libm::sin(c),
                libm::cos(b),
            ];
            let dot = described[0] * direction[0]
                + described[1] * direction[1]
                + described[2] * direction[2];
            worst_measured = worst_measured.max(libm::acos(dot.clamp(-1.0, 1.0)));
            assert!(
                worst_measured <= worst_case_rad,
                "measured worst-case direction error {worst_measured} exceeds the published \
                 {worst_case_rad} rad bound"
            );
        }
    }
}

/// The `J5` magnitude, in radians, at or below which the spherical wrist is treated as singular and
/// `J4` is held at its previous value rather than recomputed.
///
/// Same shape as [`SINGULAR_CONE_SIN_TILT`] and the same reason: at `J5 = 0` the wrist's fourth and
/// sixth axes become colinear, so `atan2(r03_t_orient[1], r03_t_orient[0])` is reading a direction
/// the pose no longer determines, and spinning `J4` to whatever it returns is motion with no
/// geometric meaning. Deliberately **much coarser** than the `1e-9` cone threshold, and it is worth
/// being explicit about why the two differ by four orders of magnitude rather than leaving the
/// asymmetry to be rediscovered: the cone epsilon is bounded by what a `{v:.6}` G-code word can
/// print, whereas this one guards a *joint-space* solve whose consumers are robot post-processors
/// with their own interpolation. `1e-5` rad is 5.7e-4°, which is above a typical robot's repeatability
/// but small enough that the substituted `J4` cannot move the TCP appreciably.
///
/// Recorded as `policy` rather than `bounded` in the numeric profile: unlike the cone threshold, no
/// worst-case TCP error has been derived for it. That derivation is the open obligation registered
/// with `FM1.F64.EMIT.ROBOT6.WRIST_SINGULAR_HOLD`.
const WRIST_SINGULAR_J5_RAD: f64 = 1e-5;

/// Standard Denavit-Hartenberg (DH) parameter for a revolute robot joint.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DhParam {
    /// Link length $a_{i-1}$ (mm).
    pub a: f64,
    /// Link twist $\alpha_{i-1}$ (radians).
    pub alpha: f64,
    /// Link offset $d_i$ (mm).
    pub d: f64,
    /// Joint angle offset $\theta_{\text{offset}}$ (radians).
    pub theta_offset: f64,
}

impl DhParam {
    pub const fn new(a: f64, alpha: f64, d: f64, theta_offset: f64) -> Self {
        Self {
            a,
            alpha,
            d,
            theta_offset,
        }
    }
}

/// 6-Axis robot joint angles in degrees $(J_1 \dots J_6)$.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct RobotJoints6 {
    pub j1_deg: f64,
    pub j2_deg: f64,
    pub j3_deg: f64,
    pub j4_deg: f64,
    pub j5_deg: f64,
    pub j6_deg: f64,
}

impl RobotJoints6 {
    pub const fn new(j1: f64, j2: f64, j3: f64, j4: f64, j5: f64, j6: f64) -> Self {
        Self {
            j1_deg: j1,
            j2_deg: j2,
            j3_deg: j3,
            j4_deg: j4,
            j5_deg: j5,
            j6_deg: j6,
        }
    }

    pub fn to_radians(&self) -> [f64; 6] {
        [
            self.j1_deg.to_radians(),
            self.j2_deg.to_radians(),
            self.j3_deg.to_radians(),
            self.j4_deg.to_radians(),
            self.j5_deg.to_radians(),
            self.j6_deg.to_radians(),
        ]
    }
}

/// 6-Axis articulated serial robot arm model with analytical FK and IK.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Robot6AxisModel {
    pub name: String,
    pub dh: [DhParam; 6],
    pub joint_limits_deg: [(f64, f64); 6],
}

impl Robot6AxisModel {
    /// Create reference KUKA KR6 R900 robot arm model.
    pub fn kuka_kr6_r900() -> Self {
        use std::f64::consts::FRAC_PI_2;
        Self {
            name: "KUKA KR6 R900 sixx".into(),
            dh: [
                DhParam::new(25.0, -FRAC_PI_2, 400.0, 0.0),
                DhParam::new(455.0, 0.0, 0.0, 0.0),
                DhParam::new(35.0, -FRAC_PI_2, 0.0, 0.0),
                DhParam::new(0.0, FRAC_PI_2, 420.0, 0.0),
                DhParam::new(0.0, -FRAC_PI_2, 0.0, 0.0),
                DhParam::new(0.0, 0.0, 80.0, 0.0),
            ],
            joint_limits_deg: [
                (-170.0, 170.0),
                (-190.0, 45.0),
                (-120.0, 156.0),
                (-185.0, 185.0),
                (-120.0, 120.0),
                (-350.0, 350.0),
            ],
        }
    }

    /// Compute Forward Kinematics (FK): maps joint angles to Cartesian Tool Center Point (TCP) $[x, y, z]$.
    ///
    /// **Precondition: every joint angle is finite.** The forward solves return plain values and have
    /// no error channel to refuse into, unlike [`Self::solve_ik`], so a non-finite joint propagates
    /// into the result rather than being rejected. Giving them a `Result` was considered and declined:
    /// it would cascade through `multi_robot`'s clearance code for no additional safety, because the
    /// consumer that matters already validates. `check_dual_robot_clearance` and
    /// `check_continuous_dual_robot_trajectory` — the collision checks these feed — refuse non-finite
    /// joints, base offsets, link radii and safety margins at their own boundary and fail closed. A
    /// caller using FK for anything safety-bearing should validate the same way.
    pub fn solve_fk(&self, joints: &RobotJoints6) -> [f64; 3] {
        let (pos, _) = self.solve_fk_pose(joints);
        pos
    }

    /// Compute Forward Kinematics (FK) returning both TCP position $[x, y, z]$ and toolframe $Z$-axis vector $[i, j, k]$.
    pub fn solve_fk_pose(&self, joints: &RobotJoints6) -> ([f64; 3], [f64; 3]) {
        let rads = joints.to_radians();
        let mut transform = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        for (i, &rad) in rads.iter().enumerate() {
            let theta = rad + self.dh[i].theta_offset;
            let ct = libm::cos(theta);
            let st = libm::sin(theta);
            let ca = libm::cos(self.dh[i].alpha);
            let sa = libm::sin(self.dh[i].alpha);
            let a = self.dh[i].a;
            let d = self.dh[i].d;

            let a_mat = [
                [ct, -st * ca, st * sa, a * ct],
                [st, ct * ca, -ct * sa, a * st],
                [0.0, sa, ca, d],
                [0.0, 0.0, 0.0, 1.0],
            ];

            transform = mat4_mul(&transform, &a_mat);
        }

        let pos = [transform[0][3], transform[1][3], transform[2][3]];
        let orient = [transform[0][2], transform[1][2], transform[2][2]];
        (pos, orient)
    }

    /// Compute 3D Cartesian coordinates for all 6 link frame origins along the kinematic chain.
    pub fn solve_all_link_positions(&self, joints: &RobotJoints6) -> [[f64; 3]; 6] {
        let rads = joints.to_radians();
        let mut transform = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut links = [[0.0; 3]; 6];

        for (i, &rad) in rads.iter().enumerate() {
            let theta = rad + self.dh[i].theta_offset;
            let ct = libm::cos(theta);
            let st = libm::sin(theta);
            let ca = libm::cos(self.dh[i].alpha);
            let sa = libm::sin(self.dh[i].alpha);
            let a = self.dh[i].a;
            let d = self.dh[i].d;

            let a_mat = [
                [ct, -st * ca, st * sa, a * ct],
                [st, ct * ca, -ct * sa, a * st],
                [0.0, sa, ca, d],
                [0.0, 0.0, 0.0, 1.0],
            ];

            transform = mat4_mul(&transform, &a_mat);
            links[i] = [transform[0][3], transform[1][3], transform[2][3]];
        }

        links
    }

    /// Compute Inverse Kinematics (IK) with spherical wrist singularity hold ($J_5 \approx 0$).
    ///
    /// **This is a five-degree-of-freedom solve returned in a six-joint shape.** `J6` is not solved
    /// and is always `0.0`: the tool roll about its own axis is not determined by a TCP point plus a
    /// tool *direction*, and this signature takes no roll reference to determine it from. A caller
    /// that needs roll must set it themselves. Likewise only the elbow-up branch is produced, so the
    /// solver cannot follow a path that requires reconfiguration.
    ///
    /// Refuses what carries no pose, on the same terms as [`crate::emit::kinematics::unit_orientation`] on the 5-axis path:
    /// a non-finite TCP point, a zero or non-finite tool direction, or a non-finite previous joint
    /// state. Before this the refusals disagreed — `NaN.abs() > 1.0` is false, so the reach check
    /// below never fired on non-finite input and the solve returned `Ok` with `NaN` in every joint,
    /// which is the class `H1.1`/`H1.2` closed everywhere else. The direction is also *normalised*
    /// here rather than used raw: it scales the wrist-centre offset `d6`, so a non-unit vector
    /// silently displaced the wrist centre by `d6·(‖v‖ − 1)` and returned confidently wrong joints.
    pub fn solve_ik(
        &self,
        tcp_pos: [f64; 3],
        tool_orient: [f64; 3],
        prev_joints: &RobotJoints6,
    ) -> Result<RobotJoints6, String> {
        if !tcp_pos.iter().all(|v| v.is_finite()) {
            return Err(format!(
                "TCP position [{}, {}, {}] must be finite",
                tcp_pos[0], tcp_pos[1], tcp_pos[2]
            ));
        }
        if !prev_joints.to_radians().iter().all(|v| v.is_finite()) {
            return Err("previous joint state must be finite in every joint".to_string());
        }
        let tool_orient = unit_orientation(Some(tool_orient))?;

        let d6 = self.dh[5].d;
        // Wrist center position
        let wx = tcp_pos[0] - d6 * tool_orient[0];
        let wy = tcp_pos[1] - d6 * tool_orient[1];
        let wz = tcp_pos[2] - d6 * tool_orient[2];

        // J1 = atan2(wy, wx)
        let j1_rad = libm::atan2(wy, wx);

        // Position relative to shoulder (J1)
        let d1 = self.dh[0].d;
        let a1 = self.dh[0].a;
        let r = libm::hypot(wx, wy) - a1;
        let s = d1 - wz;
        let d_sq = r * r + s * s;

        let a2 = self.dh[1].a;
        let a3 = self.dh[2].a;
        let d4 = self.dh[3].d;
        let l3 = libm::hypot(a3, d4);

        let cos_phi = (d_sq - a2 * a2 - l3 * l3) / (2.0 * a2 * l3);
        if cos_phi.abs() > 1.0 {
            return Err("Target point is outside robot reachable workspace envelope".into());
        }

        let delta = libm::atan2(d4, a3);
        let sin_phi = (1.0 - cos_phi * cos_phi).max(0.0).sqrt();
        let phi = libm::atan2(sin_phi, cos_phi);
        let j3_rad = phi - delta;

        let j2_rad = libm::atan2(s, r) - libm::atan2(l3 * sin_phi, a2 + l3 * cos_phi);

        let mut t03 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let thetas = [j1_rad, j2_rad, j3_rad];
        for (i, &th) in thetas.iter().enumerate() {
            let theta = th + self.dh[i].theta_offset;
            let ct = libm::cos(theta);
            let st = libm::sin(theta);
            let ca = libm::cos(self.dh[i].alpha);
            let sa = libm::sin(self.dh[i].alpha);
            let a = self.dh[i].a;
            let d = self.dh[i].d;

            let a_mat = [
                [ct, -st * ca, st * sa, a * ct],
                [st, ct * ca, -ct * sa, a * st],
                [0.0, sa, ca, d],
                [0.0, 0.0, 0.0, 1.0],
            ];
            t03 = mat4_mul(&t03, &a_mat);
        }

        // Relative tool orientation in frame 3: R03^T * tool_orient
        let r03_t_orient = [
            t03[0][0] * tool_orient[0] + t03[1][0] * tool_orient[1] + t03[2][0] * tool_orient[2],
            t03[0][1] * tool_orient[0] + t03[1][1] * tool_orient[1] + t03[2][1] * tool_orient[2],
            t03[0][2] * tool_orient[0] + t03[1][2] * tool_orient[1] + t03[2][2] * tool_orient[2],
        ];

        // In standard DH with alpha4=+PI/2, alpha5=-PI/2:
        // When J4=0, J5=0: tool points along frame 3 +Z axis
        let r_xy = libm::hypot(r03_t_orient[0], r03_t_orient[1]);
        let mut j5_rad = libm::atan2(r_xy, r03_t_orient[2]);
        let j4_rad: f64;
        let j6_rad: f64 = 0.0;

        // Wrist singularity handling: if J5 is near 0 rad, hold J4 to previous state
        if j5_rad.abs() < WRIST_SINGULAR_J5_RAD {
            j4_rad = prev_joints.j4_deg.to_radians();
            j5_rad = 0.0;
        } else {
            j4_rad = libm::atan2(r03_t_orient[1], r03_t_orient[0]);
        }

        let joints = RobotJoints6::new(
            j1_rad.to_degrees(),
            j2_rad.to_degrees(),
            j3_rad.to_degrees(),
            j4_rad.to_degrees(),
            j5_rad.to_degrees(),
            j6_rad.to_degrees(),
        );

        Ok(joints)
    }
}

fn mat4_mul(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut res = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            res[i][j] =
                a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
    res
}
