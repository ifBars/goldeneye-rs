use crate::mission::{
    actor_definitions::{self, LookupBudget, Primary, Registry},
    catalog::{self, InitialSelection, LevelRegistry, LevelRow},
};

#[path = "operation.rs"]
mod operation_tests;

fn lookup_budget() -> LookupBudget {
    LookupBudget { comparisons: 16 }
}

fn row(key: u32, group: u32, source_index: usize, file: u32, first: u32, second: u32) -> Primary {
    let mut words = [0xeeee_eeee; 17];
    words[0] = key;
    words[1] = group;
    words[0x18 / 4] = file;
    words[0x1c / 4] = first;
    words[0x28 / 4] = second;
    Primary {
        source_index,
        words,
    }
}

fn registry(rows: Vec<Primary>) -> Registry {
    Registry::build(
        rows,
        actor_definitions::Budget {
            rows: 16,
            groups: 16,
            members: 16,
        },
    )
    .unwrap()
}

fn grouped_registry() -> Registry {
    registry(
        (0..4)
            .rev()
            .map(|index| {
                row(
                    0x4d00_0701 + index,
                    8,
                    80 + index as usize,
                    0x0102_0000 + index,
                    0x0d00_0301 + index,
                    0x0600_0401 + index,
                )
            })
            .collect(),
    )
}

fn level(reference: u32) -> LevelRow {
    let mut words = [0xcccc_cccc; 45];
    words[0] = 0x0181_1234;
    words[0x84 / 4] = reference;
    LevelRegistry::new(
        vec![words],
        vec![],
        catalog::Budget {
            primary: 1,
            secondary: 0,
        },
        InitialSelection {
            current_hash: u32::MAX,
            previous_hash: 0x1122_3344,
            primary_hash: None,
            secondary_hash: None,
        },
    )
    .unwrap()
    .primary()[0]
        .clone()
}

fn cursor(registry: &Registry) -> usize {
    registry
        .groups()
        .iter()
        .find(|group| group.key() == 8)
        .unwrap()
        .cursor()
}
