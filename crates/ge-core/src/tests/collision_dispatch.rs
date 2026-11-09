use super::*;
use anyhow::Context;
use std::collections::{BTreeMap, VecDeque};

const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
#[derive(Clone)]
struct TestNode {
    kind: u32,
    sphere: Option<LocalSphere>,
    bounds: Option<Bounds>,
    children: Vec<Node>,
    links: Vec<Option<Node>>,
    context_index: Option<u16>,
}
fn node(kind: u32) -> TestNode {
    TestNode {
        kind,
        sphere: Some(LocalSphere {
            center: [0.; 3],
            radius: 10.,
        }),
        bounds: Some(Bounds {
            minimum: [-10., -10., -10., 0.],
            maximum: [10., 10., 10., 0.],
        }),
        children: Vec::new(),
        links: Vec::new(),
        context_index: Some(0),
    }
}
struct Source {
    nodes: BTreeMap<Node, TestNode>,
    events: Vec<String>,
    split_counts: BTreeMap<Node, VecDeque<u32>>,
    link_counts: BTreeMap<Node, VecDeque<u8>>,
}
impl Source {
    fn new(nodes: impl IntoIterator<Item = (Node, TestNode)>) -> Self {
        Self {
            nodes: nodes.into_iter().collect(),
            events: Vec::new(),
            split_counts: BTreeMap::new(),
            link_counts: BTreeMap::new(),
        }
    }
    fn get(&mut self, node: Node, label: &str) -> Result<&TestNode> {
        self.events.push(format!("{label}:{node}"));
        self.nodes.get(&node).context("missing addressed node")
    }
}
impl GeometrySource for Source {
    fn node_type(&mut self, node: Node) -> Result<u32> {
        Ok(self.get(node, "type")?.kind)
    }
    fn sphere(&mut self, node: Node) -> Result<LocalSphere> {
        self.get(node, "sphere")?
            .sphere
            .context("unavailable sphere")
    }
    fn bounds(&mut self, node: Node) -> Result<Bounds> {
        self.get(node, "bounds")?
            .bounds
            .context("unavailable bounds")
    }
    fn split_child_count(&mut self, node: Node) -> Result<u32> {
        let fallback = self.get(node, "split_count")?.children.len() as u32;
        Ok(self
            .split_counts
            .get_mut(&node)
            .and_then(VecDeque::pop_front)
            .unwrap_or(fallback))
    }
    fn split_child(&mut self, node: Node, index: u32) -> Result<Node> {
        self.events.push(format!("split_child:{node}:{index}"));
        self.nodes
            .get(&node)
            .context("missing split")?
            .children
            .get(index as usize)
            .copied()
            .context("missing child")
    }
    fn link_count(&mut self, node: Node) -> Result<u8> {
        let fallback = self.get(node, "link_count")?.links.len() as u8;
        Ok(self
            .link_counts
            .get_mut(&node)
            .and_then(VecDeque::pop_front)
            .unwrap_or(fallback))
    }
    fn link_target(&mut self, node: Node, index: u8) -> Result<Option<Node>> {
        self.events.push(format!("link_target:{node}:{index}"));
        self.nodes
            .get(&node)
            .context("missing link")?
            .links
            .get(index as usize)
            .copied()
            .context("missing reference")
    }
    fn optional_context_index(&mut self, node: Node) -> Result<u16> {
        self.get(node, "context_index")?
            .context_index
            .context("unavailable optional-context index")
    }
}
fn request() -> DispatchRequest {
    DispatchRequest {
        query: QuerySphere {
            origin: [0.; 3],
            radius: 0.,
        },
        context: CollectionContext::map_placement(IDENTITY, 9),
        optional_context_key: 0,
        traversal_flags: 0,
        collection_flags: 0,
    }
}
fn run(
    source: &mut Source,
    root: Node,
    request: &DispatchRequest,
    records: &mut RecordBuffer,
) -> Result<DispatchOutcome> {
    collect(source, root, request, records, DispatchLimits::default())
}

#[test]
fn sphere_pruning_fuses_z_after_rounded_xy_and_keeps_unscaled_signed_radius() {
    let z = 1. + f32::EPSILON;
    let local = LocalSphere {
        center: [2f32.powi(-12), 0., z],
        radius: z,
    };
    let probe = probe_sphere(&IDENTITY, local, request().query).unwrap();
    assert_eq!(probe.distance_squared.to_bits(), 1f32.to_bits() + 3);
    assert_eq!(probe.radius_squared.to_bits(), 1f32.to_bits() + 2);
    assert!(!probe.passes);
    let separately_rounded = local.center[0] * local.center[0] + z * z;
    assert_eq!(separately_rounded.to_bits(), 1f32.to_bits() + 2);
    let mut scaled = IDENTITY;
    scaled[0] = 2.;
    assert!(
        !probe_sphere(
            &scaled,
            LocalSphere {
                center: [0.; 3],
                radius: 1.
            },
            QuerySphere {
                origin: [1.5, 0., 0.],
                radius: 0.
            }
        )
        .unwrap()
        .passes
    );
    assert!(
        probe_sphere(
            &IDENTITY,
            LocalSphere {
                center: [1., 0., 0.],
                radius: -1.
            },
            request().query
        )
        .unwrap()
        .passes
    );
}

