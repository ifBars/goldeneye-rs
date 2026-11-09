use super::*;
use crate::collision::{bounds::Bounds, dispatch::LocalSphere};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

type Events = Rc<RefCell<Vec<String>>>;
struct Fixture {
    events: Events,
    kinds: BTreeMap<Node, u32>,
    splits: BTreeMap<Node, Vec<Node>>,
    links: BTreeMap<Node, Vec<Option<Node>>>,
    outside: Vec<Node>,
    context: BTreeMap<Node, u16>,
    changing_count: bool,
    count_reads: usize,
}
impl Fixture {
    fn new(kinds: &[(Node, u32)]) -> Self {
        Self {
            events: Rc::default(),
            kinds: kinds.iter().copied().collect(),
            splits: BTreeMap::new(),
            links: BTreeMap::new(),
            outside: vec![],
            context: BTreeMap::new(),
            changing_count: false,
            count_reads: 0,
        }
    }
    fn event(&self, event: String) {
        self.events.borrow_mut().push(event);
    }
}
impl GeometrySource for Fixture {
    fn node_type(&mut self, n: Node) -> Result<u32> {
        self.event(format!("type{n}"));
        Ok(*self.kinds.get(&n).unwrap())
    }
    fn sphere(&mut self, _: Node) -> Result<LocalSphere> {
        panic!("line traversal has no sphere gate")
    }
    fn bounds(&mut self, n: Node) -> Result<Bounds> {
        self.event(format!("bounds{n}"));
        let shift = if self.outside.contains(&n) { 10. } else { 0. };
        Ok(Bounds {
            minimum: [shift, shift, shift, f32::NAN],
            maximum: [shift + 1., shift + 1., shift + 1., f32::NAN],
        })
    }
    fn split_child_count(&mut self, n: Node) -> Result<u32> {
        self.event(format!("count{n}"));
        self.count_reads += 1;
        if self.changing_count && self.count_reads > 1 {
            return Ok(1);
        }
        Ok(self.splits[&n].len() as u32)
    }
    fn split_child(&mut self, n: Node, i: u32) -> Result<Node> {
        self.event(format!("child{n}:{i}"));
        Ok(self.splits[&n][i as usize])
    }
    fn link_count(&mut self, n: Node) -> Result<u8> {
        self.event(format!("count{n}"));
        Ok(self.links[&n].len() as u8)
    }
    fn link_target(&mut self, n: Node, i: u8) -> Result<Option<Node>> {
        self.event(format!("link{n}:{i}"));
        Ok(self.links[&n][i as usize])
    }
    fn optional_context_index(&mut self, n: Node) -> Result<u16> {
        self.event(format!("context{n}"));
        Ok(self.context[&n])
    }
}
fn request() -> LineGeometryRequest {
    LineGeometryRequest {
        origin: [0.; 3],
        delta: [1.; 3],
        bounds: StoredBounds {
            minimum: [0.; 3],
            maximum: [1.; 3],
        },
        optional_context_key: 0,
        dispatch_flags: 0x1234,
        stop_first: false,
    }
}

#[test]
fn mesh_calls_wrapper_before_any_traversal_bounds_or_coarse_gate() {
    let mut f = Fixture::new(&[(1, 0x601)]);
    f.outside.push(1);
    let calls = RefCell::new(vec![]);
    let outcome = query(
        &mut f,
        1,
        &request(),
        DispatchLimits::default(),
        |_| panic!("mesh coarse owned by wrapper"),
        |n, c, d| {
            calls.borrow_mut().push((n, c, d));
            Ok(true)
        },
    )
    .unwrap();
    assert!(outcome.returned);
    assert_eq!(*calls.borrow(), vec![(1, 0, 0x1234)]);
    assert_eq!(*f.events.borrow(), vec!["type1"]);
}

#[test]
fn split_gates_order_bounds_coarse_context_then_mesh_and_keeps_all_hits() {
    let mut f = Fixture::new(&[(1, 0x603), (2, 0x601), (3, 0x601), (4, 0x601), (5, 0x601)]);
    f.splits.insert(1, vec![2, 3, 4, 5]);
    f.outside.push(2);
    f.context.extend([(4, 2), (5, 3)]);
    let events = f.events.clone();
    let mesh_events = events.clone();
    let q = LineGeometryRequest {
        optional_context_key: 100,
        ..request()
    };
    let outcome = query(
        &mut f,
        1,
        &q,
        DispatchLimits::default(),
        |n| {
            events.borrow_mut().push(format!("coarse{n}"));
            Ok(Some(n != 3))
        },
        |n, c, d| {
            mesh_events.borrow_mut().push(format!("mesh{n}:{c}:{d}"));
            Ok(true)
        },
    )
    .unwrap();
    assert!(outcome.returned);
    assert_eq!(outcome.mesh_calls, 2);
    assert_eq!(outcome.bounds_tests, 5);
    assert_eq!(outcome.bounds_rejections, 1);
    assert_eq!(outcome.coarse_rejections, 1);
    assert_eq!(
        *f.events.borrow(),
        vec![
            "type1",
            "bounds1",
            "count1",
            "child1:0",
            "bounds2",
            "count1",
            "child1:1",
            "bounds3",
            "coarse3",
            "count1",
            "child1:2",
            "bounds4",
            "coarse4",
            "context4",
            "type4",
            "mesh4:156:1",
            "count1",
            "child1:3",
            "bounds5",
            "coarse5",
            "context5",
            "type5",
            "mesh5:184:1",
            "count1"
        ]
    );
}

