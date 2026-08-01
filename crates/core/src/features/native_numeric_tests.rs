use super::{radians_from_degrees, ExpandError, FeaturePose, Transform};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use serde::Deserialize;

const FIXTURES: &str =
    include_str!("../../../../proofs/fixtures/native-feature-numeric-interval-v0.json");

const ANGLE_BUDGET_ID: &str = "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.ANGLE_RAD_ABS_ERROR";
const TRIG_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.TRIG_COEFFICIENT_ABS_ERROR";
const COMPOSE_ROTATION_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSE_ROTATION_COMPONENT_ABS_ERROR";
const COMPOSE_TRANSLATION_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.COMPOSE_TRANSLATION_COMPONENT_ABS_ERROR_MM";
const POINT_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.POINT_COMPONENT_ABS_ERROR_MM";
const ARC_CENTER_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.ARC_CENTER_COMPONENT_ABS_ERROR_MM";
const ORIENTATION_BUDGET_ID: &str =
    "FM1.NUMERIC.PROFILE.FEATURE.PLANAR.V0.BUDGET.ORIENTATION_COMPONENT_ABS_ERROR";

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    schema_version: u32,
    model: String,
    model_checks: bool,
    budgets: Budgets,
    limits: Limits,
    pose_cases: Vec<PoseCase>,
    compose_cases: Vec<ComposeCase>,
    point_application_cases: Vec<PointApplicationCase>,
    xy_application_cases: Vec<XYApplicationCase>,
    vector_application_cases: Vec<VectorApplicationCase>,
}

#[derive(Debug, Deserialize)]
struct Budgets {
    angle: Budget,
    trig: Budget,
    compose_rotation: Budget,
    compose_translation: Budget,
    point: Budget,
    arc_center: Budget,
    orientation: Budget,
}

#[derive(Debug, Deserialize)]
struct Budget {
    id: String,
    ceiling: f64,
}

#[derive(Debug, Deserialize)]
struct Limits {
    local_coordinate_abs: f64,
    pose_translation_abs: f64,
    pose_rotation_abs_deg: f64,
    arc_center_abs: f64,
    orientation_component_abs: f64,
    multiply_exact_result_abs: f64,
    add_sub_exact_result_abs: f64,
    radian_intermediate_abs: f64,
}