#[test]
fn sphere_center_uses_point_order_and_translation_last_with_no_perspective_divide() {
    let mut matrix = IDENTITY;
    matrix[4] = 1.;
    matrix[12] = 1e8;
    for lane in [3, 7, 11, 15] {
        matrix[lane] = f32::NAN;
    }
    let mut query = request().query;
    query.origin[1] = 1.; // Isolate the X accumulation; stored center Y is one.
    let probe = probe_sphere(
        &matrix,
        LocalSphere {
            center: [-1e8, 1., 0.],
            radius: 0.,
        },
        query,
    )
    .unwrap();
    assert_eq!(probe.distance_squared, 0.);
    assert!(probe.passes);
    // Bounds accumulation starts with translation and would produce X=1 here.
    assert_eq!((1e8f32 + -1e8) + 1., 1.);
}

#[test]
fn sphere_inputs_and_used_overflow_are_explicit_errors() {
    let local = LocalSphere {
        center: [0.; 3],
        radius: 1.,
    };
    let mut query = request().query;
    query.origin[0] = f32::NAN;
    assert!(probe_sphere(&IDENTITY, local, query).is_err());
    let mut matrix = IDENTITY;
    matrix[0] = f32::NAN;
    assert!(probe_sphere(&matrix, local, request().query).is_err());
    assert!(
        probe_sphere(
            &IDENTITY,
            LocalSphere {
                center: [f32::MAX, 0., 0.],
                radius: 1.
            },
            request().query
        )
        .is_err()
    );
    assert!(
        probe_sphere(
            &IDENTITY,
            LocalSphere {
                radius: f32::MAX,
                ..local
            },
            QuerySphere {
                radius: f32::MAX,
                ..request().query
            }
        )
        .is_err()
    );
}

#[test]
fn leaf_rejections_and_flag_bypasses_preserve_read_order() {
    let mut leaf = node(0x601);
    leaf.sphere = Some(LocalSphere {
        center: [100., 0., 0.],
        radius: 1.,
    });
    leaf.bounds = None;
    let mut source = Source::new([(1, leaf)]);
    let mut records = RecordBuffer::new(2).unwrap();
    let result = run(&mut source, 1, &request(), &mut records).unwrap();
    assert!(!result.returned);
    assert_eq!(result.sphere_rejections, 1);
    assert_eq!(source.events, ["type:1", "sphere:1"]);
    let mut leaf = node(0x601);
    leaf.sphere = None;
    leaf.bounds = Some(Bounds {
        minimum: [100., 100., 100., 0.],
        maximum: [101., 101., 101., 0.],
    });
    let mut source = Source::new([(1, leaf)]);
    let mut req = request();
    req.collection_flags = 2;
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!((result.sphere_tests, result.bounds_rejections), (0, 1));
    assert_eq!(source.events, ["type:1", "bounds:1"]);
    let mut leaf = node(0x601);
    leaf.sphere = None;
    leaf.bounds = None;
    let mut source = Source::new([(1, leaf)]);
    req.traversal_flags = 1;
    req.query.origin = [f32::NAN; 3];
    req.context.matrix[0] = f32::NAN;
    req.collection_flags = 0x1234fffe;
    req.optional_context_key = 77;
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert!(result.returned);
    assert_eq!(source.events, ["type:1"]);
    assert_eq!(records.records()[0].flags_raw, 0xfffe);
    assert_eq!(records.records()[0].optional_context_key, 77);
    assert_eq!(records.records()[0].slot, 9);
    assert!(records.records()[0].matrix[0].is_nan());
}

#[test]
fn full_leaf_still_performs_native_pruning_before_rejecting_append() {
    let mut source = Source::new([(1, node(0x601))]);
    let mut records = RecordBuffer::new(0).unwrap();
    let result = run(&mut source, 1, &request(), &mut records).unwrap();
    assert_eq!(
        (
            result.sphere_tests,
            result.bounds_tests,
            result.full_rejections
        ),
        (1, 1, 1)
    );
    assert!(!result.returned);
    assert!(records.records().is_empty());
    assert_eq!(source.events, ["type:1", "sphere:1", "bounds:1"]);
}

