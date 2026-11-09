use super::*;
use anyhow::Context;
use std::collections::BTreeMap;

#[derive(Clone)]
enum Contents {
    Poison,
    Branches(Vec<(StoredBounds, Option<u32>)>),
    Leaves(Vec<(u8, Option<u16>)>),
}
#[derive(Clone)]
struct Node {
    flags: u8,
    contents: Contents,
}
#[derive(Default)]
struct Fixture {
    nodes: BTreeMap<u32, Node>,
    reads: Vec<(u32, &'static str, usize)>,
}
impl Fixture {
    fn read(&mut self, node: u32, kind: &'static str, index: usize) -> Result<Node> {
        self.reads.push((node, kind, index));
        self.nodes
            .get(&node)
            .cloned()
            .context("missing addressed hierarchy node")
    }
}
impl CandidateHierarchy for Fixture {
    type Node = u32;
    fn node_flags(&mut self, node: u32) -> Result<u8> {
        Ok(self.read(node, "flags", 0)?.flags)
    }
    fn child_count(&mut self, node: u32) -> Result<u8> {
        match self.read(node, "children", 0)?.contents {
            Contents::Branches(v) => Ok(v.len().try_into()?),
            Contents::Leaves(_) => Ok(0),
            Contents::Poison => anyhow::bail!("unread disabled node count"),
        }
    }
    fn leaf_count(&mut self, node: u32) -> Result<u16> {
        match self.read(node, "leaves", 0)?.contents {
            Contents::Leaves(v) => Ok(v.len().try_into()?),
            _ => anyhow::bail!("not a leaf node"),
        }
    }
    fn branch_bounds(&mut self, node: u32, index: u8) -> Result<StoredBounds> {
        match self.read(node, "bounds", usize::from(index))?.contents {
            Contents::Branches(v) => Ok(v[usize::from(index)].0),
            _ => anyhow::bail!("not an internal node"),
        }
    }
    fn branch_target(&mut self, node: u32, index: u8) -> Result<u32> {
        match self.read(node, "target", usize::from(index))?.contents {
            Contents::Branches(v) => v[usize::from(index)]
                .1
                .context("bad addressed child pointer"),
            _ => anyhow::bail!("not an internal node"),
        }
    }
    fn leaf_flags(&mut self, node: u32, index: u16) -> Result<u8> {
        match self.read(node, "entry_flags", usize::from(index))?.contents {
            Contents::Leaves(v) => Ok(v[usize::from(index)].0),
            _ => anyhow::bail!("not a leaf node"),
        }
    }
    fn leaf_placement(&mut self, node: u32, index: u16) -> Result<u16> {
        match self.read(node, "placement", usize::from(index))?.contents {
            Contents::Leaves(v) => v[usize::from(index)].1.context("bad addressed placement"),
            _ => anyhow::bail!("not a leaf node"),
        }
    }
}
fn limits() -> CandidateLimits {
    CandidateLimits {
        node_visits: 100,
        entry_tests: 100,
        output_indices: 100,
    }
}
fn near() -> StoredBounds {
    StoredBounds {
        minimum: [-1.; 3],
        maximum: [1.; 3],
    }
}
fn leaf(entries: Vec<(u8, Option<u16>)>) -> Node {
    Node {
        flags: 8,
        contents: Contents::Leaves(entries),
    }
}

#[test]
fn traversal_is_depth_first_storage_order_and_retains_repeated_indices() {
    let far = StoredBounds {
        minimum: [10.; 3],
        maximum: [20.; 3],
    };
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node {
            flags: 0xff,
            contents: Contents::Branches(vec![(near(), Some(1)), (near(), Some(2)), (far, None)]),
        },
    );
    f.nodes.insert(
        1,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(3)), (near(), Some(4))]),
        },
    );
    f.nodes.insert(2, leaf(vec![(8, Some(9)), (0, None)]));
    f.nodes.insert(3, leaf(vec![(8, Some(7)), (0xff, Some(7))]));
    f.nodes.insert(4, leaf(vec![(1, None)]));
    let result = collect_candidates(&mut f, 0, [0.; 3], 0., limits()).unwrap();
    assert_eq!(result.placements, [7, 7, 9]);
    assert_eq!(
        (result.nodes_tested, result.branch_tests, result.leaf_tests),
        (6, 5, 5)
    );
    assert!(!f.reads.contains(&(0, "target", 2)));
    assert!(!f.reads.contains(&(2, "placement", 1)));
    assert!(!f.reads.contains(&(4, "placement", 0)));
    let order = f
        .reads
        .iter()
        .filter(|(_, kind, _)| *kind == "flags")
        .map(|v| v.0)
        .collect::<Vec<_>>();
    // Native prechecks internal child one, then checks it again on recursion.
    assert_eq!(order, [0, 1, 1, 3, 4, 2]);
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, kind, _)| *kind == "leaves")
            .count(),
        8
    );
}

