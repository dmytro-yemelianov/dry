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

impl Default for Kinematics {
    fn default() -> Self {
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        }
    }
}

impl Kinematics {
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
pub(super) struct Rotary {
    pub(super) letter: char,
    pub(super) value: f64,
}

/// Map a toolframe orientation (tool-direction unit vector) to the two rotary words for `kinematics`,
/// in source order. `None` ⇒ identity (+Z) ⇒ all-zero angles. Conventions (each documented on
/// [`Kinematics`]):
///
/// - **AB**: `B = atan2(i, k)` (lead in X-Z), `A = atan2(j, hypot(i, k))` (tilt toward Y).
/// - **AC**: `C = atan2(j, i)` (azimuth about Z), `A = acos(k)` (polar tilt from +Z).
/// - **BC**: `C = atan2(j, i)`, `B = acos(k)`.
///
/// `+Z` gives `atan2(0, 0) = 0` and `acos(1) = 0`, so every convention yields zeros there.
pub(super) fn tool_rotaries(orientation: Option<[f64; 3]>, kinematics: Kinematics) -> [Rotary; 2] {
    let [i, j, k] = orientation.unwrap_or([0.0, 0.0, 1.0]);
    match kinematics {
        Kinematics::Ab {
            pivot_offset: _,
            rotary_offset,
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
        Kinematics::Ac {
            pivot_offset: _,
            rotary_offset,
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
        Kinematics::Bc {
            pivot_offset: _,
            rotary_offset,
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
    }
}