#[test]
fn split_spheres_ignore_collection_flag_two_and_prune_before_other_child_fields() {
    let mut split = node(0x603);
    split.children = vec![2];
    split.sphere = Some(LocalSphere {
        center: [100., 0., 0.],
        radius: 1.,
    });
    let mut source = Source::new([(1, split.clone()), (2, node(0x601))]);
    let mut records = RecordBuffer::new(2).unwrap();
    let mut req = request();
    req.collection_flags = 2;
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!(result.sphere_rejections, 1);
    assert_eq!(source.events, ["type:1", "sphere:1"]);
    split.sphere = node(0x603).sphere;
    let mut child = node(0x601);
    child.sphere = Some(LocalSphere {
        center: [100., 0., 0.],
        radius: 1.,
    });
    child.context_index = None;
    let mut source = Source::new([(1, split), (2, child)]);
    req.optional_context_key = 100;
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!((result.sphere_tests, result.sphere_rejections), (2, 1));
    assert_eq!(
        source.events,
        [
            "type:1",
            "sphere:1",
            "split_count:1",
            "split_child:1:0",
            "sphere:2",
            "split_count:1"
        ]
    );
}

#[test]
fn split_preserves_duplicate_order_remaps_context_and_continues_after_full() {
    let mut split = node(0x603);
    split.children = vec![2, 3, 2];
    let mut a = node(0x601);
    a.context_index = Some(2);
    a.bounds = None;
    let mut b = node(0x601);
    b.context_index = Some(3);
    b.bounds = None;
    let mut source = Source::new([(1, split.clone()), (2, a.clone()), (3, b.clone())]);
    let mut req = request();
    req.optional_context_key = 1000;
    req.collection_flags = 0x12340002;
    let mut records = RecordBuffer::new(3).unwrap();
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!(
        (result.appended, result.sphere_tests, result.bounds_tests),
        (3, 4, 0)
    );
    assert!(result.returned);
    assert_eq!(
        records
            .records()
            .iter()
            .map(|r| (r.geometry_key, r.optional_context_key, r.flags_raw, r.slot))
            .collect::<Vec<_>>(),
        [(2, 1056, 2, 9), (3, 1084, 2, 9), (2, 1056, 2, 9)]
    );
    let mut source = Source::new([(1, split), (2, a), (3, b)]);
    let mut records = RecordBuffer::new(1).unwrap();
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert!(result.returned);
    assert_eq!(
        (
            result.appended,
            result.full_rejections,
            result.nodes_dispatched
        ),
        (1, 2, 4)
    );
    assert_eq!(
        source
            .events
            .iter()
            .filter(|event| event.starts_with("split_child:"))
            .count(),
        3
    );
}

#[test]
fn nested_split_skips_its_coarse_gate_but_still_tests_its_children() {
    let mut root = node(0x603);
    root.children = vec![2];
    let mut middle = node(0x603);
    middle.children = vec![3];
    let mut leaf = node(0x601);
    leaf.bounds = None;
    let mut source = Source::new([(1, root), (2, middle), (3, leaf)]);
    let mut records = RecordBuffer::new(2).unwrap();
    let result = run(&mut source, 1, &request(), &mut records).unwrap();
    assert_eq!((result.appended, result.sphere_tests), (1, 3));
    assert_eq!(
        source
            .events
            .iter()
            .filter(|event| event.as_str() == "sphere:2")
            .count(),
        1
    );
    assert!(
        !source
            .events
            .iter()
            .any(|event| event.starts_with("bounds:"))
    );
}

#[test]
fn link_targets_keep_misses_and_duplicate_calls_without_adding_split_flags() {
    let mut root = node(0x608);
    root.links = vec![None, Some(2), Some(2)];
    root.sphere = None;
    root.bounds = None;
    let mut leaf = node(0x601);
    leaf.sphere = None;
    leaf.context_index = Some(3);
    let mut source = Source::new([(1, root), (2, leaf)]);
    let mut records = RecordBuffer::new(3).unwrap();
    let mut req = request();
    req.collection_flags = 2;
    req.optional_context_key = 100;
    let result = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!(
        (
            result.appended,
            result.link_misses,
            result.bounds_tests,
            result.sphere_tests
        ),
        (2, 1, 2, 0)
    );
    assert_eq!(
        records
            .records()
            .iter()
            .map(|r| (r.geometry_key, r.optional_context_key))
            .collect::<Vec<_>>(),
        [(2, 184), (2, 184)]
    );
    assert_eq!(
        source
            .events
            .iter()
            .filter(|event| event.starts_with("link_target:"))
            .count(),
        3
    );
}

