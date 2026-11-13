use super::*;
use crate::animation::script_child::initialization::{self, tests as init};

const FILE: u32 = 0x0102_00ab;

struct Resource {
    generation: u32,
    body: bool,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
}
impl Resource {
    fn read(&mut self, at: &'static str) -> Result<()> {
        self.calls.push(at);
        ensure!(self.fail != Some(at), "authored resource failure at {at}");
        Ok(())
    }
}
impl Provider for Resource {
    fn file(&self) -> u32 {
        FILE
    }
    fn generation(&mut self) -> Result<u32> {
        self.read("generation")?;
        Ok(self.generation)
    }
    fn resolve_script(&mut self, reference: u32) -> Result<Option<Binding>> {
        self.read("resolve")?;
        ensure!(
            reference == 0x8400_0002,
            "retained script reference changed"
        );
        Ok(Some(Binding {
            directory_index: 2,
            hash: 0x0400_00ab,
        }))
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        self.read("body")?;
        Ok(self.body)
    }
}
fn resource() -> Resource {
    Resource {
        generation: 0,
        body: true,
        fail: None,
        calls: Vec::new(),
    }
}

struct Position {
    generation: Option<u32>,
    body: position::Body,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
}
impl Position {
    fn read(&mut self, at: &'static str) -> Result<()> {
        self.calls.push(at);
        ensure!(self.fail != Some(at), "authored position failure at {at}");
        Ok(())
    }
}
impl position::Source for Position {
    fn file(&self) -> u32 {
        FILE
    }
    fn generation(&mut self, _: u32) -> Result<Option<u32>> {
        self.read("generation")?;
        Ok(self.generation)
    }
    fn resolve(&mut self, _: u32, reference: u32) -> Result<Option<position::Binding>> {
        self.read("resolve")?;
        ensure!(
            reference == 0x8300_0003,
            "retained animation reference changed"
        );
        Ok(Some(position::Binding { directory_index: 3 }))
    }
    fn body(&mut self, _: u32, _: position::Binding) -> Result<position::Body> {
        self.read("body")?;
        Ok(self.body)
    }
    fn timing_flags(&mut self, _: u32) -> Result<u32> {
        self.read("timing")?;
        Ok(0)
    }
    fn animation_fps(&mut self, _: u32, _: position::Binding) -> Result<u8> {
        self.read("fps")?;
        Ok(30)
    }
    fn script_fps(&mut self) -> Result<u32> {
        self.read("script_fps")?;
        Ok(30f32.to_bits())
    }
}
fn position() -> Position {
    Position {
        generation: Some(0),
        body: position::Body::Resident,
        fail: None,
        calls: Vec::new(),
    }
}

