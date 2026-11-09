use super::*;

#[test]
fn source_lower_clamp_canonicalizes_negative_zero() {
    assert_eq!(clamp_fraction(-0.).to_bits(), 0f32.to_bits());
}
use anyhow::Context;
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
struct Fixture {
    planes: BTreeMap<u16, VecDeque<[f32; 4]>>,
    children: BTreeMap<(u16, u8), i16>,
    reads: Vec<(u16, i8)>,
}
impl Fixture {
    fn node(&mut self, index: u16, plane: [f32; 4], children: [i16; 2]) {
        self.planes.insert(index, VecDeque::from([plane]));
        self.children.insert((index, 0), children[0]);
        self.children.insert((index, 1), children[1]);
    }
}
impl SegmentSource for Fixture {
    fn plane(&mut self, node: u16) -> Result<[f32; 4]> {
        self.reads.push((node, -1));
        let values = self.planes.get_mut(&node).context("unreadable plane")?;
        if values.len() > 1 {
            Ok(values.pop_front().unwrap())
        } else {
            values.front().copied().context("empty plane sequence")
        }
    }
    fn child(&mut self, node: u16, slot: u8) -> Result<i16> {
        self.reads.push((node, slot as i8));
        self.children
            .get(&(node, slot))
            .copied()
            .context("unreadable child")
    }
}
fn limits() -> SegmentLimits {
    SegmentLimits {
        node_tests: 100,
        leaf_visits: 100,
        pending_segments: 32,
    }
}
fn point(x: f32) -> [f32; 4] {
    [x, 0., 0., f32::NAN]
}

#[test]
fn zero_tie_is_front_and_w_is_overwritten_without_reading_other_child() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., -2.], [-4, -5]);
    f.children.remove(&(0, 1));
    let result = query(&mut f, point(2.), point(3.), 6, limits()).unwrap();
    assert_eq!(result.zones, [4]);
    assert_eq!(f.reads, [(0, -1), (0, 0)]);
    assert_eq!(result.leaf_visits, 1);
    assert!(!result.capacity_reached);
}

#[test]
fn crossings_follow_near_first_but_read_far_child_first() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., 0.], [-1, -2]);
    let result = query(&mut f, point(-1.), point(1.), 3, limits()).unwrap();
    assert_eq!(result.zones, [2, 1]);
    assert_eq!(f.reads, [(0, -1), (0, 0), (0, 1)]);
    assert_eq!(result.maximum_pending_segments, 1);
}

#[test]
fn decreasing_overlap_and_increasing_epsilon_have_distinct_clipped_geometry() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., 0.], [1, 2]);
    f.node(1, [1., 0., 0., SPLIT_EPSILON * 0.5], [0, -2]);
    f.node(2, [1., 0., 0., -SPLIT_EPSILON * 0.5], [-3, -1]);
    let decreasing = query(&mut f, point(1.), point(-1.), 4, limits()).unwrap();
    assert_eq!(decreasing.zones, [0, 2, 3, 1]);
    // For increasing travel, both root split numerators add epsilon. There is
    // no symmetric overlap around zero, and node two stays entirely negative.
    let increasing = query(&mut f, point(-1.), point(1.), 4, limits()).unwrap();
    assert_eq!(increasing.zones, [1, 2, 0]);
}

#[test]
fn duplicates_are_suppressed_but_shared_node_planes_are_reloaded() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., 0.], [1, 1]);
    f.node(1, [0., 0., 0., 1.], [-2, -3]);
    f.planes.get_mut(&1).unwrap().push_back([0., 0., 0., -1.]);
    let result = query(&mut f, point(1.), point(-1.), 4, limits()).unwrap();
    assert_eq!(result.zones, [2, 3]);
    assert_eq!(result.node_tests, 3);
    assert_eq!(
        f.reads,
        [(0, -1), (0, 1), (0, 0), (1, -1), (1, 0), (1, -1), (1, 1)]
    );
    let mut duplicate = Fixture::default();
    duplicate.node(0, [1., 0., 0., 0.], [-2, -2]);
    let result = query(&mut duplicate, point(1.), point(-1.), 3, limits()).unwrap();
    assert_eq!(result.zones, [2]);
    assert_eq!(result.leaf_visits, 2);
}

