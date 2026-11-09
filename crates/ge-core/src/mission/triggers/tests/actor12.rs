use super::registry::{budget, common, finish, map, row};
use crate::mission::{actor_definitions as definitions, source_rng, triggers::*};
use actor12::*;

struct Providers {
    level: LevelContext,
    rng: Option<source_rng::SourceRngOwner>,
    definitions: definitions::Registry,
    calls: Vec<&'static str>,
    unavailable: Option<ProviderStage>,
    malformed: bool,
}

fn providers() -> Providers {
    let rows = [0x4d00_0110, 0x4d00_0120]
        .into_iter()
        .enumerate()
        .map(|(source_index, key)| {
            let mut words = [0; 17];
            words[0] = key;
            words[1] = 4;
            definitions::Primary {
                source_index,
                words,
            }
        })
        .collect();
    Providers {
        level: LevelContext::Selected {
            descriptor_fallback: 0x4d00_0004,
            selector_words: [11, 22, 33, 44, 55, 66],
        },
        rng: Some(source_rng::SourceRngOwner::from_constructor()),
        definitions: definitions::Registry::build(
            rows,
            definitions::Budget {
                rows: 2,
                groups: 1,
                members: 2,
            },
        )
        .unwrap(),
        calls: Vec::new(),
        unavailable: None,
        malformed: false,
    }
}

impl Factory for Providers {
    fn actor12_descriptor_fallback(&mut self, _: Request) -> anyhow::Result<ProviderOutcome<u32>> {
        self.calls.push("fallback");
        Ok(
            if self.unavailable == Some(ProviderStage::DescriptorFallback) {
                ProviderOutcome::Unavailable
            } else {
                ProviderOutcome::Ready(self.level.descriptor_fallback())
            },
        )
    }
    fn actor12_level_selector(
        &mut self,
        _: Request,
        token: u32,
    ) -> anyhow::Result<ProviderOutcome<LevelSelection>> {
        self.calls.push("selector");
        anyhow::ensure!(!self.malformed, "authored malformed selector context");
        if self.unavailable == Some(ProviderStage::LevelSelector) {
            return Ok(ProviderOutcome::Unavailable);
        }
        Ok(
            match self
                .level
                .select(token, self.rng.as_mut(), source_rng::Budget { draws: 8 })?
            {
                LevelOutcome::Selected(selection) => ProviderOutcome::Ready(selection),
                LevelOutcome::RequiresRng => ProviderOutcome::Unavailable,
            },
        )
    }
    fn actor12_definition(
        &mut self,
        _: Request,
        reference: u32,
    ) -> anyhow::Result<ProviderOutcome<definitions::RecordId>> {
        self.calls.push("definition");
        if self.unavailable == Some(ProviderStage::Definition) {
            return Ok(ProviderOutcome::Unavailable);
        }
        Ok(match self.definitions.selection_mut(reference)? {
            definitions::Selection::Selected { record, .. } => ProviderOutcome::Ready(record),
            _ => ProviderOutcome::Unavailable,
        })
    }
    fn initialize_animator(&mut self, _: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        panic!("status1 Actor12 must stop its absent animator")
    }
}

fn actor(ordinal: usize) -> RowInput {
    let mut row = row(ordinal);
    row.descriptor = Descriptor {
        native_type_index: 9,
        class: 0x1e00_0002,
        subtype: 0x1f00_0012,
        enable_byte_raw: 1,
    };
    row
}

#[test]
fn constructor_calls_real_providers_in_order_and_retains_raw_and_runtime_separately() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut providers = providers();
    let mut payload = common(0x4d00_0000);
    payload.numeric[1] = 0x5220_0001;
    payload.numeric[2] = 0x5230_0000;
    payload.numeric[3] = 0x5200_0009;
    payload.numeric[4] = 0x0b00_0002;
    payload.numeric[5] = 0x5460_0000;
    payload.numeric[6] = 0x4b30_0002;
    payload.numeric[7] = u32::MAX;
    payload.numeric[10] = 0x4200_0004;
    payload.word_74_raw = 0x01b4_0007;
    payload.word_78_raw = 0x0400_0000;
    payload.animator_present = true;
    payload.property_mask_raw |= 1 << 26;
    let raw_numeric = payload.numeric;
    registry
        .visit(actor(0), || Ok(payload), &mut providers)
        .unwrap();
    assert_eq!(providers.calls, ["fallback", "selector", "definition"]);
    assert_eq!(providers.rng.as_ref().unwrap().draws_since_snapshot(), 0);
    assert_eq!(providers.definitions.groups()[0].cursor(), 1);
    let object = &registry.objects()[0];
    let state = object.actor12().unwrap();
    assert_eq!(object.common().numeric, raw_numeric);
    assert_eq!(object.runtime_numeric()[0], 0x4d00_0004);
    assert_eq!(
        [
            object.runtime_numeric()[1],
            object.runtime_numeric()[2],
            object.runtime_numeric()[3],
            object.runtime_numeric()[4]
        ],
        [0x5220_0001, 0, 0x5200_0009, 0x0b00_0002]
    );
    assert_eq!(
        [
            object.runtime_numeric()[5],
            object.runtime_numeric()[6],
            object.runtime_numeric()[7],
            object.runtime_numeric()[10]
        ],
        [0, 22, 63, 0x4200_0004]
    );
    assert_eq!((state.word_74(), state.word_78()), (0x01b4_0007, 0));
    assert_eq!(state.radius_squared_bits(), Some(0x7f7f_ffff));
    assert_eq!(
        state.definition(),
        Some(providers.definitions.records()[0].id())
    );
    assert_eq!(state.byte_11c(), 0);
    assert_eq!(state.stage(), ConstructorStage::Complete);
    assert_eq!(object.registration_status(), Some(FactoryStatus::Status1));
    assert_eq!(object.animator(), AnimatorState::StoppedAbsent);
    finish(&mut registry);
    assert_eq!(
        registry.objects()[0]
            .actor12()
            .unwrap()
            .map_initialization(),
        MapInitialization::Complete
    );
    assert!(registry.is_complete());
    registry.rebuild_catalog().unwrap();
    assert!(registry.catalog().is_empty());
}

