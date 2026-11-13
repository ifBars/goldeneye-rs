use super::*;
use crate::mission::player::components::Registry;

fn helper(target: Target) -> MaskHelper {
    let mut helper = MaskHelper::new(target);
    helper.receiver = Some(
        Registry::from_constructor(4)
            .allocate(PlayerId::new(1).unwrap())
            .unwrap(),
    );
    helper.searched = true;
    helper
}
struct Source {
    reads: Vec<u32>,
    results: std::collections::VecDeque<MaskLookup>,
}
impl MaskSource for Source {
    fn mask(&mut self, _: Identity, reference: u32) -> Result<MaskLookup> {
        self.reads.push(reference);
        self.results.pop_front().context("unexpected mask lookup")
    }
}
fn source(results: impl IntoIterator<Item = MaskLookup>) -> Source {
    Source {
        reads: Vec::new(),
        results: results.into_iter().collect(),
    }
}
fn value(words: [u32; 4]) -> MaskLookup {
    MaskLookup::Resident(MaskValue::new(7, true, words))
}

#[test]
fn classification_reads_secondary_only_for_the_fallback_case() {
    for (primary, expected) in [
        (0x0900_071e, 1),
        (0x0900_0721, 1),
        (0x0900_0cf7, 2),
        (0x0900_0cf8, 2),
        (0x0900_083a, 3),
    ] {
        assert_eq!(
            classify(primary, || panic!("unreached secondary read")).unwrap(),
            expected
        );
    }
    assert_eq!(classify(0, || Ok(0)).unwrap(), 0);
    assert_eq!(classify(0, || Ok(u32::MAX)).unwrap(), 1);
    assert!(classify(0, || anyhow::bail!("unknown secondary")).is_err());
}

#[test]
fn helpers_apply_all_state_mappings_in_physical_slot_order() {
    let a = [0x0f0f_0f0f; 4];
    let b = [0xff00_ff00; 4];
    for state in 0..=4 {
        for target in [Target::FirstPerson, Target::ThirdPerson] {
            let expected = match (target, state) {
                (Target::FirstPerson, 0) => vec![(0, a), (1, a), (2, a)],
                (Target::FirstPerson, 1..=3) => vec![(0, a), (1, b), (2, a)],
                (Target::FirstPerson, _) => vec![(2, a)],
                (Target::ThirdPerson, 0) => vec![(0, a), (1, a), (2, a), (3, a)],
                (Target::ThirdPerson, 1) => vec![(0, a), (1, a), (2, b), (3, b)],
                (Target::ThirdPerson, 2) => vec![(0, a), (1, [0xf00f_f00f; 4]), (2, b), (3, b)],
                (Target::ThirdPerson, 3) => vec![(0, a), (1, a), (2, b), (3, [0; 4])],
                _ => vec![],
            };
            let mut helper = helper(target);
            let mut source = source([value(a), value(b)]);
            let mut writes = Vec::new();
            helper
                .advance(state, &mut source, |_, slot, words| {
                    writes.push((slot, words));
                    Ok(())
                })
                .unwrap();
            assert_eq!(source.reads, [0x0300_0147, 0x0300_0148]);
            assert_eq!(writes, expected);
            assert!(helper.returned());
            assert!(
                helper
                    .advance(state, &mut source, |_, _, _| panic!("replay"))
                    .is_err()
            );
        }
    }
}

#[test]
fn either_missing_lookup_returns_before_any_header_write() {
    for missing in [MaskLookup::MissingDescriptor, MaskLookup::MissingBody] {
        for second in [false, true] {
            let mut helper = helper(Target::FirstPerson);
            let mut source = source(if second {
                vec![value([1; 4]), missing]
            } else {
                vec![missing]
            });
            helper
                .advance(1, &mut source, |_, _, _| {
                    panic!("missing body cannot write headers")
                })
                .unwrap();
            assert!(helper.returned());
            assert_eq!(source.reads.len(), if second { 2 } else { 1 });
        }
    }
}

#[test]
fn pending_second_body_retains_first_conversion_and_resumes_before_writes() {
    let mut helper = helper(Target::ThirdPerson);
    let mut source = source([value([1; 4]), MaskLookup::Pending, value([2; 4])]);
    helper
        .advance(1, &mut source, |_, _, _| {
            panic!("pending body cannot write headers")
        })
        .unwrap();
    assert!(!helper.returned());
    let mut slots = Vec::new();
    helper
        .advance(1, &mut source, |_, slot, _| {
            slots.push(slot);
            Ok(())
        })
        .unwrap();
    assert!(helper.returned());
    assert_eq!(source.reads, [0x0300_0147, 0x0300_0148, 0x0300_0148]);
    assert_eq!(slots, [0, 1, 2, 3]);
}

#[test]
fn missing_attribute_is_a_successful_zero_mask_and_late_failure_keeps_prefix() {
    let mut helper = helper(Target::FirstPerson);
    let mut source = source([
        MaskLookup::Resident(MaskValue::new(1, false, [0; 4])),
        value([2; 4]),
    ]);
    assert!(
        helper
            .advance(1, &mut source, |_, slot, words| {
                if slot == 1 {
                    anyhow::bail!("late write failure");
                }
                assert_eq!(words, [0; 4]);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(helper.applied_slots(), [0]);
    assert!(!helper.returned());
}

#[test]
fn null_receiver_returns_without_touching_asset_provider() {
    let mut helper = MaskHelper::new(Target::FirstPerson);
    helper.searched = true;
    let mut source = source([]);
    helper
        .advance(1, &mut source, |_, _, _| panic!("NULL receiver"))
        .unwrap();
    assert!(helper.returned());
    assert!(source.reads.is_empty());
}
