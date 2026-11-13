use super::*;
use crate::animation::{
    nodes::{Ids, OwnerId},
    script_child::{
        Binding, Provider, Store,
        initialization::{self, Storage, tests as init},
    },
};
use std::collections::BTreeMap;

const FILE: u32 = 0x0102_00aa;
struct Reader {
    fps: BTreeMap<usize, u8>,
    absent: Option<usize>,
    pending: Option<usize>,
    fail_fps: Option<usize>,
    calls: Vec<String>,
}
impl position::Source for Reader {
    fn file(&self) -> u32 {
        FILE
    }
    fn generation(&mut self, file: u32) -> Result<Option<u32>> {
        self.calls.push(format!("generation:{file:x}"));
        Ok(Some(0))
    }
    fn resolve(&mut self, _: u32, reference: u32) -> Result<Option<position::Binding>> {
        self.calls.push(format!("resolve:{reference:x}"));
        Ok(Some(position::Binding {
            directory_index: (reference & 0xffff) as usize,
        }))
    }
    fn body(&mut self, _: u32, binding: position::Binding) -> Result<position::Body> {
        let i = binding.directory_index;
        self.calls.push(format!("body:{i}"));
        Ok(if self.absent == Some(i) {
            position::Body::Absent
        } else if self.pending == Some(i) {
            position::Body::Pending
        } else {
            position::Body::Resident
        })
    }
    fn timing_flags(&mut self, _: u32) -> Result<u32> {
        anyhow::bail!("rate must not read timing flags")
    }
    fn animation_fps(&mut self, _: u32, binding: position::Binding) -> Result<u8> {
        let i = binding.directory_index;
        self.calls.push(format!("fps:{i}"));
        ensure!(self.fail_fps != Some(i), "authored FPS failure");
        self.fps.get(&i).copied().context("authored FPS absent")
    }
    fn script_fps(&mut self) -> Result<u32> {
        anyhow::bail!("rate must not read script FPS")
    }
}
fn inputs() -> Inputs {
    Inputs {
        current_file_data_available: true,
        baseline: 80,
        epsilon_bits: 0.01f32.to_bits(),
        lower_bits: 0f32.to_bits(),
        upper_bits: 1f32.to_bits(),
        snap_bits: 0.99f32.to_bits(),
        index_floor: 0,
    }
}
struct Resource;
impl Provider for Resource {
    fn file(&self) -> u32 {
        FILE
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(0)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<Binding>> {
        Ok(Some(Binding {
            directory_index: 2,
            hash: 0x0400_00aa,
        }))
    }
    fn bind_body(&mut self, _: Binding) -> Result<bool> {
        Ok(true)
    }
}
struct Setup<'a> {
    inner: init::Reader,
    thresholds: &'a [f32],
}
impl initialization::Source for Setup<'_> {
    fn file(&self) -> u32 {
        FILE
    }
    fn duration(&mut self) -> Result<u32> {
        self.inner.duration()
    }
    fn entry(&mut self, offset: u32) -> Result<initialization::Entry> {
        self.inner.entry(offset)
    }
    fn counts(&mut self, _: u32) -> Result<[u8; 2]> {
        Ok([self.thresholds.len() as u8, 0])
    }
    fn file_token(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.inner.file_token(offset, secondary, row, normal)
    }
    fn lookup_file(&mut self, token: u32) -> Result<initialization::FileLookup> {
        self.inner.lookup_file(token)
    }
    fn reference(&mut self, _: u32, _: bool, row: usize, _: u8) -> Result<u32> {
        Ok(0x8300_0000 | row as u32)
    }
    fn blend(&mut self, _: u32, row: usize) -> Result<u32> {
        Ok(self.thresholds[row].to_bits())
    }
    fn start(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.inner.start(offset, secondary, row, normal)
    }
    fn end(&mut self, offset: u32, secondary: bool, row: usize, normal: u8) -> Result<i16> {
        self.inner.end(offset, secondary, row, normal)
    }
    fn initial_index(&mut self, _: u32) -> Result<u8> {
        Ok(0)
    }
    fn flags(&mut self, _: u32) -> Result<u8> {
        Ok(0)
    }
}
fn fixture(thresholds: &[f32], coordinate: f32) -> Header {
    let owner = OwnerId::new(1).unwrap();
    let mut store = Store::from_constructor(owner, 4);
    let mut ids = Ids::from_constructor(owner, 4);
    let id = store.allocate(&mut ids, FILE, 0x8400_0001).unwrap();
    store.bind_factory_context(id, 0, 0).unwrap();
    store.initialize_prefix(id, &mut Resource).unwrap();
    store
        .initialize_phase_zero(
            id,
            &mut Setup {
                inner: init::reader(),
                thresholds,
            },
            init::inputs(),
        )
        .unwrap();
    let mut h = store.headers.remove(&id).unwrap();
    h.w(0x64, coordinate.to_bits());
    h
}
fn reader() -> Reader {
    Reader {
        fps: [(0, 20), (1, 60), (2, 100)].into(),
        absent: None,
        pending: None,
        fail_fps: None,
        calls: Vec::new(),
    }
}

