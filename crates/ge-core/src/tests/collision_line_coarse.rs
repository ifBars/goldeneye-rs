use super::super::world::StoredBounds;
use super::*;
use anyhow::Context;
use std::collections::{BTreeMap, VecDeque};

#[derive(Default)]
struct Fixture {
    counts: BTreeMap<u32, VecDeque<u16>>,
    indices: BTreeMap<u32, u16>,
    spheres: BTreeMap<(u16, u16), [f32; 4]>,
    children: BTreeMap<(u32, u16), u32>,
    bounds: BTreeMap<u16, StoredBounds>,
    root_bounds: Option<StoredBounds>,
    reads: Vec<(u32, &'static str, u16)>,
}
impl CoarseSource for Fixture {
    type Node = u32;
    fn node_count(&mut self, node: u32) -> Result<u16> {
        self.reads.push((node, "count", 0));
        let counts = self
            .counts
            .get_mut(&node)
            .context("unreadable node count")?;
        let value = *counts.front().context("empty node count sequence")?;
        if counts.len() > 1 {
            counts.pop_front();
        }
        Ok(value)
    }
    fn pool_index(&mut self, node: u32) -> Result<u16> {
        self.reads.push((node, "index", 0));
        self.indices
            .get(&node)
            .copied()
            .context("unreadable node index")
    }
    fn sphere(&mut self, index: u16, slot: u16) -> Result<[f32; 4]> {
        self.reads.push((u32::from(index), "sphere", slot));
        self.spheres
            .get(&(index, slot))
            .copied()
            .context("unreadable sphere")
    }
    fn child_target(&mut self, node: u32, slot: u16) -> Result<u32> {
        self.reads.push((node, "child", slot));
        self.children
            .get(&(node, slot))
            .copied()
            .context("unreadable child")
    }
    fn root_bounds(&mut self) -> Result<StoredBounds> {
        self.reads.push((0, "root_bounds", 0));
        self.root_bounds.context("unreadable root bounds")
    }
    fn leaf_bounds(&mut self, index: u16) -> Result<StoredBounds> {
        self.reads.push((u32::from(index), "bounds", 0));
        self.bounds
            .get(&index)
            .copied()
            .context("unreadable leaf bounds")
    }
}
fn limits() -> CoarseLimits {
    CoarseLimits {
        node_reads: 100,
        sphere_tests: 100,
        fanout: 16,
    }
}
fn hit_bounds() -> StoredBounds {
    StoredBounds {
        minimum: [4., -1., -1.],
        maximum: [6., 1., 1.],
    }
}
fn miss_bounds() -> StoredBounds {
    StoredBounds {
        minimum: [4., 4., 4.],
        maximum: [6., 6., 6.],
    }
}
fn internal(f: &mut Fixture, node: u32, index: u16, children: &[u32]) {
    f.counts
        .insert(node, VecDeque::from([children.len() as u16]));
    f.indices.insert(node, index);
    for (slot, child) in children.iter().enumerate() {
        let slot = slot as u16;
        f.children.insert((node, slot), *child);
        f.spheres.insert((index, slot), [5., 0., 0., 1.]);
    }
}
fn leaf(f: &mut Fixture, node: u32, index: u16, bounds: StoredBounds) {
    f.counts.insert(node, VecDeque::from([0]));
    f.indices.insert(node, index);
    f.bounds.insert(index, bounds);
}

#[test]
fn point_distance_clamps_to_both_endpoints_and_handles_zero_segment() {
    let origin = [0.; 3];
    let delta = [10., 0., 0.];
    for (point, expected) in [
        ([-2., 3., 0.], 13.),
        ([12., 3., 0.], 13.),
        ([5., 3., 0.], 9.),
    ] {
        assert_eq!(
            point_segment_squared_distance(point, origin, delta).unwrap(),
            expected
        );
    }
    assert_eq!(
        point_segment_squared_distance([2., 3., 4.], origin, origin).unwrap(),
        29.
    );
}

#[test]
fn interior_projection_keeps_product_rounding_before_residual_subtraction() {
    let point = [0x409d02b7, 0x41054154, 0x404671b2].map(f32::from_bits);
    let origin = [0xbef5ed49, 0x4100d0b3, 0xc03dd5dc].map(f32::from_bits);
    let delta = [0xbf4452fb, 0xc018bb9f, 0x41079d57].map(f32::from_bits);
    assert_eq!(
        point_segment_squared_distance(point, origin, delta)
            .unwrap()
            .to_bits(),
        0x421849c9
    );
    // Fusing the final residual instead yields0x421849cb for these authored
    // inputs. It is a different operation from the stored projection products.
}

#[test]
fn sphere_tangency_is_strict_and_negative_radius_is_squared_without_clamp() {
    assert!(!sphere_overlaps([5., 1., 0., 1.], [0.; 3], [10., 0., 0.]).unwrap());
    assert!(sphere_overlaps([5., 0.5, 0., -1.], [0.; 3], [10., 0., 0.]).unwrap());
    assert!(!sphere_overlaps([5., 0., 0., 0.], [0.; 3], [10., 0., 0.]).unwrap());
    // An infinite line would pass through this sphere; the closed segment does not.
    assert!(!sphere_overlaps([12., 0., 0., 1.], [0.; 3], [10., 0., 0.]).unwrap());
}

#[test]
fn root_leaf_uses_pool_base_without_reading_stored_index_and_tests_full_segment() {
    let mut f = Fixture::default();
    f.counts.insert(0, VecDeque::from([0]));
    f.root_bounds = Some(hit_bounds());
    let result = query(&mut f, 0, [0.; 3], [10., 0., 0.], limits()).unwrap();
    assert!(result.overlaps);
    assert_eq!(
        (result.node_reads, result.sphere_tests, result.leaf_tests),
        (1, 0, 1)
    );
    assert_eq!(f.reads, [(0, "count", 0), (0, "root_bounds", 0)]);
}

#[test]
fn ascending_sphere_scan_precedes_descending_child_visits_and_first_leaf_hit_stops() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, &[1, 2, 3]);
    leaf(&mut f, 2, 20, hit_bounds());
    leaf(&mut f, 3, 30, miss_bounds());
    // Child0 is deliberately unreadable: child1's hit must stop before it.
    let result = query(&mut f, 0, [0.; 3], [10., 0., 0.], limits()).unwrap();
    assert!(result.overlaps);
    assert_eq!(
        (result.node_reads, result.sphere_tests, result.leaf_tests),
        (4, 3, 2)
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
            (0, "child", 1),
        ]
    );
}