#[test]
fn signed_output_scan_preserves_extreme_halfword_behavior() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., 0.], [i16::MIN, i16::MIN]);
    let result = query(&mut f, point(1.), point(-1.), 32769, limits()).unwrap();
    assert_eq!(result.zones, [32768, 32768]);
}

#[test]
fn sixteenth_append_stops_before_addressing_pending_node() {
    let mut f = Fixture::default();
    // Each successive near node sees the same plane and continues crossing.
    // A balanced sequence of constant endpoint-independent split planes is
    // unnecessary: nested epsilon overlap keeps these same-plane splits live.
    f.node(0, [1., 0., 0., 0.], [1, 99]);
    for index in 1..=16u16 {
        f.node(
            index,
            [1., 0., 0., 0.],
            [
                if index == 16 { -16 } else { (index + 1) as i16 },
                -((index - 1) as i16),
            ],
        );
    }
    let result = query(&mut f, point(1.), point(-1.), 17, limits()).unwrap();
    assert_eq!(result.zones, (1..=16).rev().collect::<Vec<_>>());
    assert!(result.capacity_reached);
    assert!(!f.reads.iter().any(|&(node, _)| node == 99));
}

#[test]
fn paired_projection_keeps_fused_z_cancellation_sign() {
    let mut f = Fixture::default();
    let step = f32::EPSILON;
    f.node(0, [-1., 0., 1. - step, 0.], [0, -1]);
    f.children.remove(&(0, 0));
    let p = [1., 0., 1. + step, f32::NAN];
    // Exact product minus one is -step^2. Rounding the product before
    // subtraction would lose its sign and select the unreadable front child.
    assert!(((1. + f64::from(step)) * (1. - f64::from(step)) - 1.) < 0.);
    let result = query(&mut f, p, p, 2, limits()).unwrap();
    assert_eq!(result.zones, [1]);
    assert_eq!(f.reads, [(0, -1), (0, 1)]);
}

#[test]
fn budgets_cycles_and_addressed_errors_fail_instead_of_returning_clear() {
    let mut cycle = Fixture::default();
    cycle.node(0, [0., 0., 0., 1.], [1, 0]);
    cycle.node(1, [0., 0., 0., 1.], [1, 0]);
    let error = query(
        &mut cycle,
        point(0.),
        point(0.),
        1,
        SegmentLimits {
            node_tests: 3,
            ..limits()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("node budget"));
    assert_eq!(
        cycle.reads.iter().filter(|&&(_, slot)| slot == -1).count(),
        3
    );
    let mut crossing = Fixture::default();
    crossing.node(0, [1., 0., 0., 0.], [1, 0]);
    crossing.node(1, [1., 0., 0., 0.], [0, 0]);
    let error = query(
        &mut crossing,
        point(1.),
        point(-1.),
        1,
        SegmentLimits {
            pending_segments: 1,
            ..limits()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("pending budget"));
    assert_eq!(crossing.reads.last(), Some(&(1, -1)));
    let mut missing = Fixture::default();
    missing.node(0, [1., 0., 0., 0.], [-1, -2]);
    missing.children.remove(&(0, 1));
    assert!(query(&mut missing, point(1.), point(-1.), 3, limits()).is_err());
    assert_eq!(missing.reads, [(0, -1), (0, 1)]);
}

#[test]
fn leaf_budget_zone_range_and_nonfinite_addressed_inputs_are_checked() {
    let mut f = Fixture::default();
    f.node(0, [1., 0., 0., 0.], [-1, -1]);
    assert!(
        query(
            &mut f,
            point(1.),
            point(-1.),
            2,
            SegmentLimits {
                leaf_visits: 1,
                ..limits()
            }
        )
        .unwrap_err()
        .to_string()
        .contains("leaf budget")
    );
    assert!(
        query(&mut f, point(1.), point(2.), 1, limits())
            .unwrap_err()
            .to_string()
            .contains("zone directory")
    );
    f.planes.insert(0, VecDeque::from([[f32::NAN, 0., 0., 0.]]));
    assert!(query(&mut f, point(1.), point(2.), 2, limits()).is_err());
}