#[test]
fn interval_blends_two_rates_in_order_then_applies_unsigned_baseline_reciprocal() {
    let mut h = fixture(&[0.0, 1.0, 2.0], 0.5);
    let initial = h.initialization().unwrap().clone();
    let mut source = reader();
    let mut t = Trace::new(inputs());
    evaluate(&mut h, &mut source, &mut t).unwrap();
    assert_eq!(t.selected_index, Some(0));
    assert_eq!(t.fraction_bits, Some(0.5f32.to_bits()));
    assert_eq!(t.thresholds_read, 2);
    assert_eq!((t.first_fps, t.second_fps), (Some(20), Some(60)));
    assert_eq!(t.unscaled_rate_bits, Some(40f32.to_bits()));
    assert_eq!(t.rate_bits, Some((40f32 * (1f32 / 80f32)).to_bits()));
    assert_eq!(
        source.calls,
        [
            "generation:10200aa",
            "resolve:83000000",
            "body:0",
            "fps:0",
            "generation:10200aa",
            "resolve:83000001",
            "body:1",
            "fps:1"
        ]
    );
    assert_eq!(h.clip_cached(0).unwrap().directory_index, 0);
    assert_eq!(h.clip_cached(1).unwrap().directory_index, 1);
    assert_eq!(h.initialization(), Some(&initial));
    assert!(h.position().is_none());
    assert!(evaluate(&mut h, &mut source, &mut t).is_err());
}

#[test]
fn zero_fraction_and_disable_blend_flag_do_not_read_a_second_animation() {
    for (coordinate, flags, expected) in [(1.0, 0, 1), (0.5, 0x80, 0), (0.995, 0, 1), (3.0, 0, 2)] {
        let mut h = fixture(&[0.0, 1.0, 2.0], coordinate);
        h.w(0x20, flags);
        let mut t = Trace::new(inputs());
        let mut source = reader();
        evaluate(&mut h, &mut source, &mut t).unwrap();
        assert_eq!(t.selected_index, Some(expected));
        assert_eq!(t.fraction_bits, Some(0));
        assert!(t.second.is_none());
        assert_eq!(
            source
                .calls
                .iter()
                .filter(|s| s.starts_with("fps:"))
                .count(),
            1
        );
    }
}

#[test]
fn selected_clip_cache_is_reused_without_repeating_resolution_or_resetting_storage() {
    let mut h = fixture(&[0.0, 1.0], 0.0);
    let mut source = reader();
    evaluate(&mut h, &mut source, &mut Trace::new(inputs())).unwrap();
    let storage = match h.clip_storage().unwrap() {
        Storage::Heap(records) => records.as_ptr(),
        Storage::Inline { .. } => panic!("authored heap fixture changed"),
    };
    source.calls.clear();
    evaluate(&mut h, &mut source, &mut Trace::new(inputs())).unwrap();
    assert_eq!(source.calls, ["generation:10200aa", "body:0", "fps:0"]);
    match h.clip_storage().unwrap() {
        Storage::Heap(records) => assert_eq!(records.as_ptr(), storage),
        Storage::Inline { .. } => panic!("rate query replaced retained heap storage"),
    }
}

