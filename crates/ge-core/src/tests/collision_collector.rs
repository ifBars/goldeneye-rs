use super::*;
#[test]
fn repeated_candidate_ordinal_reuses_inverse_but_current_translation_still_binds() {
    use crate::collision::frame::FrameCache;
    let mut first = [0.; 16];
    for i in [0, 5, 10, 15] {
        first[i] = 1.;
    }
    first[12] = 10.;
    let mut second = first;
    second[0] = 2.;
    second[12] = 20.;
    let a = CollectedRecord::from_dispatch(&CollectionContext::map_placement(first, 0), 1, 0, 0);
    let b = CollectedRecord::from_dispatch(&CollectionContext::map_placement(second, 0), 2, 0, 2);
    assert_eq!(a.frame_key(), b.frame_key());
    let mut cache = FrameCache::new(None);
    cache
        .resolve(a.frame_key(), || Ok((a.matrix, a.flags_raw)))
        .unwrap();
    let frame = cache
        .resolve(b.frame_key(), || {
            panic!("same ordinal skips new inverse and flags")
        })
        .unwrap();
    assert_eq!(
        frame
            .query_origin([25., 0., 0.], [b.matrix[12], b.matrix[13], b.matrix[14]])
            .unwrap(),
        [5., 0., 0., 0.]
    );
}
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

#[test]
fn append_retains_raw_lanes_context_and_low_sixteen_flag_and_slot_bits() {
    let mut context = CollectionContext {
        matrix: IDENTITY,
        owner_key: 0x12345678,
        slot_raw: 0x9876fffe,
    };
    context.matrix[7] = f32::from_bits(0x7fc01234);
    context.matrix[3] = -0.;
    let record = CollectedRecord::from_dispatch(&context, 0xabc, 0xdef, 0x1234fedc);
    assert_eq!(
        (record.geometry_key, record.optional_context_key),
        (0xabc, 0xdef)
    );
    assert_eq!(
        (record.owner_key, record.slot, record.flags_raw),
        (0x12345678, -2, 0xfedc)
    );
    assert_eq!(record.matrix[7].to_bits(), 0x7fc01234);
    assert_eq!(record.matrix[3].to_bits(), (-0f32).to_bits());
    assert_eq!(
        record.frame_key(),
        FrameKey {
            owner_key: 0x12345678,
            slot: -2
        }
    );
}

#[test]
fn map_context_uses_candidate_ordinal_and_does_not_promote_engine_flags() {
    let zone = CollectionContext::map_zone(IDENTITY);
    let placement = CollectionContext::map_placement(IDENTITY, 0x10001);
    assert_eq!(
        CollectedRecord::from_dispatch(&zone, 1, 0, 0).frame_key(),
        FrameKey {
            owner_key: 0,
            slot: -1
        }
    );
    assert_eq!(
        CollectedRecord::from_dispatch(&placement, 1, 0, 0).frame_key(),
        FrameKey {
            owner_key: 0,
            slot: 1
        }
    );
    assert_eq!(map_placement_flags(0x40010000, 0), 0x40010000);
    assert_eq!(map_placement_flags(0x40010000, 4), 0x40010002);
    assert_eq!(map_placement_flags(0x40010000, 0xfffb), 0x40010000);
    assert_eq!(map_placement_flags(2, 4), 2);
}

#[test]
fn full_buffer_does_not_address_sources_grow_or_discard_prior_records() {
    let context = CollectionContext::map_zone(IDENTITY);
    let mut records = RecordBuffer::new(2).unwrap();
    for geometry in [5, 5] {
        assert!(
            records
                .append(|| Ok(CollectedRecord::from_dispatch(&context, geometry, 0, 2)))
                .unwrap()
        );
    }
    assert_eq!(
        records
            .records()
            .iter()
            .map(|r| r.geometry_key)
            .collect::<Vec<_>>(),
        [5, 5]
    );
    for _ in 0..3 {
        assert!(
            !records
                .append(|| panic!("full must not address record sources"))
                .unwrap()
        );
    }
    assert_eq!(records.records().len(), 2);
    assert_eq!(records.capacity(), 2);
    let mut empty = RecordBuffer::new(0).unwrap();
    assert!(!empty.append(|| panic!("zero capacity")).unwrap());
    assert!(RecordBuffer::new(1_000_001).is_err());
}

