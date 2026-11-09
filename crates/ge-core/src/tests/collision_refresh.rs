use super::*;
use anyhow::Context;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone)]
enum Node {
    Leaves(Vec<(u8, u16)>),
    Branches(Vec<(StoredBounds, u32)>),
}
#[derive(Default)]
struct Fixture {
    nodes: BTreeMap<u32, Node>,
    placements: BTreeMap<u16, Option<StoredBounds>>,
    child_counts: BTreeMap<u32, VecDeque<u8>>,
    leaf_counts: BTreeMap<u32, VecDeque<u16>>,
    reads: Vec<(u32, &'static str, usize)>,
    fail_write: Option<(u32, u8)>,
}
impl Fixture {
    fn node(&mut self, node: u32, operation: &'static str, index: usize) -> Result<Node> {
        self.reads.push((node, operation, index));
        self.nodes
            .get(&node)
            .cloned()
            .context("missing hierarchy node")
    }
}
impl RefreshHierarchy for Fixture {
    type Node = u32;
    fn child_count(&mut self, node: u32) -> Result<u8> {
        let count = match self.node(node, "children", 0)? {
            Node::Branches(v) => v.len().try_into()?,
            Node::Leaves(_) => 0,
        };
        Ok(self
            .child_counts
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(count))
    }
    fn leaf_count(&mut self, node: u32) -> Result<u16> {
        let count = match self.node(node, "leaves", 0)? {
            Node::Leaves(v) => v.len().try_into()?,
            _ => anyhow::bail!("not leaf"),
        };
        Ok(self
            .leaf_counts
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(count))
    }
    fn leaf_placement(&mut self, node: u32, index: u16) -> Result<u16> {
        match self.node(node, "placement", usize::from(index))? {
            Node::Leaves(v) => Ok(v[usize::from(index)].1),
            _ => anyhow::bail!("not leaf"),
        }
    }
    fn placement_base_bounds(&mut self, placement: u16) -> Result<Option<StoredBounds>> {
        self.reads.push((u32::from(placement), "resolve", 0));
        self.placements
            .get(&placement)
            .copied()
            .context("unavailable placement bounds")
    }
    fn branch_bounds(&mut self, node: u32, index: u8) -> Result<StoredBounds> {
        match self.node(node, "seed", usize::from(index))? {
            Node::Branches(v) => Ok(v[usize::from(index)].0),
            _ => anyhow::bail!("not internal"),
        }
    }
    fn branch_target(&mut self, node: u32, index: u8) -> Result<u32> {
        match self.node(node, "target", usize::from(index))? {
            Node::Branches(v) => Ok(v[usize::from(index)].1),
            _ => anyhow::bail!("not internal"),
        }
    }
    fn write_branch_bounds(&mut self, node: u32, index: u8, bounds: StoredBounds) -> Result<()> {
        self.reads.push((node, "write", usize::from(index)));
        if self.fail_write == Some((node, index)) {
            anyhow::bail!("branch write failed");
        }
        match self.nodes.get_mut(&node).context("missing writable node")? {
            Node::Branches(v) => {
                v[usize::from(index)].0 = bounds;
                Ok(())
            }
            _ => anyhow::bail!("not internal"),
        }
    }
}
fn box3(low: f32, high: f32) -> StoredBounds {
    StoredBounds {
        minimum: [low; 3],
        maximum: [high; 3],
    }
}
fn limits() -> RefreshLimits {
    RefreshLimits {
        node_visits: 100,
        entry_tests: 100,
    }
}
fn assert_bounds(actual: Bounds, low: f32, high: f32) {
    assert_eq!(actual.minimum[..3], [low; 3]);
    assert_eq!(actual.maximum[..3], [high; 3]);
}
fn branch(f: &Fixture, node: u32, index: usize) -> StoredBounds {
    match &f.nodes[&node] {
        Node::Branches(v) => v[index].0,
        _ => panic!("not internal"),
    }
}

#[test]
fn existing_child_seeds_grow_without_shrinking_and_root_seed_is_explicit() {
    let mut f = Fixture::default();
    f.nodes
        .insert(0, Node::Branches(vec![(box3(-10., 10.), 1)]));
    f.nodes.insert(1, Node::Leaves(vec![(0, 7), (0xff, 8)]));
    f.placements.insert(7, Some(box3(-1., 2.)));
    f.placements.insert(8, Some(box3(-20., 3.)));
    let mut accumulator = unpack(box3(-30., -25.));
    let stats = refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_eq!(branch(&f, 0, 0).minimum, [-20.; 3]);
    assert_eq!(branch(&f, 0, 0).maximum, [10.; 3]);
    assert_bounds(accumulator, -30., 10.);
    assert_eq!(
        (
            stats.branch_entries,
            stats.branch_bounds_written,
            stats.leaf_entries
        ),
        (1, 1, 2)
    );
    assert_eq!((stats.child_count_reads, stats.leaf_count_reads), (4, 3));
    // Both entries contribute even though the first carries no enabled bit.
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, op, _)| *op == "resolve")
            .map(|v| v.0)
            .collect::<Vec<_>>(),
        [7, 8]
    );
}

