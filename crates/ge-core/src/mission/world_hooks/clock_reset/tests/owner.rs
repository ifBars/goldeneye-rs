use super::*;

#[test]
fn constructor_reads_actual_provider_sixteen_times_instead_of_leaving_timer_defaults() {
    let mut clock = Samples::constant(0x4140_0000);
    let owner = Owner::from_constructor(&mut clock).unwrap();
    assert_eq!(clock.calls, 16);
    assert_eq!(owner.stage(), Stage::Complete);
    assert_eq!(owner.completed_resets(), 1);
    assert_eq!(owner.reads_in_attempt(), 16);
    assert_eq!(owner.total_reads(), 16);
    for offset in [
        0x08, 0x0c, 0x28, 0x2c, 0x48, 0x4c, 0x68, 0x6c, 0xac, 0xb0, 0xd8, 0xdc, 0xe0, 0xe4, 0xe8,
        0xec,
    ] {
        assert_eq!(owner.word(offset), Some(0x4140_0000));
    }
    assert!(owner.buffers().iter().all(|buffer| buffer.growth() == 0
        && buffer.capacity() == 0
        && buffer.count() == 0
        && !buffer.has_backing()));
    assert_eq!(
        owner.clears_in_attempt().map(|clear| clear.unwrap().buffer),
        [0, 1, 2, 5]
    );
    assert!(
        owner
            .clears_in_attempt()
            .iter()
            .all(|clear| !clear.unwrap().stats.had_backing)
    );
    assert_eq!(owner.word(0x03), None);
    assert_eq!(owner.word(0x14), None);
}

#[test]
fn later_reset_places_each_of_sixteen_samples_in_source_order_and_distinguishes_signed_zero() {
    let mut owner = owner();
    let mut clock = Samples {
        words: vec![
            0x8000_0000,
            0x8000_0000,
            0x3f80_0000,
            0x4000_0000,
            0x4040_0000,
            0x4080_0000,
            0x40a0_0000,
            0x40c0_0000,
            0x40e0_0000,
            0x4100_0000,
            0x4110_0000,
            0x4120_0000,
            0x4130_0000,
            0x4140_0000,
            0x4150_0000,
            0x4160_0000,
        ],
        calls: 0,
        fail_at: None,
    };
    owner.reset(&mut clock).unwrap();
    assert_eq!(clock.calls, 16);
    assert_eq!(owner.completed_resets(), 2);
    assert_eq!(owner.total_reads(), 32);
    for (offset, expected) in [
        (0x08, 0),
        (0x0c, 0x8000_0000),
        (0x28, 0x3f80_0000),
        (0x2c, 0x4000_0000),
        (0x48, 0x4040_0000),
        (0x4c, 0x4080_0000),
        (0x68, 0x40a0_0000),
        (0x6c, 0x40c0_0000),
        (0xe0, 0x40e0_0000),
        (0xe4, 0x4100_0000),
        (0xe8, 0x4110_0000),
        (0xec, 0x4120_0000),
        (0xd8, 0x4130_0000),
        (0xdc, 0x4140_0000),
        (0xac, 0x4150_0000),
        (0xb0, 0x4160_0000),
    ] {
        assert_eq!(owner.word(offset), Some(expected));
    }
    for offset in [
        0x00, 0x04, 0x10, 0x30, 0x50, 0xc4, 0xb4, 0xb8, 0xbc, 0xc0, 0xc8,
    ] {
        assert_eq!(owner.word(offset), Some(0));
    }
}

#[test]
fn reset_frees_four_backings_even_with_zero_count_and_preserves_growth_and_two_other_buffers() {
    let mut owner = owner();
    supply(&mut owner, 0, 3, 7, 0, vec![1; 12]);
    *owner.buffer_mut(1).unwrap() = Buffer::empty(9);
    supply(&mut owner, 2, 2, 11, 2, vec![2; 8]);
    supply(&mut owner, 3, 5, 13, 3, vec![3; 20]);
    supply(&mut owner, 4, 7, 15, 4, vec![4; 28]);
    supply(&mut owner, 5, 1, 17, 1, vec![5; 4]);
    let preserved = [
        owner.buffers()[3].backing().unwrap().as_ptr(),
        owner.buffers()[4].backing().unwrap().as_ptr(),
    ];
    owner.reset(&mut Samples::constant(0x4000_0000)).unwrap();
    for (index, growth) in [(0, 7), (1, 9), (2, 11), (5, 17)] {
        let buffer = &owner.buffers()[index];
        assert_eq!(
            (buffer.capacity(), buffer.count(), buffer.growth()),
            (0, 0, growth)
        );
        assert!(!buffer.has_backing());
    }
    for (index, capacity, growth, count, byte) in [(3, 5, 13, 3, 3), (4, 7, 15, 4, 4)] {
        let buffer = &owner.buffers()[index];
        assert_eq!(
            (buffer.capacity(), buffer.growth(), buffer.count()),
            (capacity, growth, count)
        );
        assert!(buffer.backing().unwrap().iter().all(|value| *value == byte));
        assert_eq!(buffer.backing().unwrap().as_ptr(), preserved[index - 3]);
    }
    let receipts = owner.clears_in_attempt();
    assert_eq!(receipts.map(|clear| clear.unwrap().buffer), [0, 1, 2, 5]);
    assert_eq!(receipts[0].unwrap().stats.previous_count, 0);
    assert!(receipts[0].unwrap().stats.had_backing);
    assert!(receipts[0].unwrap().stats.bytes_released >= 12);
    assert!(!receipts[1].unwrap().stats.had_backing);
    assert!(owner.buffer_mut(6).is_none());
}