/// Delegates the real authored initializer. During re-entry, every clip setup
/// read is denied, so a cached-offset test cannot pass by recreating storage.
struct Selection {
    inner: init::Reader,
    deny_setup: bool,
    fail_duration: bool,
    alternate: bool,
    calls: Vec<String>,
}
impl Selection {
    fn setup(&mut self, name: &'static str) -> Result<()> {
        self.calls.push(name.into());
        ensure!(
            !self.deny_setup,
            "cached selection unexpectedly read {name}"
        );
        Ok(())
    }
}
impl initialization::Source for Selection {
    fn file(&self) -> u32 {
        initialization::Source::file(&self.inner)
    }
    fn duration(&mut self) -> Result<u32> {
        self.calls.push("duration".into());
        ensure!(!self.fail_duration, "authored duration failure");
        self.inner.duration()
    }
    fn entry(&mut self, offset: u32) -> Result<initialization::Entry> {
        self.calls.push(format!("entry:{offset}"));
        if self.alternate {
            return Ok(initialization::Entry {
                start: 0,
                opcode: match offset {
                    0 => 0,
                    16 => 2,
                    _ => 0x12,
                },
                next: 16,
            });
        }
        self.inner.entry(offset)
    }
    fn counts(&mut self, _: u32) -> Result<[u8; 2]> {
        self.setup("counts")?;
        Ok([2, 0])
    }
    fn file_token(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.setup("token")?;
        self.inner.file_token(offset, secondary, row, normal)
    }
    fn lookup_file(&mut self, token: u32) -> Result<initialization::FileLookup> {
        self.setup("lookup")?;
        self.inner.lookup_file(token)
    }
    fn reference(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.setup("reference")?;
        self.inner.reference(offset, secondary, row, normal)
    }
    fn blend(&mut self, _: u32, row: usize) -> Result<u32> {
        self.setup("blend")?;
        Ok((row as f32).to_bits())
    }
    fn start(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.setup("start")?;
        self.inner.start(offset, secondary, row, normal)
    }
    fn end(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.setup("end")?;
        self.inner.end(offset, secondary, row, normal)
    }
    fn initial_index(&mut self, offset: u32) -> Result<u8> {
        self.setup("index")?;
        self.inner.initial_index(offset)
    }
    fn flags(&mut self, offset: u32) -> Result<u8> {
        self.setup("flags")?;
        self.inner.flags(offset)
    }
}
fn selection(deny_setup: bool) -> Selection {
    Selection {
        inner: init::reader(),
        deny_setup,
        fail_duration: false,
        alternate: false,
        calls: Vec::new(),
    }
}
fn rate_inputs() -> rate::Inputs {
    rate::Inputs {
        current_file_data_available: true,
        baseline: 60,
        epsilon_bits: 0.01f32.to_bits(),
        lower_bits: 0f32.to_bits(),
        upper_bits: 1f32.to_bits(),
        snap_bits: 0.99f32.to_bits(),
        index_floor: 0,
    }
}
fn fixture() -> (Store, NodeId) {
    let (mut store, id) = init::fixture();
    store
        .initialize_phase_zero(id, &mut selection(false), init::inputs())
        .unwrap();
    store
        .prepare_phase_zero_position(id, &mut position())
        .unwrap();
    assert_eq!(
        store.get(id).unwrap().stage(),
        super::super::Stage::ReturnedInitialized
    );
    (store, id)
}
fn backing(header: &Header) -> *const initialization::Record {
    match header.clip_storage().unwrap() {
        initialization::Storage::Heap(records) => records.as_ptr(),
        initialization::Storage::Inline { .. } => {
            panic!("authored two-clip fixture lost heap storage")
        }
    }
}

#[test]
fn cached_initflag_update_reuses_clips_and_preserves_first_factory_traces() {
    let (mut store, id) = fixture();
    let h = store.get(id).unwrap();
    let first_init = h.initialization().unwrap().clone();
    let first_position = h.position().unwrap().clone();
    let allocation = backing(h);
    let mut root = resource();
    store.begin_initflag_update(id, &mut root).unwrap();
    assert_eq!(root.calls, ["generation", "body"]);
    let mut src = position();
    store
        .prepare_update_rate(id, &mut src, rate_inputs())
        .unwrap();
    assert_eq!(src.calls, ["generation", "body", "fps"]);
    let mut scan = selection(true);
    store.select_update(id, &mut scan, init::inputs()).unwrap();
    assert_eq!(scan.calls, ["duration", "entry:0", "entry:16"]);
    src.calls.clear();
    store.prepare_update_position(id, &mut src).unwrap();
    assert_eq!(
        src.calls,
        ["generation", "body", "timing", "fps", "script_fps"]
    );
    let h = store.get(id).unwrap();
    assert_eq!(h.stage(), super::super::Stage::ReturnedInitialized);
    assert_eq!(h.initialization(), Some(&first_init));
    assert_eq!(h.position(), Some(&first_position));
    assert_eq!(backing(h), allocation);
    let t = &h.updates()[0];
    assert_eq!(t.stage, Stage::Returned);
    assert_eq!(
        t.rate.as_ref().unwrap().rate_bits,
        Some((30f32 * (1f32 / 60f32)).to_bits())
    );
    let s = t.selection.as_ref().unwrap();
    assert_eq!(s.compared_reference, None);
    assert_eq!(s.previous_reference, None);
    assert_eq!(s.requested, None);
    assert_eq!(s.flags_raw, None);
    assert!(!s.setup_returned);
    assert_eq!(t.position.as_ref().unwrap().position_bits, Some(0));
    assert!(store.prepare_update_position(id, &mut src).is_err());
    assert_eq!(store.get(id).unwrap().updates().len(), 1);
    store.begin_initflag_update(id, &mut resource()).unwrap();
    assert_eq!(store.get(id).unwrap().updates().len(), 2);
    assert_eq!(store.get(id).unwrap().updates()[0].stage, Stage::Returned);
}