#[test]
fn nested_writes_are_depth_first_then_parent_and_duplicates_resolve_again() {
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node::Branches(vec![(box3(-1., 1.), 1), (box3(-2., 2.), 1)]),
    );
    f.nodes.insert(1, Node::Branches(vec![(box3(-3., 3.), 2)]));
    f.nodes.insert(2, Node::Leaves(vec![(0, 4), (0, 4)]));
    f.placements.insert(4, Some(box3(-4., 5.)));
    let mut accumulator = Bounds::MISSING_ENTITY;
    let stats = refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, op, _)| *op == "write")
            .map(|v| (v.0, v.2))
            .collect::<Vec<_>>(),
        [(1, 0), (0, 0), (1, 0), (0, 1)]
    );
    assert_eq!(
        f.reads.iter().filter(|(_, op, _)| *op == "resolve").count(),
        4
    );
    assert_eq!(stats.node_visits, 5);
    assert_bounds(accumulator, -4., 5.);
}

#[test]
fn missing_entities_leave_accumulator_unchanged_without_fake_zero_bounds() {
    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Leaves(vec![(0, 7), (8, 7)]));
    f.placements.insert(7, None);
    let mut accumulator = unpack(box3(20., 30.));
    let stats = refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_bounds(accumulator, 20., 30.);
    assert_eq!(
        (stats.resolved_placements, stats.unresolved_placements),
        (0, 2)
    );
    assert_eq!(stats.leaf_count_reads, 3);
}

#[test]
fn child_seed_and_transformed_leaf_w_are_positive_zero_but_root_ties_keep_maximum_sign() {
    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Branches(vec![(box3(-1., 1.), 1)]));
    f.nodes.insert(1, Node::Leaves(vec![(0, 7)]));
    f.placements.insert(7, Some(box3(-2., 2.)));
    let mut accumulator = Bounds::MISSING_ENTITY;
    refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_eq!(accumulator.minimum[3].to_bits(), 0);
    assert_eq!(accumulator.maximum[3].to_bits(), (-0f32).to_bits());
}

#[test]
fn completed_branch_is_written_before_parent_union_and_errors_do_not_rollback() {
    let mut f = Fixture::default();
    f.nodes
        .insert(0, Node::Branches(vec![(box3(-f32::MAX, -f32::MAX), 1)]));
    f.nodes.insert(1, Node::Leaves(vec![]));
    let mut accumulator = unpack(box3(f32::MAX, f32::MAX));
    assert!(refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).is_err());
    assert!(f.reads.contains(&(0, "write", 0)));
    assert_bounds(accumulator, f32::MAX, f32::MAX);

    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Leaves(vec![(0, 7), (0, 8)]));
    f.placements.insert(7, Some(box3(-2., 3.)));
    let mut accumulator = unpack(box3(-1., 1.));
    assert!(
        refresh_hierarchy(&mut f, 0, &mut accumulator, limits())
            .unwrap_err()
            .to_string()
            .contains("unavailable placement")
    );
    assert_bounds(accumulator, -2., 3.);

    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Branches(vec![(box3(-1., 1.), 1)]));
    f.nodes.insert(1, Node::Leaves(vec![(0, 7)]));
    f.placements.insert(7, Some(box3(-2., 3.)));
    f.fail_write = Some((0, 0));
    let mut accumulator = unpack(box3(20., 30.));
    assert!(
        refresh_hierarchy(&mut f, 0, &mut accumulator, limits())
            .unwrap_err()
            .to_string()
            .contains("write failed")
    );
    assert_bounds(accumulator, 20., 30.);
    assert_eq!(branch(&f, 0, 0).minimum, [-1.; 3]);
}

