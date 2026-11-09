use super::*;
use anyhow::Context;
use std::collections::{BTreeMap, VecDeque};

struct Node {
    count: u16,
    index: Option<u16>,
    children: Vec<Option<u32>>,
}
#[derive(Default)]
struct Fixture {
    nodes: BTreeMap<u32, Node>,
    spheres: BTreeMap<(u16, u16), [f32; 4]>,
    bounds: BTreeMap<u16, StoredBounds>,
    root_bounds: Option<StoredBounds>,
    counts: BTreeMap<u32, VecDeque<u16>>,
    reads: Vec<(u32, &'static str, u16)>,
}
impl CoarseSource for Fixture {
    type Node = u32;
    fn node_count(&mut self, node: u32) -> Result<u16> {
        self.reads.push((node, "count", 0));
        let fallback = self.nodes.get(&node).context("missing coarse node")?.count;
        Ok(self
            .counts
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(fallback))
    }
    fn pool_index(&mut self, node: u32) -> Result<u16> {
        self.reads.push((node, "index", 0));
        self.nodes
            .get(&node)
            .context("missing coarse node")?
            .index
            .context("unreadable coarse index")
    }
    fn sphere(&mut self, index: u16, slot: u16) -> Result<[f32; 4]> {
        self.reads.push((u32::from(index), "sphere", slot));
        self.spheres
            .get(&(index, slot))
            .copied()
            .context("missing coarse sphere")
    }
    fn child_target(&mut self, node: u32, slot: u16) -> Result<u32> {
        self.reads.push((node, "child", slot));
        self.nodes
            .get(&node)
            .context("missing coarse node")?
            .children
            .get(usize::from(slot))
            .copied()
            .flatten()
            .context("invalid addressed coarse child")
    }
    fn root_bounds(&mut self) -> Result<StoredBounds> {
        self.reads.push((0, "root_bounds", 0));
        self.root_bounds.context("missing root bounds")
    }
    fn leaf_bounds(&mut self, index: u16) -> Result<StoredBounds> {
        self.reads.push((u32::from(index), "leaf_bounds", 0));
        self.bounds
            .get(&index)
            .copied()
            .context("missing leaf bounds")
    }
}
fn limits() -> CoarseLimits {
    CoarseLimits {
        node_reads: 100,
        sphere_tests: 100,
        fanout: 16,
    }
}
fn near() -> StoredBounds {
    StoredBounds {
        minimum: [-1.; 3],
        maximum: [1.; 3],
    }
}
fn far() -> StoredBounds {
    StoredBounds {
        minimum: [10.; 3],
        maximum: [20.; 3],
    }
}
fn internal(f: &mut Fixture, id: u32, index: u16, children: Vec<Option<u32>>) {
    let count = children.len() as u16;
    f.nodes.insert(
        id,
        Node {
            count,
            index: Some(index),
            children,
        },
    );
    for slot in 0..count {
        f.spheres.insert((index, slot), [0., 0., 0., 2.]);
    }
}
fn leaf(f: &mut Fixture, id: u32, index: u16, bounds: StoredBounds) {
    f.nodes.insert(
        id,
        Node {
            count: 0,
            index: Some(index),
            children: vec![],
        },
    );
    f.bounds.insert(index, bounds);
}

#[test]
fn all_spheres_scan_ascending_before_descending_children_and_first_leaf_hit_returns() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None, Some(2), Some(3)]);
    leaf(&mut f, 2, 21, near());
    leaf(&mut f, 3, 22, far());
    let result = query(&mut f, 0, [0.; 4], 0., limits()).unwrap();
    assert!(result.overlaps);
    assert_eq!(
        (
            result.node_reads,
            result.sphere_tests,
            result.leaf_tests,
            result.maximum_fanout
        ),
        (4, 3, 2, 3)
    );
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, kind, _)| *kind == "sphere" || *kind == "child")
            .copied()
            .collect::<Vec<_>>(),
        [
            (10, "sphere", 0),
            (10, "sphere", 1),
            (10, "sphere", 2),
            (0, "child", 2),
            (0, "child", 1)
        ]
    );
    assert!(!f.reads.contains(&(0, "child", 0)));
}
#[test]
fn nested_selected_children_finish_before_parent_resumes_lower_slots() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![Some(2), Some(1)]);
    internal(&mut f, 1, 11, vec![Some(3), Some(4)]);
    leaf(&mut f, 2, 30, near());
    leaf(&mut f, 3, 31, far());
    leaf(&mut f, 4, 32, far());
    let result = query(&mut f, 0, [0.; 4], 0., limits()).unwrap();
    assert!(result.overlaps);
    assert_eq!(
        (result.node_reads, result.sphere_tests, result.leaf_tests),
        (7, 4, 3)
    );
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, kind, _)| *kind == "child")
            .copied()
            .collect::<Vec<_>>(),
        [
            (0, "child", 1),
            (1, "child", 1),
            (1, "child", 0),
            (0, "child", 0)
        ]
    );
}
#[test]
fn root_leaf_ignores_index_and_origin_w_and_containment_keeps_lazy_xyz() {
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node {
            count: 0,
            index: None,
            children: vec![],
        },
    );
    f.root_bounds = Some(near());
    assert!(
        query(&mut f, 0, [0., 0., 0., f32::NAN], 0., limits())
            .unwrap()
            .overlaps
    );
    assert_eq!(f.reads, [(0, "count", 0), (0, "root_bounds", 0)]);
    f.root_bounds = Some(StoredBounds {
        minimum: [2., f32::NAN, 0.],
        maximum: [f32::NAN; 3],
    });
    assert!(
        !query(&mut f, 0, [0., f32::NAN, f32::NAN, f32::NAN], 0., limits())
            .unwrap()
            .overlaps
    );
}
#[test]
fn rejected_spheres_do_not_address_children_and_late_sphere_errors_precede_descent() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None]);
    f.spheres.insert((10, 0), [100., 0., 0., 0.]);
    assert!(!query(&mut f, 0, [0.; 4], 0., limits()).unwrap().overlaps);
    assert!(!f.reads.iter().any(|(_, kind, _)| *kind == "child"));
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None, None]);
    f.spheres.insert((10, 1), [0., 0., 0., f32::NAN]);
    assert!(query(&mut f, 0, [0.; 4], 0., limits()).is_err());
    assert!(!f.reads.iter().any(|(_, kind, _)| *kind == "child"));
}
#[test]
fn internal_counts_reload_after_precheck_without_turning_zero_reload_into_a_leaf() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None]);
    f.counts.insert(0, [1, 0].into());
    let result = query(&mut f, 0, [f32::NAN; 4], f32::NAN, limits()).unwrap();
    assert!(!result.overlaps);
    assert_eq!(f.reads, [(0, "count", 0), (0, "index", 0), (0, "count", 0)]);
    assert_eq!(
        (result.node_reads, result.sphere_tests, result.leaf_tests),
        (2, 0, 0)
    );
}
#[test]
fn sphere_test_keeps_origin_w_grouping_closed_ties_and_negative_effective_radius() {
    assert!(sphere_overlaps([0.5, 0., 0., 1e8], [0., 0., 0., 1e8], 1.).unwrap());
    assert_eq!((1f32 + 1e8) - 1e8, 0.); // Reassociated effective radius incorrectly rejects.
    assert!(sphere_overlaps([1., 0., 0., 0.], [0.; 4], 1.).unwrap());
    assert!(sphere_overlaps([1., 0., 0., 0.], [0., 0., 0., 2.], 0.).unwrap());
    assert!(!sphere_overlaps([1.01, 0., 0., 0.], [0.; 4], 1.).unwrap());
    assert!(sphere_overlaps([f32::MAX, 0., 0., 0.], [0.; 4], 0.).is_err());
}
#[test]
fn shared_nodes_repeat_cycles_exhaust_budgets_and_large_fanout_is_explicitly_unsupported() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![Some(1), Some(1)]);
    leaf(&mut f, 1, 20, far());
    let result = query(&mut f, 0, [0.; 4], 0., limits()).unwrap();
    assert!(!result.overlaps);
    assert_eq!(result.leaf_tests, 2);
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![Some(0)]);
    let small = CoarseLimits {
        node_reads: 5,
        ..limits()
    };
    assert!(
        query(&mut f, 0, [0.; 4], 0., small)
            .unwrap_err()
            .to_string()
            .contains("node-read")
    );
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None, None]);
    let small = CoarseLimits {
        sphere_tests: 1,
        ..limits()
    };
    assert!(
        query(&mut f, 0, [0.; 4], 0., small)
            .unwrap_err()
            .to_string()
            .contains("sphere-test")
    );
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, vec![None; 17]);
    assert!(
        query(&mut f, 0, [0.; 4], 0., limits())
            .unwrap_err()
            .to_string()
            .contains("unsupported coarse fanout")
    );
    assert!(!f.reads.iter().any(|(_, kind, _)| *kind == "sphere"));
    assert!(
        query(
            &mut f,
            0,
            [0.; 4],
            0.,
            CoarseLimits {
                fanout: 0,
                ..limits()
            }
        )
        .is_err()
    );
}
#[test]
fn stable_sphere_decisions_agree_with_independent_f64_geometry() {
    let mut word = 909u32;
    let mut next = || {
        word = word.wrapping_mul(1664525).wrapping_add(1013904223);
        ((word >> 24) as i32 - 128) as f32 / 16.
    };
    let mut stable = 0;
    for _ in 0..1000 {
        let sphere = std::array::from_fn(|_| next());
        let origin = std::array::from_fn(|_| next());
        let radius = next();
        let d = std::array::from_fn::<_, 3, _>(|i| f64::from(sphere[i]) - f64::from(origin[i]));
        let distance = d.into_iter().map(|v| v * v).sum::<f64>();
        let effective = f64::from(radius) + f64::from(sphere[3]) - f64::from(origin[3]);
        let limit = effective * effective;
        if (distance - limit).abs() < 0.0001 * (distance + limit).max(1.) {
            continue;
        }
        assert_eq!(
            sphere_overlaps(sphere, origin, radius).unwrap(),
            distance <= limit
        );
        stable += 1;
    }
    assert!(stable > 950);
}