#[test]
fn missing_first_animation_returns_unscaled_default_but_missing_second_keeps_first_rate() {
    for missing in [0, 1] {
        let mut h = fixture(&[0.0, 1.0], 0.5);
        let mut source = reader();
        source.absent = Some(missing);
        let mut t = Trace::new(inputs());
        evaluate(&mut h, &mut source, &mut t).unwrap();
        assert_eq!(
            t.rate_bits,
            Some(if missing == 0 {
                1f32.to_bits()
            } else {
                (20f32 * (1f32 / 80f32)).to_bits()
            })
        );
        assert_eq!(t.first_fps, if missing == 0 { None } else { Some(20) });
        assert_eq!(t.second_fps, None);
    }
}

#[test]
fn pending_or_failed_second_lookup_retains_first_fps_and_both_cache_prefixes() {
    for pending in [true, false] {
        let mut h = fixture(&[0.0, 1.0], 0.5);
        let mut source = reader();
        if pending {
            source.pending = Some(1);
        } else {
            source.fail_fps = Some(1);
        }
        let mut t = Trace::new(inputs());
        assert_eq!(evaluate(&mut h, &mut source, &mut t).is_ok(), pending);
        assert_eq!(t.rate_bits, None);
        assert_eq!(t.first_fps, Some(20));
        assert_eq!(t.unscaled_rate_bits, Some(20f32.to_bits()));
        assert_eq!(h.clip_cached(0).unwrap().directory_index, 0);
        assert_eq!(h.clip_cached(1).unwrap().directory_index, 1);
        if pending {
            assert_eq!(h.stage(), super::super::Stage::AwaitingAnimationBody);
        }
    }
}

#[test]
fn absent_current_file_skips_unknown_coordinate_and_normal_count() {
    let mut h = fixture(&[0.0], 0.0);
    h.bytes[0x64..0x68].fill(None);
    h.bytes[0x74] = None;
    let mut i = inputs();
    i.current_file_data_available = false;
    i.baseline = 0;
    let mut source = reader();
    let mut t = Trace::new(i);
    evaluate(&mut h, &mut source, &mut t).unwrap();
    assert_eq!(t.rate_bits, Some(1f32.to_bits()));
    assert!(source.calls.is_empty());
    assert_eq!(t.coordinate_bits, None);
}

#[test]
fn selection_preserves_nan_comparison_fallthrough_and_constructor_floor() {
    let mut h = fixture(&[0.0, 1.0, 2.0], f32::NAN);
    let mut t = Trace::new(inputs());
    select(&h, &mut t).unwrap();
    assert_eq!((t.selected_index, t.fraction_bits), (Some(2), Some(0)));
    h.w(0x64, 0.5f32.to_bits());
    let mut i = inputs();
    i.index_floor = 1;
    let mut t = Trace::new(i);
    select(&h, &mut t).unwrap();
    assert_eq!(t.selected_index, Some(1));
    assert_eq!(t.fraction_bits, Some(0.5f32.to_bits()));
}

#[test]
fn single_clip_skips_threshold_flags_and_blend_input_globals() {
    let mut h = fixture(&[0.0], 0.4);
    h.bytes[0x54..0x58].fill(None);
    h.bytes[0x20..0x24].fill(None);
    let mut i = inputs();
    i.epsilon_bits = f32::NAN.to_bits();
    i.index_floor = i32::MAX;
    let mut t = Trace::new(i);
    select(&h, &mut t).unwrap();
    assert_eq!(
        (t.selected_index, t.fraction_bits, t.thresholds_read),
        (Some(0), Some(0), 0)
    );
}
