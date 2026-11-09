use super::*;

fn budget() -> Budget {
    Budget {
        actions: 8,
        composites: 8,
    }
}

fn composite(id: u32, slots: [[u32; 3]; 4]) -> [u32; 13] {
    let mut words = [0; 13];
    words[0] = id;
    for (index, slot) in slots.into_iter().enumerate() {
        words[1 + index * 3..4 + index * 3].copy_from_slice(&slot);
    }
    words
}

#[test]
fn duplicate_action_hashes_bind_the_first_row_without_discarding_raw_words() {
    let actions = vec![
        [7, 1, 2, 3, 4],
        [7, 10, 20, 30, 40],
        [0, u32::MAX, 0x7fc0_0001, 0, 8],
    ];
    let row = composite(
        55,
        [
            [7, 0x0102_0304, 0xaabb_ccdd],
            [0, u32::MAX, 0x1234_56ff],
            [99, 9, 0xffff_ff00],
            [7, 8, 0],
        ],
    );
    let tables = ActionTables::bind(actions.clone(), vec![row], budget()).unwrap();
    assert_eq!(
        tables
            .actions()
            .iter()
            .map(|row| row.words_raw)
            .collect::<Vec<_>>(),
        actions
    );
    assert_eq!(tables.actions()[2].hash(), 0);
    assert_eq!(tables.composites()[0].words_raw, row);
    let slots = &tables.composites()[0].slots;
    assert_eq!(
        slots.map(|slot| slot.action_index),
        [Some(0), Some(2), None, Some(0)]
    );
    assert_eq!(slots[0].word1_raw, 0x0102_0304);
    assert_eq!(slots[0].word2_raw, 0xaabb_ccdd);
    assert_eq!(slots[0].low_byte(), 0xdd);
    assert_eq!(slots[1].word1_raw, u32::MAX);
    assert_eq!(slots[1].low_byte(), 0xff);
    assert_eq!(slots[2].action_hash, 99);
    assert_eq!(slots[2].low_byte(), 0);
}

#[test]
fn duplicate_composite_ids_and_slot_order_are_preserved() {
    let first = composite(8, [[2, 11, 12], [1, 21, 22], [2, 31, 32], [1, 41, 42]]);
    let second = composite(8, [[1, 51, 52], [2, 61, 62], [1, 71, 72], [2, 81, 82]]);
    let tables = ActionTables::bind(
        vec![[1, 0, 0, 0, 0], [2, 0, 0, 0, 0]],
        vec![first, second],
        budget(),
    )
    .unwrap();
    assert_eq!(
        tables
            .composites()
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [8, 8]
    );
    assert_eq!(
        tables
            .composites()
            .iter()
            .map(|row| row.words_raw)
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(
        tables.composites()[0].slots.map(|slot| slot.action_index),
        [Some(1), Some(0), Some(1), Some(0)]
    );
    assert_eq!(
        tables.composites()[1].slots.map(|slot| slot.word1_raw),
        [51, 61, 71, 81]
    );
}

#[test]
fn missing_references_stay_null_even_when_zero_and_maximum_hashes_exist() {
    let row = composite(1, [[0, 0, 0], [u32::MAX, 0, 1], [123, 0, 2], [123, 0, 3]]);
    let tables = ActionTables::bind(
        vec![[0, 1, 1, 1, 1], [u32::MAX, 2, 2, 2, 2]],
        vec![row],
        budget(),
    )
    .unwrap();
    assert_eq!(
        tables.composites()[0].slots.map(|slot| slot.action_index),
        [Some(0), Some(1), None, None]
    );
    let empty = ActionTables::bind(Vec::new(), vec![row], budget()).unwrap();
    assert!(
        empty.composites()[0]
            .slots
            .iter()
            .all(|slot| slot.action_index.is_none())
    );
}

#[test]
fn empty_groups_are_valid_and_budgets_reject_without_partial_results() {
    let empty = ActionTables::bind(
        Vec::new(),
        Vec::new(),
        Budget {
            actions: 0,
            composites: 0,
        },
    )
    .unwrap();
    assert!(empty.actions().is_empty() && empty.composites().is_empty());
    assert!(
        ActionTables::bind(
            vec![[0; 5]],
            Vec::new(),
            Budget {
                actions: 0,
                composites: 0
            }
        )
        .is_err()
    );
    assert!(
        ActionTables::bind(
            Vec::new(),
            vec![[0; 13]],
            Budget {
                actions: 0,
                composites: 0
            }
        )
        .is_err()
    );
}