#[test]
fn source_generation_cache_updates_survive_later_body_failure() {
    let (mut store, id) = fixture();
    let mut root = resource();
    root.generation = 9;
    root.fail = Some("body");
    assert!(store.begin_initflag_update(id, &mut root).is_err());
    assert_eq!(root.calls, ["generation", "resolve", "body"]);
    let h = store.get(id).unwrap();
    assert_eq!(h.word(0x14), Some(9));
    assert_eq!(h.observed_generation(), Some(9));
    assert_eq!(h.cached().unwrap().directory_index, 2);
    assert_eq!(h.stage(), super::super::Stage::ReturnedInitialized);
    assert_eq!(h.updates()[0].stage, Stage::Failed);
    assert_eq!(h.updates()[0].body_bound, None);
    assert_eq!(h.body_bound(), Some(true));
    let mut retry = resource();
    assert!(store.begin_initflag_update(id, &mut retry).is_err());
    assert!(retry.calls.is_empty());
}

#[test]
fn animation_rate_failure_retains_cache_and_does_not_replace_initializer_history() {
    let (mut store, id) = fixture();
    let old_position = store.get(id).unwrap().position().unwrap().clone();
    store.begin_initflag_update(id, &mut resource()).unwrap();
    let mut src = position();
    src.generation = Some(7);
    src.fail = Some("fps");
    assert!(
        store
            .prepare_update_rate(id, &mut src, rate_inputs())
            .is_err()
    );
    assert_eq!(src.calls, ["generation", "resolve", "body", "fps"]);
    let h = store.get(id).unwrap();
    assert_eq!(h.clip_word(0, 8), Some(7));
    assert_eq!(h.position(), Some(&old_position));
    let t = &h.updates()[0];
    assert_eq!(t.stage, Stage::Failed);
    assert_eq!(
        t.rate.as_ref().unwrap().first.as_ref().unwrap().generation,
        Some(7)
    );
    assert_eq!(t.rate.as_ref().unwrap().rate_bits, None);
    let calls = src.calls.clone();
    assert!(
        store
            .prepare_update_rate(id, &mut src, rate_inputs())
            .is_err()
    );
    assert_eq!(src.calls, calls);
    assert!(store.begin_initflag_update(id, &mut resource()).is_err());
}

#[test]
fn selection_failure_retains_returned_rate_and_rejects_later_continuations() {
    let (mut store, id) = fixture();
    store.begin_initflag_update(id, &mut resource()).unwrap();
    let mut scan = selection(true);
    assert!(store.select_update(id, &mut scan, init::inputs()).is_err());
    assert!(scan.calls.is_empty());
    store
        .prepare_update_rate(id, &mut position(), rate_inputs())
        .unwrap();
    scan.fail_duration = true;
    assert!(store.select_update(id, &mut scan, init::inputs()).is_err());
    let h = store.get(id).unwrap();
    let t = &h.updates()[0];
    assert_eq!(t.stage, Stage::Failed);
    assert!(t.rate.as_ref().unwrap().rate_bits.is_some());
    assert_eq!(t.selection.as_ref().unwrap().duration_raw, None);
    assert!(t.position.is_none());
    let mut retry = selection(true);
    assert!(store.select_update(id, &mut retry, init::inputs()).is_err());
    assert!(retry.calls.is_empty());
    assert!(store.prepare_update_position(id, &mut position()).is_err());
    assert!(store.begin_initflag_update(id, &mut resource()).is_err());
}

#[test]
fn missing_animation_owner_or_pending_body_cannot_return_an_update() {
    for owner_missing in [true, false] {
        let (mut store, id) = fixture();
        store.begin_initflag_update(id, &mut resource()).unwrap();
        let mut src = position();
        if owner_missing {
            src.generation = None;
        } else {
            src.body = position::Body::Pending;
        }
        store
            .prepare_update_rate(id, &mut src, rate_inputs())
            .unwrap();
        let h = store.get(id).unwrap();
        let t = &h.updates()[0];
        assert_eq!(h.stage(), super::super::Stage::ReturnedInitialized);
        assert_eq!(t.stage, Stage::Pending);
        assert_eq!(
            t.boundary,
            Some(if owner_missing {
                super::super::Stage::AwaitingAnimationOwner
            } else {
                super::super::Stage::AwaitingAnimationBody
            })
        );
        assert_eq!(t.rate.as_ref().unwrap().rate_bits, None);
        assert!(t.selection.is_none() && t.position.is_none());
        assert!(
            store
                .select_update(id, &mut selection(true), init::inputs())
                .is_err()
        );
        assert!(store.begin_initflag_update(id, &mut resource()).is_err());
    }
}