#[derive(Debug, Deserialize)]
struct PoseCase {
    id: String,
    pose: FixturePose,
    radian_bounds: [f64; 2],
    expected_coefficient: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct ComposeCase {
    id: String,
    parent: FixturePose,
    local: FixturePose,
    expected_coefficient: [f64; 2],
    expected_translation: [f64; 3],
}

#[derive(Debug, Deserialize)]
struct PointApplicationCase {
    id: String,
    transform: FixturePose,
    point: [f64; 3],
    expected: [f64; 3],
}

#[derive(Debug, Deserialize)]
struct XYApplicationCase {
    id: String,
    transform: FixturePose,
    point: [f64; 2],
    expected: [f64; 2],
}

#[derive(Debug, Deserialize)]
struct VectorApplicationCase {
    id: String,
    transform: FixturePose,
    vector: [f64; 3],
    expected: [f64; 3],
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct FixturePose {
    x: f64,
    y: f64,
    z: f64,
    rotate_z_deg: f64,
}

impl FixturePose {
    fn to_core(self) -> FeaturePose {
        FeaturePose {
            x: self.x,
            y: self.y,
            z: self.z,
            rotate_z_deg: self.rotate_z_deg,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dyadic {
    significand: BigInt,
    exponent: i32,
}

impl Dyadic {
    fn new(significand: BigInt, exponent: i32) -> Self {
        if significand.is_zero() {
            return Self {
                significand,
                exponent: 0,
            };
        }
        Self {
            significand,
            exponent,
        }
    }

    fn from_f64(value: f64) -> Self {
        assert!(value.is_finite(), "dyadic conversion requires a finite f64");
        if value == 0.0 {
            return Self::new(BigInt::zero(), 0);
        }
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let encoded_exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (unsigned_significand, exponent) = if encoded_exponent == 0 {
            (fraction, -1074)
        } else {
            ((1_u64 << 52) | fraction, encoded_exponent - 1023 - 52)
        };
        let significand = BigInt::from(unsigned_significand);
        Self::new(if negative { -significand } else { significand }, exponent)
    }

    fn neg(&self) -> Self {
        Self::new(-&self.significand, self.exponent)
    }

    fn add(&self, other: &Self) -> Self {
        if self.significand.is_zero() {
            return other.clone();
        }
        if other.significand.is_zero() {
            return self.clone();
        }
        let exponent = self.exponent.min(other.exponent);
        let left = self.scaled_significand(exponent);
        let right = other.scaled_significand(exponent);
        Self::new(left + right, exponent)
    }

    fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    fn mul(&self, other: &Self) -> Self {
        Self::new(
            &self.significand * &other.significand,
            self.exponent
                .checked_add(other.exponent)
                .expect("selected fixture exponent fits i32"),
        )
    }

    fn abs_le_f64(&self, ceiling: f64) -> bool {
        assert!(ceiling.is_finite() && ceiling >= 0.0);
        let bound = Self::from_f64(ceiling);
        let exponent = self.exponent.min(bound.exponent);
        self.scaled_significand(exponent).abs() <= bound.scaled_significand(exponent).abs()
    }

    fn scaled_significand(&self, exponent: i32) -> BigInt {
        debug_assert!(exponent <= self.exponent);
        let shift = usize::try_from(self.exponent - exponent).expect("nonnegative shift");
        &self.significand << shift
    }
}

fn assert_budget(budget: &Budget, expected_id: &str, expected_ceiling: f64) {
    assert_eq!(budget.id, expected_id);
    assert_eq!(
        budget.ceiling.to_bits(),
        expected_ceiling.to_bits(),
        "{expected_id} ceiling drift"
    );
}

fn assert_approx(actual: f64, expected: f64, ceiling: f64, context: &str) {
    assert!(
        actual.is_finite() && (actual - expected).abs() <= ceiling,
        "{context}: expected {expected}, observed {actual}, ceiling {ceiling}"
    );
}

fn assert_dyadic_error(actual: f64, exact: &Dyadic, ceiling: f64, context: &str) {
    let error = Dyadic::from_f64(actual).sub(exact);
    assert!(
        error.abs_le_f64(ceiling),
        "{context}: observed {actual}, exact dyadic {exact:?}, ceiling {ceiling}"
    );
}

fn checked_mul(left: &Dyadic, right: &Dyadic, limit: f64, context: &str) -> Dyadic {
    let exact = left.mul(right);
    assert!(
        exact.abs_le_f64(limit),
        "{context}: exact multiplication exceeds profiled limit {limit}: {exact:?}"
    );
    exact
}

fn checked_add(left: &Dyadic, right: &Dyadic, limit: f64, context: &str) -> Dyadic {
    let exact = left.add(right);
    assert!(
        exact.abs_le_f64(limit),
        "{context}: exact addition exceeds profiled limit {limit}: {exact:?}"
    );
    exact
}

fn checked_sub(left: &Dyadic, right: &Dyadic, limit: f64, context: &str) -> Dyadic {
    let exact = left.sub(right);
    assert!(
        exact.abs_le_f64(limit),
        "{context}: exact subtraction exceeds profiled limit {limit}: {exact:?}"
    );
    exact
}

fn transform_from_fixture(id: &str, pose: FixturePose) -> Result<Transform, ExpandError> {
    Transform::from_pose(pose.to_core(), id)
}

fn exact_rotated_xy(
    transform: Transform,
    point: [f64; 2],
    limits: &Limits,
    context: &str,
) -> [Dyadic; 2] {
    let cosine = Dyadic::from_f64(transform.cos);
    let sine = Dyadic::from_f64(transform.sin);
    let x = Dyadic::from_f64(point[0]);
    let y = Dyadic::from_f64(point[1]);
    let multiply_limit = limits.multiply_exact_result_abs;
    let add_limit = limits.add_sub_exact_result_abs;
    [
        checked_sub(
            &checked_mul(&cosine, &x, multiply_limit, &format!("{context}.x.0")),
            &checked_mul(&sine, &y, multiply_limit, &format!("{context}.x.1")),
            add_limit,
            &format!("{context}.x"),
        ),
        checked_add(
            &checked_mul(&sine, &x, multiply_limit, &format!("{context}.y.0")),
            &checked_mul(&cosine, &y, multiply_limit, &format!("{context}.y.1")),
            add_limit,
            &format!("{context}.y"),
        ),
    ]
}

fn selected(id: &str, witness: &Option<String>) -> bool {
    witness.as_ref().is_none_or(|selected| selected == id)
}

#[test]
fn native_f64_matches_lean_numeric_intervals() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid native numeric fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "native-feature-numeric-interval-v0");
    assert!(document.model_checks);
    assert_eq!(document.pose_cases.len(), 4);
    assert_eq!(document.compose_cases.len(), 3);
    assert_eq!(document.point_application_cases.len(), 2);
    assert_eq!(document.xy_application_cases.len(), 2);
    assert_eq!(document.vector_application_cases.len(), 2);

    assert_budget(&document.budgets.angle, ANGLE_BUDGET_ID, 2.0_f64.powi(-46));
    assert_budget(&document.budgets.trig, TRIG_BUDGET_ID, 2.0_f64.powi(-45));
    assert_budget(
        &document.budgets.compose_rotation,
        COMPOSE_ROTATION_BUDGET_ID,
        2.0_f64.powi(-29),
    );
    assert_budget(
        &document.budgets.compose_translation,
        COMPOSE_TRANSLATION_BUDGET_ID,
        2.0_f64.powi(-28),
    );
    assert_budget(&document.budgets.point, POINT_BUDGET_ID, 2.0_f64.powi(-28));
    assert_budget(
        &document.budgets.arc_center,
        ARC_CENTER_BUDGET_ID,
        2.0_f64.powi(-28),
    );
    assert_budget(
        &document.budgets.orientation,
        ORIENTATION_BUDGET_ID,
        2.0_f64.powi(-29),
    );
    assert_eq!(document.limits.local_coordinate_abs, 2.0_f64.powi(20));
    assert_eq!(document.limits.pose_translation_abs, 2.0_f64.powi(20));
    assert_eq!(document.limits.pose_rotation_abs_deg, 360.0);
    assert_eq!(document.limits.arc_center_abs, 2.0_f64.powi(20));
    assert_eq!(document.limits.orientation_component_abs, 1.0);
    assert_eq!(document.limits.multiply_exact_result_abs, 2.0_f64.powi(20));
    assert_eq!(document.limits.add_sub_exact_result_abs, 2.0_f64.powi(22));
    assert_eq!(document.limits.radian_intermediate_abs, 7.0);

    let witness = std::env::var("DRY_NUMERIC_MUTATION_WITNESS").ok();
    let selected_count = document
        .pose_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
        .count()
        + document
            .compose_cases
            .iter()
            .filter(|fixture| selected(&fixture.id, &witness))
            .count()
        + document
            .point_application_cases
            .iter()
            .filter(|fixture| selected(&fixture.id, &witness))
            .count()
        + document
            .xy_application_cases
            .iter()
            .filter(|fixture| selected(&fixture.id, &witness))
            .count()
        + document
            .vector_application_cases
            .iter()
            .filter(|fixture| selected(&fixture.id, &witness))
            .count();
    if let Some(witness_id) = &witness {
        assert_eq!(
            selected_count, 1,
            "numeric mutation witness {witness_id:?} must name exactly one fixture"
        );
    }

    for fixture in document
        .pose_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
    {
        let pose = fixture.pose;
        assert!(pose.rotate_z_deg.abs() <= document.limits.pose_rotation_abs_deg);
        for value in [pose.x, pose.y, pose.z] {
            assert!(value.abs() <= document.limits.pose_translation_abs);
        }

        let radians = radians_from_degrees(pose.rotate_z_deg);
        assert!(radians.abs() <= document.limits.radian_intermediate_abs);
        assert!(
            radians >= fixture.radian_bounds[0] - document.budgets.angle.ceiling
                && radians <= fixture.radian_bounds[1] + document.budgets.angle.ceiling,
            "{} radians: expected [{}, {}] ± {}, observed {}",
            fixture.id,
            fixture.radian_bounds[0],
            fixture.radian_bounds[1],
            document.budgets.angle.ceiling,
            radians
        );

        let transform = transform_from_fixture(&fixture.id, pose).unwrap();
        assert_approx(
            transform.cos,
            fixture.expected_coefficient[0],
            document.budgets.trig.ceiling,
            &format!("{} cosine", fixture.id),
        );
        assert_approx(
            transform.sin,
            fixture.expected_coefficient[1],
            document.budgets.trig.ceiling,
            &format!("{} sine", fixture.id),
        );
        for (axis, expected) in [pose.x, pose.y, pose.z].into_iter().enumerate() {
            assert_eq!(
                transform.translation[axis].to_bits(),
                expected.to_bits(),
                "{} copied translation axis {axis}",
                fixture.id
            );
        }
    }

    for fixture in document
        .compose_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
    {
        let parent =
            transform_from_fixture(&format!("{}.parent", fixture.id), fixture.parent).unwrap();
        let local =
            transform_from_fixture(&format!("{}.local", fixture.id), fixture.local).unwrap();
        let actual = parent.compose(local);

        let pc = Dyadic::from_f64(parent.cos);
        let ps = Dyadic::from_f64(parent.sin);
        let lc = Dyadic::from_f64(local.cos);
        let ls = Dyadic::from_f64(local.sin);
        let [ltx, lty, ltz] = local.translation.map(Dyadic::from_f64);
        let [ptx, pty, ptz] = parent.translation.map(Dyadic::from_f64);
        let multiply_limit = document.limits.multiply_exact_result_abs;
        let add_limit = document.limits.add_sub_exact_result_abs;

        let exact_cos = checked_sub(
            &checked_mul(&pc, &lc, multiply_limit, &format!("{}.cos.0", fixture.id)),
            &checked_mul(&ps, &ls, multiply_limit, &format!("{}.cos.1", fixture.id)),
            add_limit,
            &format!("{}.cos", fixture.id),
        );
        let exact_sin = checked_add(
            &checked_mul(&ps, &lc, multiply_limit, &format!("{}.sin.0", fixture.id)),
            &checked_mul(&pc, &ls, multiply_limit, &format!("{}.sin.1", fixture.id)),
            add_limit,
            &format!("{}.sin", fixture.id),
        );
        let exact_tx = checked_add(
            &checked_sub(
                &checked_mul(&pc, &ltx, multiply_limit, &format!("{}.tx.0", fixture.id)),
                &checked_mul(&ps, &lty, multiply_limit, &format!("{}.tx.1", fixture.id)),
                add_limit,
                &format!("{}.tx.rotated", fixture.id),
            ),
            &ptx,
            add_limit,
            &format!("{}.tx", fixture.id),
        );
        let exact_ty = checked_add(
            &checked_add(
                &checked_mul(&ps, &ltx, multiply_limit, &format!("{}.ty.0", fixture.id)),
                &checked_mul(&pc, &lty, multiply_limit, &format!("{}.ty.1", fixture.id)),
                add_limit,
                &format!("{}.ty.rotated", fixture.id),
            ),
            &pty,
            add_limit,
            &format!("{}.ty", fixture.id),
        );
        let exact_tz = checked_add(&ltz, &ptz, add_limit, &format!("{}.tz", fixture.id));

        assert_dyadic_error(
            actual.cos,
            &exact_cos,
            document.budgets.compose_rotation.ceiling,
            &format!("{} composed cosine", fixture.id),
        );
        assert_dyadic_error(
            actual.sin,
            &exact_sin,
            document.budgets.compose_rotation.ceiling,
            &format!("{} composed sine", fixture.id),
        );
        for (axis, exact) in [exact_tx, exact_ty, exact_tz].into_iter().enumerate() {
            assert_dyadic_error(
                actual.translation[axis],
                &exact,
                document.budgets.compose_translation.ceiling,
                &format!("{} composed translation axis {axis}", fixture.id),
            );
        }

        assert_approx(
            actual.cos,
            fixture.expected_coefficient[0],
            2.0_f64.powi(-10),
            &format!("{} end-to-end cosine", fixture.id),
        );
        assert_approx(
            actual.sin,
            fixture.expected_coefficient[1],
            2.0_f64.powi(-10),
            &format!("{} end-to-end sine", fixture.id),
        );
        for (axis, expected) in fixture.expected_translation.into_iter().enumerate() {
            assert_approx(
                actual.translation[axis],
                expected,
                2.0_f64.powi(-10),
                &format!("{} selected end-to-end translation axis {axis}", fixture.id),
            );
        }
    }

    for fixture in document
        .point_application_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
    {
        for value in fixture.point {
            assert!(value.abs() <= document.limits.local_coordinate_abs);
        }
        let transform =
            transform_from_fixture(&format!("{}.transform", fixture.id), fixture.transform)
                .unwrap();
        let actual = transform.apply_point(fixture.point);
        let [rotated_x, rotated_y] = exact_rotated_xy(
            transform,
            [fixture.point[0], fixture.point[1]],
            &document.limits,
            &fixture.id,
        );
        let exact = [
            checked_add(
                &rotated_x,
                &Dyadic::from_f64(transform.translation[0]),
                document.limits.add_sub_exact_result_abs,
                &format!("{}.translated.x", fixture.id),
            ),
            checked_add(
                &rotated_y,
                &Dyadic::from_f64(transform.translation[1]),
                document.limits.add_sub_exact_result_abs,
                &format!("{}.translated.y", fixture.id),
            ),
            checked_add(
                &Dyadic::from_f64(fixture.point[2]),
                &Dyadic::from_f64(transform.translation[2]),
                document.limits.add_sub_exact_result_abs,
                &format!("{}.translated.z", fixture.id),
            ),
        ];
        for (axis, exact_axis) in exact.iter().enumerate() {
            assert_dyadic_error(
                actual[axis],
                exact_axis,
                document.budgets.point.ceiling,
                &format!("{} applied point axis {axis}", fixture.id),
            );
            assert_approx(
                actual[axis],
                fixture.expected[axis],
                2.0_f64.powi(-10),
                &format!("{} selected end-to-end point axis {axis}", fixture.id),
            );
        }
    }

    for fixture in document
        .xy_application_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
    {
        for value in fixture.point {
            assert!(value.abs() <= document.limits.arc_center_abs);
        }
        let transform =
            transform_from_fixture(&format!("{}.transform", fixture.id), fixture.transform)
                .unwrap();
        let actual = transform.apply_xy(fixture.point);
        let rotated = exact_rotated_xy(transform, fixture.point, &document.limits, &fixture.id);
        let exact = [
            checked_add(
                &rotated[0],
                &Dyadic::from_f64(transform.translation[0]),
                document.limits.add_sub_exact_result_abs,
                &format!("{}.translated.x", fixture.id),
            ),
            checked_add(
                &rotated[1],
                &Dyadic::from_f64(transform.translation[1]),
                document.limits.add_sub_exact_result_abs,
                &format!("{}.translated.y", fixture.id),
            ),
        ];
        for (axis, exact_axis) in exact.iter().enumerate() {
            assert_dyadic_error(
                actual[axis],
                exact_axis,
                document.budgets.arc_center.ceiling,
                &format!("{} applied arc center axis {axis}", fixture.id),
            );
            assert_approx(
                actual[axis],
                fixture.expected[axis],
                2.0_f64.powi(-10),
                &format!("{} selected end-to-end arc center axis {axis}", fixture.id),
            );
        }
    }

    for fixture in document
        .vector_application_cases
        .iter()
        .filter(|fixture| selected(&fixture.id, &witness))
    {
        for value in fixture.vector {
            assert!(value.abs() <= document.limits.orientation_component_abs);
        }
        let transform =
            transform_from_fixture(&format!("{}.transform", fixture.id), fixture.transform)
                .unwrap();
        let actual = transform.apply_vector(fixture.vector);
        let exact = exact_rotated_xy(
            transform,
            [fixture.vector[0], fixture.vector[1]],
            &document.limits,
            &fixture.id,
        );
        for (axis, exact_axis) in exact.iter().enumerate() {
            assert_dyadic_error(
                actual[axis],
                exact_axis,
                document.budgets.orientation.ceiling,
                &format!("{} applied orientation axis {axis}", fixture.id),
            );
            assert_approx(
                actual[axis],
                fixture.expected[axis],
                2.0_f64.powi(-10),
                &format!("{} selected end-to-end orientation axis {axis}", fixture.id),
            );
        }
        assert_eq!(
            actual[2].to_bits(),
            fixture.vector[2].to_bits(),
            "{} orientation Z must be copied bit-for-bit",
            fixture.id
        );
        assert_eq!(
            actual[2].to_bits(),
            fixture.expected[2].to_bits(),
            "{} selected end-to-end orientation Z",
            fixture.id
        );
    }
}