#[test]
fn valid_reference_skips_fallback_and_missing_provider_retains_only_reached_state() {
    for stage in [
        ProviderStage::DescriptorFallback,
        ProviderStage::LevelSelector,
        ProviderStage::Definition,
    ] {
        let mut registry = Registry::new(budget());
        registry.begin_map(map(1), 1, None).unwrap();
        let mut providers = providers();
        providers.unavailable = Some(stage);
        let mut payload = common(0);
        payload.numeric[6] = 0x4b30_0001;
        let result = registry
            .visit(actor(0), || Ok(payload), &mut providers)
            .unwrap();
        assert!(
            matches!(result, Slot::Unsupported(Unsupported::Actor12Provider { stage: actual, .. }) if actual == stage)
        );
        assert_eq!(registry.scratch().len(), 1);
        assert!(registry.membership().is_empty());
        assert!(!registry.objects()[0].is_registered());
        assert_eq!(
            registry.objects()[0].actor12().unwrap().stage(),
            ConstructorStage::Provider(stage)
        );
        finish(&mut registry);
        assert!(!registry.is_complete());
        assert_eq!(
            providers.calls.len(),
            match stage {
                ProviderStage::DescriptorFallback => 1,
                ProviderStage::LevelSelector => 2,
                ProviderStage::Definition => 3,
            }
        );
    }
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut providers = providers();
    providers.unavailable = Some(ProviderStage::DescriptorFallback);
    let mut payload = common(0x4d00_0110);
    payload.numeric[6] = 0x4b30_0001;
    registry
        .visit(actor(0), || Ok(payload), &mut providers)
        .unwrap();
    assert_eq!(providers.calls, ["selector", "definition"]);
    assert!(registry.objects()[0].is_registered());
}

#[test]
fn selector_is_lazy_bounds_context_and_preserves_shared_rng_mutation() {
    let budget = source_rng::Budget { draws: 2 };
    let mut rng = source_rng::SourceRngOwner::from_constructor();
    let absent = LevelContext::Absent
        .select(0, Some(&mut rng), budget)
        .unwrap();
    assert_eq!(
        absent,
        LevelOutcome::Selected(LevelSelection {
            word: 0x4b00_000d,
            branch: SelectionBranch::AbsentContext,
            sentinel_fallback: false
        })
    );
    let level = LevelContext::Selected {
        descriptor_fallback: 0x4d00_0007,
        selector_words: [10, u32::MAX, 30, 40, 50, 60],
    };
    assert!(matches!(
        level.select(0x4b30_0002, Some(&mut rng), budget).unwrap(),
        LevelOutcome::Selected(LevelSelection {
            word: 0x4b00_000d,
            sentinel_fallback: true,
            branch: SelectionBranch::Direct { selector: 2 }
        })
    ));
    assert_eq!(rng.draws_since_snapshot(), 0);
    assert!(level.select(0x4b30_0007, Some(&mut rng), budget).is_err());
    assert_eq!(rng.draws_since_snapshot(), 0);
    assert_eq!(
        level.select(0, None, budget).unwrap(),
        LevelOutcome::RequiresRng
    );
    assert!(matches!(
        level.select(0x4b30_0000, Some(&mut rng), budget).unwrap(),
        LevelOutcome::Selected(LevelSelection {
            word: 30,
            branch: SelectionBranch::Random { selector: 3 },
            ..
        })
    ));
    assert_eq!(rng.draws_since_snapshot(), 1);
    assert!(
        level
            .select(0, Some(&mut rng), source_rng::Budget { draws: 1 })
            .is_err()
    );
    assert_eq!(rng.draws_since_snapshot(), 1);
}