#[test]
fn preparation_errors_leave_prior_append_order_intact() {
    let context = CollectionContext::map_zone(IDENTITY);
    let mut records = RecordBuffer::new(3).unwrap();
    assert!(
        records
            .append(|| Ok(CollectedRecord::from_dispatch(&context, 9, 0, 0)))
            .unwrap()
    );
    assert!(
        records
            .append(|| anyhow::bail!("missing adapter data"))
            .is_err()
    );
    assert!(
        records
            .append(|| Ok(CollectedRecord::from_dispatch(&context, 2, 0, 0)))
            .unwrap()
    );
    assert_eq!(
        records
            .records()
            .iter()
            .map(|r| r.geometry_key)
            .collect::<Vec<_>>(),
        [9, 2]
    );
}

#[test]
fn bypass_and_selected_component_preserve_lazy_source_access() {
    let bypass = GeometryFilter {
        current_slot: u32::MAX,
        flags_raw: 0x400,
    };
    assert_eq!(
        bypass
            .select::<u8>(|| panic!("bypass mask unread"), |_| panic!("unread"))
            .unwrap(),
        GeometrySelection::Keep
    );
    let direct = GeometryFilter {
        current_slot: 3,
        flags_raw: 0,
    };
    let mut called = Vec::new();
    assert_eq!(
        direct
            .select(
                || Ok(0b1011),
                |slot| {
                    called.push(slot);
                    Ok(slot)
                }
            )
            .unwrap(),
        GeometrySelection::Replace(3)
    );
    assert_eq!(called, [3]);
    assert!(
        direct
            .select::<u8>(|| anyhow::bail!("missing mask"), |_| panic!("unread"))
            .is_err()
    );
    assert!(
        direct
            .select::<u8>(|| Ok(8), |_| anyhow::bail!("missing selected body"))
            .is_err()
    );
}

#[test]
fn permitted_fallbacks_descend_before_base_default_and_slot_zero_is_special() {
    let mut called = Vec::new();
    let filter = GeometryFilter {
        current_slot: 4,
        flags_raw: 0b1101,
    };
    let selected = filter
        .select(
            || Ok(0b0111),
            |slot| {
                called.push(slot);
                Ok(slot)
            },
        )
        .unwrap();
    assert_eq!(selected, GeometrySelection::Replace(2));
    assert_eq!(called, [2]);
    assert_eq!(
        GeometryFilter {
            current_slot: 4,
            flags_raw: 1
        }
        .select::<u8>(|| Ok(7), |_| panic!("flags reject all fallback slots"))
        .unwrap(),
        GeometrySelection::Keep
    );
    assert_eq!(
        GeometryFilter {
            current_slot: 4,
            flags_raw: 0
        }
        .select::<u8>(|| Ok(7), |_| panic!("flags reject all fallback slots"))
        .unwrap(),
        GeometrySelection::Reject
    );
    assert_eq!(
        GeometryFilter {
            current_slot: 0,
            flags_raw: 0
        }
        .select::<u8>(|| Ok(0), |_| panic!("no slot"))
        .unwrap(),
        GeometrySelection::Keep
    );
    assert_eq!(
        GeometryFilter {
            current_slot: 0,
            flags_raw: 0
        }
        .select(|| Ok(1), Ok::<_, anyhow::Error>)
        .unwrap(),
        GeometrySelection::Replace(0)
    );
}

#[test]
fn powerpc_shift_semantics_and_original_eight_bit_slot_argument_are_retained() {
    assert_eq!(native_shift(1, 32), 0);
    assert_eq!(native_shift(1, 64), 1);
    assert_eq!(native_shift(2, 31), 0);
    assert_eq!(
        GeometryFilter {
            current_slot: 256,
            flags_raw: 0
        }
        .select(|| Ok(1), Ok::<_, anyhow::Error>)
        .unwrap(),
        GeometrySelection::Replace(0)
    );
    assert!(
        GeometryFilter {
            current_slot: 1_000_001,
            flags_raw: 0
        }
        .select::<u8>(|| Ok(0), |_| panic!("no match"))
        .is_err()
    );
}