#[test]
fn genuine_absent_script_returns_without_rate_selection_or_position_work() {
    let (mut store, id) = fixture();
    let old_init = store.get(id).unwrap().initialization().unwrap().clone();
    let old_position = store.get(id).unwrap().position().unwrap().clone();
    let mut root = resource();
    root.body = false;
    store.begin_initflag_update(id, &mut root).unwrap();
    let h = store.get(id).unwrap();
    let t = &h.updates()[0];
    assert_eq!(root.calls, ["generation", "body"]);
    assert_eq!(t.stage, Stage::Returned);
    assert_eq!(t.body_bound, Some(false));
    assert!(t.rate.is_none() && t.selection.is_none() && t.position.is_none());
    assert_eq!(h.initialization(), Some(&old_init));
    assert_eq!(h.position(), Some(&old_position));
    assert!(
        store
            .prepare_update_rate(id, &mut position(), rate_inputs())
            .is_err()
    );
}

#[test]
fn changed_play_retains_selected_offset_before_clip_replacement_dependency() {
    let (mut store, id) = fixture();
    let old = backing(store.get(id).unwrap());
    store.begin_initflag_update(id, &mut resource()).unwrap();
    store
        .prepare_update_rate(id, &mut position(), rate_inputs())
        .unwrap();
    let mut scan = selection(false);
    scan.alternate = true;
    store.select_update(id, &mut scan, init::inputs()).unwrap();
    assert_eq!(
        scan.calls,
        ["duration", "entry:0", "entry:16", "entry:32", "reference"]
    );
    let h = store.get(id).unwrap();
    let t = h.updates().last().unwrap();
    assert_eq!(h.word(0x88), Some(16));
    assert_eq!(backing(h), old);
    assert_eq!(t.stage, Stage::Pending);
    assert_eq!(
        t.boundary,
        Some(super::super::Stage::AwaitingClipReplacement)
    );
    assert_eq!(
        t.selection.as_ref().unwrap().compared_reference,
        Some(0x8300_0003)
    );
    assert!(t.selection.as_ref().unwrap().requested.is_none());
    assert!(store.begin_initflag_update(id, &mut resource()).is_err());
}

#[test]
fn bounded_diagnostic_history_does_not_limit_simulation_invocations() {
    let (mut store, id) = fixture();
    let original = store.get(id).unwrap().initialization().unwrap().clone();
    let mut root = resource();
    root.body = false;
    for _ in 0..40 {
        store.begin_initflag_update(id, &mut root).unwrap();
    }
    let h = store.get(id).unwrap();
    assert_eq!(h.update_count(), 40);
    assert_eq!(h.updates().len(), 8);
    assert_eq!(h.updates().first().unwrap().ordinal, 33);
    assert_eq!(h.updates().last().unwrap().ordinal, 40);
    assert!(h.updates().iter().all(|t| t.stage == Stage::Returned));
    assert_eq!(h.initialization(), Some(&original));
}

#[test]
fn unknown_old_clip_reference_stops_changed_selection_before_offset_store() {
    let (mut store, id) = fixture();
    store.begin_initflag_update(id, &mut resource()).unwrap();
    store
        .prepare_update_rate(id, &mut position(), rate_inputs())
        .unwrap();
    // Authored fault injection at the reached old-reference dependency.
    let h = store.headers.get_mut(&id).unwrap();
    h.clips = Some(initialization::Storage::Inline {
        file: Some(FILE),
        cached: None,
    });
    h.bytes[0x44..0x48].fill(None);
    let mut scan = selection(false);
    scan.alternate = true;
    assert!(store.select_update(id, &mut scan, init::inputs()).is_err());
    assert_eq!(scan.calls.last().unwrap(), "reference");
    let h = store.get(id).unwrap();
    assert_eq!(h.word(0x88), Some(0));
    let t = h.updates().last().unwrap();
    assert_eq!(t.stage, Stage::Failed);
    assert_eq!(
        t.selection.as_ref().unwrap().compared_reference,
        Some(0x8300_0003)
    );
    assert_eq!(t.selection.as_ref().unwrap().previous_reference, None);
}

#[test]
fn failed_generation_read_cannot_reuse_an_old_body_result_in_new_invocation() {
    let (mut store, id) = fixture();
    let mut root = resource();
    root.fail = Some("generation");
    assert!(store.begin_initflag_update(id, &mut root).is_err());
    assert_eq!(root.calls, ["generation"]);
    let h = store.get(id).unwrap();
    assert_eq!(h.body_bound(), Some(true));
    let t = h.updates().last().unwrap();
    assert!(!t.resource_checked);
    assert_eq!(t.body_bound, None);
    assert_eq!(t.stage, Stage::Failed);
}