#[test]
fn provider_error_is_not_fallback_and_retains_staged_scratch_and_original_slot() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut providers = providers();
    providers.malformed = true;
    assert!(
        registry
            .visit(actor(0), || Ok(common(0)), &mut providers)
            .is_err()
    );
    assert_eq!(providers.calls, ["fallback", "selector"]);
    assert_eq!(registry.scratch().len(), 1);
    assert!(registry.membership().is_empty());
    assert!(matches!(
        registry.slots(0).unwrap()[0],
        Slot::Unsupported(Unsupported::Actor12Provider {
            stage: ProviderStage::LevelSelector,
            ..
        })
    ));
    finish(&mut registry);
    assert!(!registry.is_complete());
}

#[test]
fn registered_actor_blocks_real_map_init_on_unknown_reference_and_ignores_ref7_scan() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 2, None).unwrap();
    let mut providers = providers();
    let mut payload = common(0);
    payload.reference_ordinals[0] = 1;
    registry
        .visit(actor(0), || Ok(payload), &mut providers)
        .unwrap();
    registry
        .mark_unsupported(
            1,
            Unsupported::Unclassified {
                native_type_index: 111,
            },
        )
        .unwrap();
    finish(&mut registry);
    let object = &registry.objects()[0];
    assert!(object.is_registered());
    assert_eq!(
        object.actor12().unwrap().map_initialization(),
        MapInitialization::Blocked {
            scan: 0,
            ordinal: Some(1)
        }
    );
    assert_eq!(object.actor12().unwrap().scan_matches(), [None; 4]);
    assert!(!registry.is_complete());
    let mut other = Registry::new(budget());
    other.begin_map(map(1), 2, None).unwrap();
    let mut payload = common(0);
    payload.reference_ordinals[7] = 1;
    other
        .visit(actor(0), || Ok(payload), &mut providers)
        .unwrap();
    other
        .mark_unsupported(
            1,
            Unsupported::Unclassified {
                native_type_index: 111,
            },
        )
        .unwrap();
    finish(&mut other);
    assert_eq!(
        other.objects()[0].actor12().unwrap().map_initialization(),
        MapInitialization::Complete
    );
    assert!(
        !other.is_complete(),
        "global reference completeness still sees ref7 blocker"
    );
}

#[test]
fn actual_level_row_snapshot_uses_proven_offsets_without_revalidating_fallback() {
    use crate::mission::catalog::{Budget, InitialSelection, LevelRegistry};
    let mut words = [0; 45];
    words[0] = 7;
    words[0x84 / 4] = 0x1234;
    for i in 0..6 {
        words[0x8c / 4 + i] = i as u32 + 100;
    }
    let registry = LevelRegistry::new(
        vec![words],
        Vec::new(),
        Budget {
            primary: 1,
            secondary: 0,
        },
        InitialSelection {
            current_hash: 7,
            previous_hash: u32::MAX,
            primary_hash: Some(7),
            secondary_hash: None,
        },
    )
    .unwrap();
    let context = LevelContext::from_level_row(registry.selected_primary());
    assert_eq!(context.descriptor_fallback(), 0x1234);
    assert_eq!(
        LevelContext::from_level_row(None).descriptor_fallback(),
        0x4d00_0002
    );
    assert_eq!(
        context
            .select(0x4b30_0006, None, source_rng::Budget { draws: 0 })
            .unwrap(),
        LevelOutcome::Selected(LevelSelection {
            word: 105,
            branch: SelectionBranch::Direct { selector: 6 },
            sentinel_fallback: false
        })
    );
}

#[test]
fn missing_definition_preserves_prior_rng_draw_rotation_and_scalar_result() {
    let mut registry = Registry::new(budget());
    registry.begin_map(map(1), 1, None).unwrap();
    let mut providers = providers();
    providers.unavailable = Some(ProviderStage::Definition);
    let mut payload = common(0);
    payload.rotation_bits = [0x4049_0fdb, 0, 0x4049_0fdb, 0];
    let original = payload.rotation_bits;
    registry
        .visit(actor(0), || Ok(payload), &mut providers)
        .unwrap();
    let object = &registry.objects()[0];
    let state = object.actor12().unwrap();
    assert_eq!(providers.rng.as_ref().unwrap().draws_since_snapshot(), 1);
    assert_eq!(providers.definitions.groups()[0].cursor(), 0);
    assert_eq!(object.runtime_numeric()[6], 33);
    assert_eq!(object.common().rotation_bits, original);
    assert_eq!(object.original_rotation_bits(), original);
    assert_eq!(state.current_rotation_bits(), [0, 0x4049_0fdb, 0, 0]);
    assert!(state.rotation_repaired());
    assert_eq!(state.definition(), None);
    assert!(!object.is_registered());
    assert_eq!(
        state.stage(),
        ConstructorStage::Provider(ProviderStage::Definition)
    );
}