#[test]
fn zero_optional_context_never_reads_indices_and_nonzero_remapping_wraps() {
    let mut root = node(0x608);
    root.links = vec![Some(2)];
    let mut child = node(0x601);
    child.context_index = None;
    child.sphere = None;
    child.bounds = None;
    let mut source = Source::new([(1, root.clone()), (2, child.clone())]);
    let mut records = RecordBuffer::new(2).unwrap();
    let mut req = request();
    req.traversal_flags = 3;
    assert!(run(&mut source, 1, &req, &mut records).unwrap().returned);
    assert!(
        !source
            .events
            .iter()
            .any(|event| event.starts_with("context_index:"))
    );
    child.context_index = Some(2);
    let mut source = Source::new([(1, root), (2, child)]);
    req.optional_context_key = u32::MAX - 10;
    assert!(run(&mut source, 1, &req, &mut records).unwrap().returned);
    assert_eq!(records.records()[1].optional_context_key, 45);
}

#[test]
fn cycles_and_work_limits_are_errors_and_unsupported_nodes_never_become_meshes() {
    let mut cycle = node(0x603);
    cycle.children = vec![1];
    let mut source = Source::new([(1, cycle.clone())]);
    let mut records = RecordBuffer::new(4).unwrap();
    let mut req = request();
    req.traversal_flags = 1;
    assert!(
        collect(
            &mut source,
            1,
            &req,
            &mut records,
            DispatchLimits {
                work_budget: 100,
                depth_budget: 2
            }
        )
        .unwrap_err()
        .to_string()
        .contains("depth budget")
    );
    let mut source = Source::new([(1, cycle)]);
    assert!(
        collect(
            &mut source,
            1,
            &req,
            &mut records,
            DispatchLimits {
                work_budget: 2,
                depth_budget: 128
            }
        )
        .unwrap_err()
        .to_string()
        .contains("work budget")
    );
    let mut root = node(0x608);
    root.links = vec![Some(2), Some(3), Some(2)];
    let mut source = Source::new([(1, root), (2, node(0x601)), (3, node(0x607))]);
    assert!(
        run(&mut source, 1, &req, &mut records)
            .unwrap_err()
            .to_string()
            .contains("0x607")
    );
    assert_eq!(records.records().len(), 1);
    assert!(!source.events.iter().any(|event| event == "link_target:1:2"));
    assert!(
        collect(
            &mut source,
            1,
            &req,
            &mut records,
            DispatchLimits {
                work_budget: 0,
                depth_budget: 128
            }
        )
        .is_err()
    );
}

#[test]
fn mutable_counts_are_reloaded_after_every_child_and_at_termination() {
    let mut split = node(0x603);
    split.children = vec![2, 3];
    let mut source = Source::new([(1, split.clone()), (2, node(0x601)), (3, node(0x601))]);
    source.split_counts.insert(1, VecDeque::from([1, 2, 2]));
    let mut req = request();
    req.traversal_flags = 3;
    let mut records = RecordBuffer::new(8).unwrap();
    assert_eq!(run(&mut source, 1, &req, &mut records).unwrap().appended, 2);
    assert_eq!(
        source
            .events
            .iter()
            .filter(|event| event.as_str() == "split_count:1")
            .count(),
        3
    );
    let mut link = node(0x608);
    link.links = vec![Some(2), Some(3)];
    let mut source = Source::new([(1, link), (2, node(0x601)), (3, node(0x601))]);
    source.link_counts.insert(1, VecDeque::from([1, 2, 2]));
    assert_eq!(run(&mut source, 1, &req, &mut records).unwrap().appended, 2);
    assert_eq!(
        source
            .events
            .iter()
            .filter(|event| event.as_str() == "link_count:1")
            .count(),
        3
    );
    // A rejected child still returns to a fresh count check before another address.
    let mut far = node(0x601);
    far.sphere = Some(LocalSphere {
        center: [100., 0., 0.],
        radius: 1.,
    });
    let mut source = Source::new([(1, split), (2, far), (3, node(0x601))]);
    source.split_counts.insert(1, VecDeque::from([2, 1]));
    req.traversal_flags = 0;
    let outcome = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!((outcome.appended, outcome.sphere_rejections), (0, 1));
    assert!(!source.events.iter().any(|event| event == "split_child:1:1"));
    let mut link = node(0x608);
    link.links = vec![None, Some(2)];
    let mut source = Source::new([(1, link), (2, node(0x601))]);
    source.link_counts.insert(1, VecDeque::from([2, 1]));
    let outcome = run(&mut source, 1, &req, &mut records).unwrap();
    assert_eq!((outcome.appended, outcome.link_misses), (0, 1));
    assert_eq!(source.events.last().unwrap(), "link_count:1");
    assert!(!source.events.iter().any(|event| event == "link_target:1:1"));
}