#[test]
fn tangent_spheres_leave_even_invalid_child_pointers_unread() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, &[1]);
    f.spheres.insert((10, 0), [5., 1., 0., 1.]);
    f.children.clear();
    assert!(
        !query(&mut f, 0, [0.; 3], [10., 0., 0.], limits())
            .unwrap()
            .overlaps
    );
    assert!(!f.reads.iter().any(|(_, kind, _)| *kind == "child"));
}

#[test]
fn nested_internal_count_is_reread_after_index_before_scan_then_parent_resumes() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, &[2, 1]);
    // Initially internal, then zero at scan: no bounds or child reads for node1.
    f.counts.insert(1, VecDeque::from([1, 0]));
    f.indices.insert(1, 11);
    leaf(&mut f, 2, 20, hit_bounds());
    assert!(
        query(&mut f, 0, [0.; 3], [10., 0., 0.], limits())
            .unwrap()
            .overlaps
    );
    let reads = f
        .reads
        .iter()
        .filter(|(node, _, _)| *node == 1)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(reads, [(1, "count", 0), (1, "index", 0), (1, "count", 0)]);
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, kind, _)| *kind == "child")
            .copied()
            .collect::<Vec<_>>(),
        [(0, "child", 1), (0, "child", 0)]
    );
}

#[test]
fn cycles_and_sphere_budget_exhaustion_error_instead_of_returning_clear() {
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, &[0]);
    let mut small = limits();
    small.node_reads = 4;
    assert!(
        query(&mut f, 0, [0.; 3], [10., 0., 0.], small)
            .unwrap_err()
            .to_string()
            .contains("node-read budget")
    );
    let mut f = Fixture::default();
    internal(&mut f, 0, 10, &[1, 2]);
    small = limits();
    small.sphere_tests = 1;
    assert!(
        query(&mut f, 0, [0.; 3], [10., 0., 0.], small)
            .unwrap_err()
            .to_string()
            .contains("sphere-test budget")
    );
    assert!(!f.reads.iter().any(|(_, kind, _)| *kind == "child"));
}

#[test]
fn wide_native_masks_and_nonfinite_or_overflowing_arithmetic_are_explicit_errors() {
    let mut f = Fixture::default();
    f.counts.insert(0, VecDeque::from([17]));
    f.indices.insert(0, 10);
    assert!(
        query(&mut f, 0, [0.; 3], [10., 0., 0.], limits())
            .unwrap_err()
            .to_string()
            .contains("unsupported line coarse fanout")
    );
    assert!(point_segment_squared_distance([f32::NAN, 0., 0.], [0.; 3], [1.; 3]).is_err());
    assert!(point_segment_squared_distance([f32::MAX; 3], [-f32::MAX; 3], [0.; 3]).is_err());
    assert!(sphere_overlaps([0., 0., 0., f32::MAX], [0.; 3], [0.; 3]).is_err());
}