#[test]
fn failure_on_third_read_retains_first_pair_and_first_clear_but_not_later_writes() {
    let mut owner = owner();
    for offset in WORD_OFFSETS {
        owner.store(offset, 0x4248_0000);
    }
    for index in 0..6 {
        supply(
            &mut owner,
            index,
            1,
            20 + index as u32,
            1,
            vec![index as u8; 4],
        );
    }
    let mut clock = Samples::failing(3);
    assert!(owner.reset(&mut clock).is_err());
    assert_eq!(owner.stage(), Stage::Failed { ordinal: 3 });
    assert_eq!(
        (
            owner.reads_in_attempt(),
            owner.total_reads(),
            owner.completed_resets()
        ),
        (3, 19, 1)
    );
    assert_eq!(owner.word(0x00), Some(0));
    assert_eq!(owner.word(0x04), Some(0));
    assert_eq!(owner.word(0x10), Some(0));
    assert_eq!(owner.word(0x08), Some(0x4040_0000));
    assert_eq!(owner.word(0x0c), Some(0x4040_0000));
    for offset in [0x28, 0x2c, 0x30, 0xc4, 0xac] {
        assert_eq!(owner.word(offset), Some(0x4248_0000));
    }
    assert!(!owner.buffers()[0].has_backing());
    assert!(owner.buffers()[1..].iter().all(Buffer::has_backing));
    assert_eq!(owner.clears_in_attempt()[0].unwrap().buffer, 0);
    assert_eq!(&owner.clears_in_attempt()[1..], &[None, None, None]);
}

#[test]
fn failure_on_sixteenth_read_keeps_all_clears_and_first_last_pair_store() {
    let mut owner = owner();
    owner.store(0xac, 0x4248_0000);
    owner.store(0xb0, 0x42a0_0000);
    for index in 0..6 {
        supply(&mut owner, index, 1, 0, 1, vec![index as u8; 4]);
    }
    let mut clock = Samples::failing(16);
    assert!(owner.reset(&mut clock).is_err());
    assert_eq!(owner.stage(), Stage::Failed { ordinal: 16 });
    assert_eq!(owner.reads_in_attempt(), 16);
    assert_eq!(owner.completed_resets(), 1);
    assert_eq!(owner.word(0xac), Some(0x4040_0000));
    assert_eq!(owner.word(0xb0), Some(0x42a0_0000));
    assert!(owner.clears_in_attempt().iter().all(Option::is_some));
    assert_eq!(owner.word(0xc4), Some(0));
    assert_eq!(owner.word(0xe8), Some(0x4040_0000));
    for index in [0, 1, 2, 5] {
        assert!(!owner.buffers()[index].has_backing());
    }
    assert!(owner.buffers()[3].has_backing());
    assert!(owner.buffers()[4].has_backing());
}

#[test]
fn constructor_provider_failure_returns_error_without_inventing_a_completed_owner() {
    let mut clock = Samples::failing(4);
    let result = Owner::from_constructor(&mut clock);
    let error = result
        .err()
        .expect("constructor must report actual clock failure");
    assert_eq!(clock.calls, 4);
    assert!(error.to_string().contains("read 4"));
    assert!(format!("{error:#}").contains("authored clock unavailable"));
}

#[test]
fn later_success_reuses_partial_owner_and_counts_constructor_separately_from_completed_resets() {
    let mut owner = owner();
    supply(&mut owner, 0, 1, 5, 1, vec![9; 4]);
    supply(&mut owner, 3, 2, 6, 2, vec![7; 8]);
    let preserved = owner.buffers()[3].backing().unwrap().as_ptr();
    assert!(owner.reset(&mut Samples::failing(3)).is_err());
    let mut actual = Samples::constant(0x40a0_0000);
    owner.reset(&mut actual).unwrap();
    assert_eq!(actual.calls, 16);
    assert_eq!(owner.stage(), Stage::Complete);
    assert_eq!(owner.completed_resets(), 2);
    assert_eq!(owner.total_reads(), 35);
    assert_eq!(owner.reads_in_attempt(), 16);
    assert!(!owner.clears_in_attempt()[0].unwrap().stats.had_backing);
    assert_eq!(owner.buffers()[0].growth(), 5);
    assert_eq!(owner.buffers()[3].backing().unwrap().as_ptr(), preserved);
    assert_eq!(owner.buffers()[3].count(), 2);
    assert_eq!(owner.word(0xe8), Some(0x40a0_0000));
}

#[test]
fn raw_second_samples_and_negative_values_are_not_clamped_or_silently_replaced() {
    let mut owner = owner();
    let mut clock = Samples::constant(0x3f80_0000);
    clock.words[0] = 0xc000_0000;
    clock.words[1] = 0x7fc0_1234;
    clock.words[2] = 0x7f80_0000;
    clock.words[3] = 0x8000_0000;
    owner.reset(&mut clock).unwrap();
    assert_eq!(owner.word(0x08), Some(0xc000_0000));
    assert_eq!(owner.word(0x0c), Some(0x7fc0_1234));
    assert_eq!(owner.word(0x28), Some(0x7f80_0000));
    assert_eq!(owner.word(0x2c), Some(0x8000_0000));
    assert_eq!(owner.stage(), Stage::Complete);
}