#[test]
fn live_loop_counts_reload_and_entry_classification_stays_separate() {
    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Branches(vec![(box3(-1., 1.), 1)]));
    f.nodes.insert(1, Node::Leaves(vec![]));
    // First count chooses the internal path; the loop's zero ends it without
    // changing to a leaf or touching the branch payload.
    f.child_counts.insert(0, [1, 0].into());
    let mut accumulator = unpack(box3(20., 30.));
    let stats = refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_eq!(
        (
            stats.node_visits,
            stats.child_count_reads,
            stats.leaf_count_reads
        ),
        (1, 2, 0)
    );
    assert!(!f.reads.iter().any(|(_, op, _)| *op == "seed"));
    assert_bounds(accumulator, 20., 30.);

    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Leaves(vec![(0, 7), (0, 8)]));
    f.placements.insert(7, Some(box3(1., 2.)));
    f.placements.insert(8, Some(box3(3., 4.)));
    f.leaf_counts.insert(0, [1, 2, 2].into());
    let mut accumulator = Bounds::MISSING_ENTITY;
    let stats = refresh_hierarchy(&mut f, 0, &mut accumulator, limits()).unwrap();
    assert_eq!(stats.leaf_entries, 2);
    assert_eq!(stats.leaf_count_reads, 3);
    assert_bounds(accumulator, 1., 4.);
}

#[test]
fn cycle_and_mutable_count_work_limits_are_errors_never_completed_refreshes() {
    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Branches(vec![(box3(-1., 1.), 0)]));
    let mut accumulator = Bounds::MISSING_ENTITY;
    let small = RefreshLimits {
        node_visits: 3,
        ..limits()
    };
    assert!(
        refresh_hierarchy(&mut f, 0, &mut accumulator, small)
            .unwrap_err()
            .to_string()
            .contains("node-visit")
    );
    assert!(!f.reads.iter().any(|(_, op, _)| *op == "write"));
    assert_eq!(accumulator.minimum, Bounds::MISSING_ENTITY.minimum);

    let mut f = Fixture::default();
    f.nodes.insert(0, Node::Leaves(vec![(0, 7), (0, 8)]));
    f.placements.insert(7, Some(box3(1., 2.)));
    f.placements.insert(8, Some(box3(3., 4.)));
    f.leaf_counts.insert(0, [1, 2].into());
    let mut accumulator = Bounds::MISSING_ENTITY;
    let small = RefreshLimits {
        entry_tests: 1,
        ..limits()
    };
    assert!(
        refresh_hierarchy(&mut f, 0, &mut accumulator, small)
            .unwrap_err()
            .to_string()
            .contains("entry-test")
    );
    assert_bounds(accumulator, 1., 2.);
    assert!(!f.reads.contains(&(0, "placement", 1)));
    assert!(
        refresh_hierarchy(
            &mut f,
            0,
            &mut accumulator,
            RefreshLimits {
                node_visits: 0,
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn aggregate_extrema_agree_with_independent_f64_minimum_and_maximum() {
    let mut f = Fixture::default();
    let mut word = 33u32;
    let mut expected_min = [f64::INFINITY; 3];
    let mut expected_max = [f64::NEG_INFINITY; 3];
    let mut entries = Vec::new();
    for placement in 0..1000u16 {
        let mut low = [0.; 3];
        let mut high = [0.; 3];
        for axis in 0..3 {
            word = word.wrapping_mul(1664525).wrapping_add(1013904223);
            low[axis] = (word >> 16) as f32 / 128. - 300.;
            word = word.wrapping_mul(1664525).wrapping_add(1013904223);
            high[axis] = low[axis] + (word >> 24) as f32 / 64.;
            expected_min[axis] = expected_min[axis].min(f64::from(low[axis]));
            expected_max[axis] = expected_max[axis].max(f64::from(high[axis]));
        }
        f.placements.insert(
            placement,
            Some(StoredBounds {
                minimum: low,
                maximum: high,
            }),
        );
        entries.push((0, placement));
    }
    f.nodes.insert(0, Node::Leaves(entries));
    let mut accumulator = Bounds::MISSING_ENTITY;
    let stats = refresh_hierarchy(
        &mut f,
        0,
        &mut accumulator,
        RefreshLimits {
            node_visits: 1,
            entry_tests: 1000,
        },
    )
    .unwrap();
    assert_eq!(stats.resolved_placements, 1000);
    for axis in 0..3 {
        assert_eq!(f64::from(accumulator.minimum[axis]), expected_min[axis]);
        assert_eq!(f64::from(accumulator.maximum[axis]), expected_max[axis]);
    }
}
