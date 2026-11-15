use super::*;
struct Input {
    duration: u32,
    fps: u32,
    body: bool,
    fail_fps: bool,
    reads: Vec<&'static str>,
}
impl Provider for Input {
    fn file(&self) -> u32 {
        0x0102_0016
    }
    fn generation(&mut self) -> Result<u32> {
        self.reads.push("generation");
        Ok(1)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<Binding>> {
        self.reads.push("resolve");
        Ok(Some(Binding {
            directory_index: 7,
            hash: 9,
        }))
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        self.reads.push("body");
        Ok(self.body)
    }
}
impl Source for Input {
    fn duration(&mut self, _: Binding) -> Result<u32> {
        self.reads.push("duration");
        Ok(self.duration)
    }
    fn fps_bits(&mut self, _: Binding) -> Result<u32> {
        self.reads.push("fps");
        ensure!(!self.fail_fps, "synthetic FPS unavailable");
        Ok(self.fps)
    }
}
fn fixture() -> (Store, NodeId, Input) {
    let owner = OwnerId::new(3).unwrap();
    let mut ids = Ids::from_constructor(owner, 4);
    let mut store = Store::from_constructor(owner, 4);
    let id = store.allocate(&mut ids, 0x0102_0016, 0x0400_0007).unwrap();
    store.bind_factory_context(id, 0, 3).unwrap();
    // Independent synthetic returned-factory fixture. No live game payload.
    store.headers.get_mut(&id).unwrap().stage = super::super::Stage::ReturnedWithoutBody;
    (
        store,
        id,
        Input {
            duration: 120,
            fps: 30f32.to_bits(),
            body: true,
            fail_fps: false,
            reads: Vec::new(),
        },
    )
}
#[test]
fn zero_tick_still_resolves_and_reads_body_scalars() {
    let (mut store, id, mut input) = fixture();
    let mut trace = Trace::default();
    store.forward_zero(id, &mut input, &mut trace).unwrap();
    assert_eq!(
        input.reads,
        ["generation", "resolve", "body", "duration", "fps"]
    );
    assert_eq!(trace.stage(), Stage::Returned);
    assert_eq!(trace.duration_raw(), Some(120));
    assert_eq!(store.get(id).unwrap().word(0x80), Some(0));
    assert_eq!(store.get(id).unwrap().word(0x84), Some(0));
    assert!(store.get(id).unwrap().initializer_returned());
    assert!(store.forward_zero(id, &mut input, &mut trace).is_err());
}
#[test]
fn missing_body_skips_scalars_but_clears_both_phases() {
    let (mut store, id, mut input) = fixture();
    input.body = false;
    let mut trace = Trace::default();
    store.forward_zero(id, &mut input, &mut trace).unwrap();
    assert_eq!(input.reads, ["generation", "resolve", "body"]);
    assert_eq!(trace.body_bound(), Some(false));
    assert_eq!(trace.duration_raw(), None);
    assert_eq!(store.get(id).unwrap().word(0x84), Some(0));
}
#[test]
fn fps_failure_retains_reached_duration_and_phase_stores() {
    let (mut store, id, mut input) = fixture();
    input.fail_fps = true;
    let mut trace = Trace::default();
    assert!(store.forward_zero(id, &mut input, &mut trace).is_err());
    assert_eq!(trace.stage(), Stage::Failed);
    assert_eq!(trace.duration_raw(), Some(120));
    assert_eq!(trace.fps_bits(), None);
    assert_eq!(store.get(id).unwrap().word(0x80), Some(0));
    assert_eq!(store.get(id).unwrap().word(0x84), Some(0));
}
#[test]
fn source_zero_duration_and_infinite_fps_are_not_faked_into_zero() {
    for (duration, fps) in [(0, 30f32.to_bits()), (120, f32::INFINITY.to_bits())] {
        let (mut store, id, mut input) = fixture();
        input.duration = duration;
        input.fps = fps;
        let mut trace = Trace::default();
        store.forward_zero(id, &mut input, &mut trace).unwrap();
        assert!(f32::from_bits(trace.result_bits().unwrap()).is_nan());
        assert!(f32::from_bits(store.get(id).unwrap().word(0x84).unwrap()).is_nan());
    }
}