#[test]
fn disabled_root_and_empty_leaf_do_not_use_query_values_or_unneeded_fields() {
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node {
            flags: 0xf7,
            contents: Contents::Poison,
        },
    );
    let result = collect_candidates(&mut f, 0, [f32::NAN; 3], f32::NAN, limits()).unwrap();
    assert!(result.placements.is_empty());
    assert_eq!(f.reads, [(0, "flags", 0)]);
    f.nodes.insert(0, leaf(vec![]));
    f.reads.clear();
    assert!(
        collect_candidates(&mut f, 0, [f32::NAN; 3], f32::NAN, limits())
            .unwrap()
            .placements
            .is_empty()
    );
    assert_eq!(
        f.reads,
        [(0, "flags", 0), (0, "children", 0), (0, "leaves", 0)]
    );
}

#[test]
fn disabled_child_reads_no_counts_and_branch_rejection_precedes_pointer_load() {
    let mut f = Fixture::default();
    let rejecting = StoredBounds {
        minimum: [2., f32::NAN, 0.],
        maximum: [f32::NAN; 3],
    };
    f.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(rejecting, None), (near(), Some(1))]),
        },
    );
    f.nodes.insert(
        1,
        Node {
            flags: 0,
            contents: Contents::Poison,
        },
    );
    let result = collect_candidates(&mut f, 0, [0.; 3], 0., limits()).unwrap();
    assert!(result.placements.is_empty());
    assert!(!f.reads.contains(&(0, "target", 0)));
    assert_eq!(
        f.reads
            .iter()
            .filter(|v| v.0 == 1)
            .cloned()
            .collect::<Vec<_>>(),
        [(1, "flags", 0)]
    );
}

#[test]
fn radius_gate_is_closed_and_shared_child_nodes_are_not_deduplicated() {
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(1)), (near(), Some(1))]),
        },
    );
    f.nodes.insert(1, leaf(vec![(8, Some(u16::MAX))]));
    assert_eq!(
        collect_candidates(&mut f, 0, [1.5, 0., 0.], 0.5, limits())
            .unwrap()
            .placements,
        [65535, 65535]
    );
    assert!(
        collect_candidates(&mut f, 0, [1.50001, 0., 0.], 0.5, limits())
            .unwrap()
            .placements
            .is_empty()
    );
}

#[test]
fn cycles_and_entry_or_output_limits_are_errors_never_partial_success() {
    let mut f = Fixture::default();
    f.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(0))]),
        },
    );
    let small = CandidateLimits {
        node_visits: 3,
        ..limits()
    };
    assert!(
        collect_candidates(&mut f, 0, [0.; 3], 0., small)
            .unwrap_err()
            .to_string()
            .contains("node-visit")
    );
    assert_eq!(
        f.reads
            .iter()
            .filter(|(_, kind, _)| *kind == "flags")
            .count(),
        3
    );
    let small = CandidateLimits {
        entry_tests: 2,
        ..limits()
    };
    assert!(
        collect_candidates(&mut f, 0, [0.; 3], 0., small)
            .unwrap_err()
            .to_string()
            .contains("entry-test")
    );
    f.nodes.insert(0, leaf(vec![(8, Some(1)), (8, Some(2))]));
    let small = CandidateLimits {
        output_indices: 1,
        ..limits()
    };
    assert!(
        collect_candidates(&mut f, 0, [0.; 3], 0., small)
            .unwrap_err()
            .to_string()
            .contains("output-index")
    );
    for bad in [0, 1_000_001] {
        assert!(
            collect_candidates(
                &mut f,
                0,
                [0.; 3],
                0.,
                CandidateLimits {
                    node_visits: bad,
                    ..limits()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn addressed_errors_propagate_and_leaf_origin_is_not_a_hidden_distance_filter() {
    let mut f = Fixture::default();
    f.nodes.insert(0, leaf(vec![(8, Some(123))]));
    assert_eq!(
        collect_candidates(&mut f, 0, [f32::NAN; 3], f32::NAN, limits())
            .unwrap()
            .placements,
        [123]
    );
    f.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), None)]),
        },
    );
    assert!(
        collect_candidates(&mut f, 0, [0.; 3], 0., limits())
            .unwrap_err()
            .to_string()
            .contains("bad addressed child")
    );
    assert!(collect_candidates(&mut f, 999, [0.; 3], 0., limits()).is_err());
}

#[derive(Default)]
struct Mutable {
    fixture: Fixture,
    flags: BTreeMap<u32, std::collections::VecDeque<u8>>,
    children: BTreeMap<u32, std::collections::VecDeque<u8>>,
    leaves: BTreeMap<u32, std::collections::VecDeque<u16>>,
}
impl CandidateHierarchy for Mutable {
    type Node = u32;
    fn node_flags(&mut self, node: u32) -> Result<u8> {
        let fallback = self.fixture.node_flags(node)?;
        Ok(self
            .flags
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(fallback))
    }
    fn child_count(&mut self, node: u32) -> Result<u8> {
        let fallback = self.fixture.child_count(node)?;
        Ok(self
            .children
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(fallback))
    }
    fn leaf_count(&mut self, node: u32) -> Result<u16> {
        let fallback = self.fixture.leaf_count(node)?;
        Ok(self
            .leaves
            .get_mut(&node)
            .and_then(|v| v.pop_front())
            .unwrap_or(fallback))
    }
    fn branch_bounds(&mut self, node: u32, index: u8) -> Result<StoredBounds> {
        self.fixture.branch_bounds(node, index)
    }
    fn branch_target(&mut self, node: u32, index: u8) -> Result<u32> {
        self.fixture.branch_target(node, index)
    }
    fn leaf_flags(&mut self, node: u32, index: u16) -> Result<u8> {
        self.fixture.leaf_flags(node, index)
    }
    fn leaf_placement(&mut self, node: u32, index: u16) -> Result<u16> {
        self.fixture.leaf_placement(node, index)
    }
}