#[test]
fn split_parent_bounds_are_unconditional_even_with_incoming_bypass_flags() {
    let mut f = Fixture::new(&[(1, 0x603)]);
    f.outside.push(1);
    let q = LineGeometryRequest {
        dispatch_flags: u32::MAX,
        ..request()
    };
    let outcome = query(
        &mut f,
        1,
        &q,
        DispatchLimits::default(),
        |_| panic!(),
        |_, _, _| panic!(),
    )
    .unwrap();
    assert!(!outcome.returned);
    assert_eq!(*f.events.borrow(), vec!["type1", "bounds1"]);
}

#[test]
fn links_skip_missing_preserve_duplicates_reset_flags_and_reload_final_count() {
    let mut f = Fixture::new(&[(1, 0x608), (2, 0x601)]);
    f.links.insert(1, vec![None, Some(2), Some(2)]);
    let calls = RefCell::new(vec![]);
    let outcome = query(
        &mut f,
        1,
        &request(),
        DispatchLimits::default(),
        |_| panic!(),
        |n, c, d| {
            calls.borrow_mut().push((n, c, d));
            Ok(true)
        },
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec![(2, 0, 0), (2, 0, 0)]);
    assert_eq!(outcome.link_misses, 1);
    assert_eq!(outcome.bounds_tests, 0);
    assert_eq!(
        f.events
            .borrow()
            .iter()
            .filter(|s| s.as_str() == "count1")
            .count(),
        4
    );
}

#[test]
fn stop_first_skips_later_callbacks_and_terminating_count_reload() {
    let mut f = Fixture::new(&[(1, 0x608), (2, 0x601)]);
    f.links.insert(1, vec![Some(2), Some(999)]);
    let q = LineGeometryRequest {
        stop_first: true,
        ..request()
    };
    let outcome = query(
        &mut f,
        1,
        &q,
        DispatchLimits::default(),
        |_| panic!(),
        |_, _, _| Ok(true),
    )
    .unwrap();
    assert!(outcome.returned);
    assert_eq!(outcome.mesh_calls, 1);
    assert_eq!(
        *f.events.borrow(),
        vec!["type1", "count1", "link1:0", "type2"]
    );
}

#[test]
fn count_changes_are_observed_and_context_binding_wraps_each_level() {
    let mut f = Fixture::new(&[(1, 0x603), (2, 0x608), (3, 0x601)]);
    f.splits.insert(1, vec![2, 999]);
    f.changing_count = true;
    f.links.insert(2, vec![Some(3)]);
    f.context.extend([(2, 2), (3, 3)]);
    let q = LineGeometryRequest {
        optional_context_key: u32::MAX - 15,
        ..request()
    };
    let calls = RefCell::new(vec![]);
    query(
        &mut f,
        1,
        &q,
        DispatchLimits::default(),
        |_| Ok(None),
        |n, c, d| {
            calls.borrow_mut().push((n, c, d));
            Ok(false)
        },
    )
    .unwrap();
    assert_eq!(*calls.borrow(), vec![(3, 124, 0)]);
    assert_eq!(f.count_reads, 2);
}

#[test]
fn present_unimplemented_coarse_and_unknown_types_are_errors() {
    let mut f = Fixture::new(&[(1, 0x603), (2, 0x601)]);
    f.splits.insert(1, vec![2]);
    let error = query(
        &mut f,
        1,
        &request(),
        DispatchLimits::default(),
        |_| bail!("unsupported child metadata"),
        |_, _, _| panic!(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("unsupported child metadata"));
    let mut f = Fixture::new(&[(1, 0x607)]);
    assert!(
        query(
            &mut f,
            1,
            &request(),
            DispatchLimits::default(),
            |_| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .to_string()
        .contains("0x607")
    );
}

#[test]
fn cycles_and_excess_work_are_bounded_without_suppressing_duplicate_paths() {
    let mut f = Fixture::new(&[(1, 0x608)]);
    f.links.insert(1, vec![Some(1)]);
    assert!(
        query(
            &mut f,
            1,
            &request(),
            DispatchLimits {
                work_budget: 100,
                depth_budget: 3
            },
            |_| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .to_string()
        .contains("depth budget")
    );
    let mut f = Fixture::new(&[(1, 0x608), (2, 0x601)]);
    f.links.insert(1, vec![Some(2)]);
    assert!(
        query(
            &mut f,
            1,
            &request(),
            DispatchLimits {
                work_budget: 2,
                depth_budget: 3
            },
            |_| panic!(),
            |_, _, _| panic!()
        )
        .unwrap_err()
        .to_string()
        .contains("work budget")
    );
}
