use super::super::initialization::tests as init;
use super::*;
struct Reader {
    generation: Option<u32>,
    found: bool,
    body: Body,
    flags: u32,
    fps: u8,
    script_fps: u32,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
}
impl Reader {
    fn read(&mut self, at: &'static str) -> Result<()> {
        self.calls.push(at);
        ensure!(self.fail != Some(at), "authored failure at {at}");
        Ok(())
    }
}
impl Source for Reader {
    fn file(&self) -> u32 {
        0x0102_00ab
    }
    fn generation(&mut self, _: u32) -> Result<Option<u32>> {
        self.read("generation")?;
        Ok(self.generation)
    }
    fn resolve(&mut self, _: u32, _: u32) -> Result<Option<Binding>> {
        self.read("resolve")?;
        Ok(self.found.then_some(binding()))
    }
    fn body(&mut self, _: u32, _: Binding) -> Result<Body> {
        self.read("body")?;
        Ok(self.body)
    }
    fn timing_flags(&mut self, _: u32) -> Result<u32> {
        self.read("timing")?;
        Ok(self.flags)
    }
    fn animation_fps(&mut self, _: u32, _: Binding) -> Result<u8> {
        self.read("fps")?;
        Ok(self.fps)
    }
    fn script_fps(&mut self) -> Result<u32> {
        self.read("script_fps")?;
        Ok(self.script_fps)
    }
}
fn binding() -> Binding {
    Binding { directory_index: 3 }
}
fn reader() -> Reader {
    Reader {
        generation: Some(0),
        found: true,
        body: Body::Resident,
        flags: 0,
        fps: 30,
        script_fps: 30f32.to_bits(),
        fail: None,
        calls: Vec::new(),
    }
}
fn fixture() -> (Store, NodeId) {
    let (mut s, id) = init::fixture();
    s.initialize_phase_zero(id, &mut init::reader(), init::inputs())
        .unwrap();
    (s, id)
}
#[test]
fn actual_retained_child_finishes_zero_position_after_cache_body_and_timing_reads() {
    let (mut s, id) = fixture();
    let mut r = reader();
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    let h = s.get(id).unwrap();
    assert_eq!(h.stage(), Stage::ReturnedInitialized);
    assert_eq!(
        r.calls,
        [
            "generation",
            "resolve",
            "body",
            "timing",
            "fps",
            "script_fps"
        ]
    );
    assert_eq!(h.clip_cached(0), Some(binding()));
    assert_eq!(h.clip_word(0, 8), Some(0));
    assert_eq!(h.clip_word(0, 0xc), None);
    assert_eq!(h.word(0x28), Some(0));
    assert_eq!(h.word(0x68), Some(0));
    assert_eq!(h.word(0x84), Some((-1f32).to_bits()));
    assert_eq!(h.word(0), None);
    assert_eq!(h.word(4), None);
    let calls = r.calls.clone();
    assert!(s.prepare_phase_zero_position(id, &mut r).is_err());
    assert_eq!(r.calls, calls);
}
#[test]
fn later_provider_failure_retains_cache_before_reading_position_or_replaying_lookup() {
    for at in ["body", "timing", "fps", "script_fps"] {
        let (mut s, id) = fixture();
        let mut r = reader();
        r.fail = Some(at);
        assert!(s.prepare_phase_zero_position(id, &mut r).is_err());
        let h = s.get(id).unwrap();
        assert_eq!(h.stage(), Stage::Failed);
        assert_eq!(h.clip_cached(0), Some(binding()));
        assert_eq!(h.clip_word(0, 8), Some(0));
        assert_eq!(h.word(0x68), Some((-1f32).to_bits()));
        assert!(h.position().unwrap().position_bits.is_none());
        let calls = r.calls.clone();
        assert!(s.prepare_phase_zero_position(id, &mut r).is_err());
        assert_eq!(r.calls, calls);
    }
}
#[test]
fn missing_metadata_is_a_null_cache_with_old_generation_and_skips_all_fps_reads() {
    let (mut s, id) = fixture();
    let mut r = reader();
    r.found = false;
    r.fail = Some("fps");
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    let h = s.get(id).unwrap();
    assert_eq!(h.stage(), Stage::ReturnedInitialized);
    assert_eq!(h.clip_word(0, 0xc), Some(0));
    assert_eq!(h.clip_word(0, 8), Some(u32::MAX));
    assert_eq!(r.calls, ["generation", "resolve", "timing"]);
    assert_eq!(h.position().unwrap().body_bound, Some(false));
}
#[test]
fn cache_hit_skips_lookup_and_unavailable_body_retains_the_cached_binding() {
    let (mut s, id) = fixture();
    let mut r = reader();
    r.body = Body::Pending;
    let h = s.headers.get_mut(&id).unwrap();
    h.set_clip_cache(0, Some(binding())).unwrap();
    h.clip_write(0, 8, &0u32.to_be_bytes()).unwrap();
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    let h = s.get(id).unwrap();
    assert_eq!(r.calls, ["generation", "body"]);
    assert_eq!(h.stage(), Stage::AwaitingAnimationBody);
    assert_eq!(h.clip_cached(0), Some(binding()));
}
#[test]
fn unavailable_owner_and_unknown_matching_cache_do_not_become_missing_animations() {
    let (mut s, id) = fixture();
    let mut r = reader();
    r.generation = None;
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    assert_eq!(s.get(id).unwrap().stage(), Stage::AwaitingAnimationOwner);
    assert_eq!(r.calls, ["generation"]);
    let (mut s, id) = fixture();
    let mut r = reader();
    r.generation = Some(u32::MAX);
    // Inline constructor's +c is known NULL, even when its generation matches.
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    assert!(!r.calls.contains(&"resolve"));
    assert_eq!(
        s.get(id).unwrap().position().unwrap().body_bound,
        Some(false)
    );
}
#[test]
fn only_low_two_timing_bits_gate_curves_and_do_not_preflight_fps() {
    for flags in [1, 2, 0x8000_0003] {
        let (mut s, id) = fixture();
        let mut r = reader();
        r.flags = flags;
        r.fail = Some("fps");
        s.prepare_phase_zero_position(id, &mut r).unwrap();
        let h = s.get(id).unwrap();
        assert_eq!(h.stage(), Stage::AwaitingTimingCurve);
        assert_eq!(h.word(0x68), Some((-1f32).to_bits()));
        assert!(!r.calls.contains(&"fps"));
    }
    let (mut s, id) = fixture();
    let mut r = reader();
    r.flags = 0x8000_0004;
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    assert_eq!(s.get(id).unwrap().stage(), Stage::ReturnedInitialized);
}
#[test]
fn fps_conversion_precedes_bounds_and_nonzero_root_motion_stays_a_distinct_dependency() {
    let (mut s, id) = fixture();
    let mut r = reader();
    r.script_fps = 15f32.to_bits();
    let h = s.headers.get_mut(&id).unwrap();
    h.initialization.as_mut().unwrap().selected_start = Some(-2);
    h.w(0x20, 2);
    s.prepare_phase_zero_position(id, &mut r).unwrap();
    let h = s.get(id).unwrap();
    assert_eq!(h.stage(), Stage::AwaitingRootMotion);
    assert_eq!(h.position().unwrap().frame_bits, Some(4f32.to_bits()));
    assert_eq!(h.word(0x28), Some(0.4f32.to_bits()));
    assert!(h.position().unwrap().root_motion_pending);
}
#[test]
fn zero_span_preserves_the_native_division_write_before_host_nonfinite_limit() {
    let (mut s, id) = fixture();
    let mut r = reader();
    s.headers.get_mut(&id).unwrap().h(0x70, 0);
    assert!(s.prepare_phase_zero_position(id, &mut r).is_err());
    let h = s.get(id).unwrap();
    assert_eq!(h.stage(), Stage::Failed);
    assert!(f32::from_bits(h.word(0x28).unwrap()).is_nan());
    assert_eq!(h.word(0x68), Some(0));
}