#[test]
fn recursive_internal_child_rechecks_enabled_flag_before_reading_payload() {
    let mut source = Mutable::default();
    source.fixture.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(1))]),
        },
    );
    source.fixture.nodes.insert(
        1,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), None)]),
        },
    );
    source.flags.insert(1, [8, 0].into());
    let result = collect_candidates(&mut source, 0, [0.; 3], 0., limits()).unwrap();
    assert!(result.placements.is_empty());
    assert_eq!(result.nodes_tested, 3);
    assert_eq!(
        source
            .fixture
            .reads
            .iter()
            .filter(|&&(node, kind, _)| node == 1 && kind == "flags")
            .count(),
        2
    );
    assert_eq!(
        source
            .fixture
            .reads
            .iter()
            .filter(|&&(node, kind, _)| node == 1 && kind == "children")
            .count(),
        1
    );
    assert!(
        !source
            .fixture
            .reads
            .iter()
            .any(|&(node, kind, _)| node == 1 && kind == "bounds")
    );
}

#[test]
fn recursive_child_count_is_reloaded_and_can_select_leaf_after_parent_saw_internal() {
    let mut source = Mutable::default();
    source.fixture.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(1))]),
        },
    );
    source.fixture.nodes.insert(1, leaf(vec![(8, Some(55))]));
    source.children.insert(1, [1, 0].into());
    let result = collect_candidates(&mut source, 0, [0.; 3], 0., limits()).unwrap();
    assert_eq!(result.placements, [55]);
    assert_eq!(
        source
            .fixture
            .reads
            .iter()
            .filter(|&&(node, kind, _)| node == 1 && kind == "children")
            .count(),
        2
    );
    assert_eq!(
        source
            .fixture
            .reads
            .iter()
            .filter(|&&(node, kind, _)| node == 1 && kind == "leaves")
            .count(),
        2
    );
}

#[test]
fn leaf_count_is_reloaded_after_each_entry_and_can_shrink_or_grow() {
    for (counts, expected) in [(vec![3, 1], vec![11]), (vec![1, 2, 2], vec![11, 22])] {
        let mut source = Mutable::default();
        source
            .fixture
            .nodes
            .insert(0, leaf(vec![(8, Some(11)), (8, Some(22)), (8, Some(33))]));
        source.leaves.insert(0, counts.into());
        let result = collect_candidates(&mut source, 0, [0.; 3], 0., limits()).unwrap();
        assert_eq!(result.placements, expected);
        assert_eq!(
            source
                .fixture
                .reads
                .iter()
                .filter(|(_, kind, _)| *kind == "leaves")
                .count(),
            expected.len() + 1
        );
    }
}

#[test]
fn budgets_bound_growing_leaf_counts_and_reloaded_internal_child_counts() {
    let mut source = Mutable::default();
    source
        .fixture
        .nodes
        .insert(0, leaf(vec![(8, Some(1)), (8, Some(2))]));
    source.leaves.insert(0, [1, 2].into());
    let small = CandidateLimits {
        entry_tests: 1,
        ..limits()
    };
    assert!(
        collect_candidates(&mut source, 0, [0.; 3], 0., small)
            .unwrap_err()
            .to_string()
            .contains("entry-test")
    );
    assert_eq!(
        source
            .fixture
            .reads
            .iter()
            .filter(|(_, kind, _)| *kind == "leaves")
            .count(),
        2
    );
    assert!(!source.fixture.reads.contains(&(0, "entry_flags", 1)));

    let mut source = Mutable::default();
    source.fixture.nodes.insert(
        0,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(1))]),
        },
    );
    source.fixture.nodes.insert(
        1,
        Node {
            flags: 8,
            contents: Contents::Branches(vec![(near(), Some(2)), (near(), Some(2))]),
        },
    );
    source.fixture.nodes.insert(2, leaf(vec![(8, Some(5))]));
    source.children.insert(1, [1, 2].into());
    let small = CandidateLimits {
        node_visits: 3,
        ..limits()
    };
    assert!(
        collect_candidates(&mut source, 0, [0.; 3], 0., small)
            .unwrap_err()
            .to_string()
            .contains("node-visit")
    );
    assert!(!source.fixture.reads.iter().any(|&(node, _, _)| node == 2));
    source.children.insert(1, [1, 2].into());
    assert_eq!(
        collect_candidates(&mut source, 0, [0.; 3], 0., limits())
            .unwrap()
            .placements,
        [5, 5]
    );
}
