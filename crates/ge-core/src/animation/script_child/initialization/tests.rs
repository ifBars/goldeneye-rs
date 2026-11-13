use super::*;
use std::collections::BTreeMap;
pub(in super::super) struct Reader {
    entries: BTreeMap<u32, Entry>,
    counts: [u8; 2],
    raw_index: u8,
    range: [i16; 2],
    tokens: Vec<u32>,
    lookup: FileLookup,
    fail: Option<&'static str>,
    calls: Vec<String>,
    duration: u32,
}
impl Reader {
    fn read(&mut self, name: &'static str) -> Result<()> {
        self.calls.push(name.into());
        ensure!(self.fail != Some(name), "authored failure at {name}");
        Ok(())
    }
}
impl Source for Reader {
    fn file(&self) -> u32 {
        0x0102_00ab
    }
    fn duration(&mut self) -> Result<u32> {
        self.read("duration")?;
        Ok(self.duration)
    }
    fn entry(&mut self, offset: u32) -> Result<Entry> {
        self.read("entry")?;
        self.calls.push(format!("offset:{offset}"));
        self.entries
            .get(&offset)
            .copied()
            .context("authored entry absent")
    }
    fn counts(&mut self, _: u32) -> Result<[u8; 2]> {
        self.read("counts")?;
        Ok(self.counts)
    }
    fn file_token(&mut self, _: u32, secondary: bool, row: usize, normal: u8) -> Result<u32> {
        self.read("token")?;
        self.calls.push(format!("token:{secondary}:{row}:{normal}"));
        Ok(self
            .tokens
            .get(row + if secondary { usize::from(normal) } else { 0 })
            .copied()
            .unwrap_or(u32::MAX))
    }
    fn lookup_file(&mut self, _: u32) -> Result<FileLookup> {
        self.read("lookup")?;
        Ok(self.lookup)
    }
    fn reference(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<u32> {
        self.read("reference")?;
        Ok(0x8300_0003)
    }
    fn blend(&mut self, _: u32, _: usize) -> Result<u32> {
        self.read("blend")?;
        Ok(0x7fc0_1234)
    }
    fn start(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        self.read("start")?;
        Ok(self.range[0])
    }
    fn end(&mut self, _: u32, _: bool, _: usize, _: u8) -> Result<i16> {
        self.read("end")?;
        Ok(self.range[1])
    }
    fn initial_index(&mut self, _: u32) -> Result<u8> {
        self.read("index")?;
        Ok(self.raw_index)
    }
    fn flags(&mut self, _: u32) -> Result<u8> {
        self.read("flags")?;
        Ok(1)
    }
}
pub(in super::super) fn reader() -> Reader {
    Reader {
        entries: [
            (
                0,
                Entry {
                    start: 0,
                    opcode: 2,
                    next: 16,
                },
            ),
            (
                16,
                Entry {
                    start: 0,
                    opcode: 0x12,
                    next: 255,
                },
            ),
        ]
        .into(),
        counts: [1, 0],
        raw_index: 0,
        range: [-2, 8],
        tokens: Vec::new(),
        lookup: FileLookup::Pending,
        fail: None,
        calls: Vec::new(),
        duration: 60,
    }
}
pub(in super::super) fn fixture() -> (Store, NodeId) {
    let (mut store, _, id) = super::super::tests::fixture();
    store.bind_factory_context(id, 0, 0x120).unwrap();
    store
        .initialize_prefix(id, &mut super::super::tests::source())
        .unwrap();
    (store, id)
}
pub(in super::super) fn inputs() -> Inputs {
    Inputs {
        component_speed_bits: 1f32.to_bits(),
        scaled_step_bits: 1f32.to_bits(),
        maximum_entries: 100,
    }
}
#[test]
fn fresh_selection_binds_inline_clip_then_preserves_raw_flags_without_returning_initializer() {
    let (mut store, id) = fixture();
    let mut source = reader();
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    let t = h.initialization().unwrap();
    assert_eq!(h.stage(), Stage::AwaitingAnimation);
    assert_eq!((t.entries_visited, t.selected_offset), (2, Some(0)));
    assert!(matches!(
        h.clip_storage(),
        Some(Storage::Inline {
            file: Some(0x0102_00ab),
            ..
        })
    ));
    assert_eq!(h.word(0x40), None);
    assert_eq!(h.word(0x44), Some(0x8300_0003));
    assert_eq!(h.word(0x4c), Some(0));
    assert_eq!(h.word(0x54), Some(0x7fc0_1234));
    assert_eq!(h.word(0x20), Some(0x1a0));
    assert_eq!(h.word(0x70), Some(10 << 16));
    assert_eq!(t.frame_range, Some(10));
    assert!(t.setup_returned);
    assert_eq!(source.calls.iter().filter(|c| *c == "index").count(), 2);
    let count = source.calls.len();
    assert!(
        store
            .initialize_phase_zero(id, &mut source, inputs())
            .is_err()
    );
    assert_eq!(source.calls.len(), count);
}
#[test]
fn selection_uses_storage_order_ignores_unknown_payloads_and_stops_on_first_future_play() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.entries = [
        (
            0,
            Entry {
                start: 5,
                opcode: 0xfe,
                next: 8,
            },
        ),
        (
            8,
            Entry {
                start: -1,
                opcode: 2,
                next: 8,
            },
        ),
        (
            16,
            Entry {
                start: -8,
                opcode: 2,
                next: 8,
            },
        ),
        (
            24,
            Entry {
                start: 1,
                opcode: 2,
                next: 8,
            },
        ),
    ]
    .into();
    source.duration = (-3i32) as u32;
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let t = store.get(id).unwrap().initialization().unwrap();
    assert_eq!((t.selected_offset, t.selected_start), (Some(16), Some(-8)));
    assert_eq!(t.frame_bits, Some((-0f32).to_bits()));
    assert_eq!(t.entries_visited, 4);
}
#[test]
fn no_play_returns_without_reading_clip_payload_or_syncing_previous_phase() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.entries.insert(
        0,
        Entry {
            start: 1,
            opcode: 2,
            next: 0,
        },
    );
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    assert_eq!(h.stage(), Stage::ReturnedWithoutPlay);
    assert!(h.clip_storage().is_none());
    assert_eq!(h.word(0x84), Some((-1f32).to_bits()));
    assert_eq!(source.calls, ["duration", "entry", "offset:0"]);
}
#[test]
fn sign_change_and_selected_offset_writes_survive_later_failures() {
    for failure in ["duration", "counts", "blend", "start", "end"] {
        let (mut store, id) = fixture();
        let mut source = reader();
        source.fail = Some(failure);
        let mut timing = inputs();
        timing.scaled_step_bits = (-2f32).to_bits();
        assert!(
            store
                .initialize_phase_zero(id, &mut source, timing)
                .is_err()
        );
        let h = store.get(id).unwrap();
        assert_eq!(h.stage(), Stage::Failed);
        assert_eq!(
            (h.byte(0x77), h.word(0x68), h.word(0x84)),
            (Some(255), Some(0), Some(0))
        );
        if failure != "duration" {
            assert_eq!(h.word(0x88), Some(0));
        }
        if failure == "end" {
            assert_eq!(h.word(0x48), Some(u32::MAX));
            assert_eq!(h.word(0x50), Some(u32::MAX));
            assert_eq!(h.word(0x54), Some(0));
            assert_eq!(h.byte(0x74), Some(0));
        }
    }
}
#[test]
fn missing_external_file_skips_remaining_fields_but_keeps_raw_secondary_base_and_packed_records() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.counts = [2, 1];
    source.tokens = vec![0x0100_0009, u32::MAX, u32::MAX];
    source.lookup = FileLookup::Missing;
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    let t = h.initialization().unwrap();
    assert_eq!(t.bound, [1, 1]);
    assert_eq!(t.rows_visited, [2, 1]);
    assert!(source.calls.contains(&"token:true:0:2".into()));
    assert_eq!(source.calls.iter().filter(|c| *c == "reference").count(), 3); // includes pre-setup comparison
    assert_eq!(h.clip_word(0, 0xc), None);
    assert_eq!(h.clip_word(1, 0xc), None);
    assert_eq!(h.clip_word(1, 0x14), Some(0));
    assert_eq!(h.clip_word(2, 4), None);
}
#[test]
fn unavailable_file_lookup_and_nonpositive_range_remain_pending_before_unreached_reads() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.tokens = vec![0x0100_0009];
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    assert_eq!(h.stage(), Stage::AwaitingClipFile);
    assert_eq!(h.initialization().unwrap().pending_file, Some(0x0100_0009));
    assert!(!source.calls.contains(&"blend".into()));
    let (mut store, id) = fixture();
    let mut source = reader();
    source.range = [4, -5];
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    assert_eq!(h.stage(), Stage::AwaitingFrameCount);
    assert_eq!(h.initialization().unwrap().frame_range, Some(-9));
    assert!(!source.calls.contains(&"flags".into()));
}
#[test]
fn index_clamp_uses_count_and_preserves_unknown_unwritten_heap_record() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.counts = [2, 0];
    source.raw_index = 255;
    source.tokens = vec![u32::MAX, 0x0100_0009];
    source.lookup = FileLookup::Missing;
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    let t = h.initialization().unwrap();
    assert_eq!(h.stage(), Stage::AwaitingClipRange);
    assert_eq!(t.initial_index_clamped, Some(1));
    assert_eq!(t.initial_index_raw, Some(255));
    assert_eq!(h.word(0x6c), Some(1 << 16));
    assert!(!t.setup_returned);
}
#[test]
fn zero_progress_and_budget_exhaustion_fail_without_running_unselected_clip_setup() {
    for next in [0, 8] {
        let (mut store, id) = fixture();
        let mut source = reader();
        source.entries.insert(
            0,
            Entry {
                start: 0,
                opcode: 0x7f,
                next,
            },
        );
        let mut timing = inputs();
        timing.maximum_entries = 1;
        assert!(
            store
                .initialize_phase_zero(id, &mut source, timing)
                .is_err()
        );
        assert_eq!(store.get(id).unwrap().stage(), Stage::Failed);
        assert!(!source.calls.contains(&"counts".into()));
    }
}
#[test]
fn positive_secondary_range_returns_setup_before_raw_index_overwrite_even_outside_allocation() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.counts = [1, 1];
    source.raw_index = 255;
    store
        .initialize_phase_zero(id, &mut source, inputs())
        .unwrap();
    let h = store.get(id).unwrap();
    let t = h.initialization().unwrap();
    assert_eq!(t.initial_index_clamped, Some(1));
    assert_eq!(h.word(0x6c), Some(255 << 16));
    assert_eq!(h.stage(), Stage::AwaitingAnimation);
    assert!(t.setup_returned);
    assert_eq!(t.bound, [1, 1]);
    assert_eq!(h.clip_word(1, 0x14), Some(0));
}
#[test]
fn heap_read_failure_keeps_only_writes_reached_before_the_failed_scalar() {
    let (mut store, id) = fixture();
    let mut source = reader();
    source.counts = [2, 0];
    source.fail = Some("end");
    assert!(
        store
            .initialize_phase_zero(id, &mut source, inputs())
            .is_err()
    );
    let h = store.get(id).unwrap();
    assert_eq!(h.clip_word(0, 4), Some(0x8300_0003));
    assert_eq!(h.clip_word(0, 8), Some(u32::MAX));
    for at in [0xc, 0x10, 0x14, 0x18] {
        assert_eq!(h.clip_word(0, at), None);
    }
    assert_eq!(h.byte(0x74), Some(0));
    assert_eq!(h.initialization().unwrap().bound, [0, 0]);
    assert_eq!(h.word(0x88), Some(0));
}
