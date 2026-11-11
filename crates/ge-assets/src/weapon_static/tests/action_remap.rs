use super::*;

#[test]
fn tp_override_precedes_normal_and_does_not_dereference_unselected_pointer() {
    let mut fixture = Fixture::new();
    fixture.patch(0xc, TEXT);
    fixture.patch(0x14, BASE + 0x140);
    fixture.patch(0x140, GENERIC_A);
    fixture.patch(0x144, MODE_B);
    let tables = fixture.load().unwrap();
    let mut seen = Vec::new();
    let result = tables
        .map_action(
            WEAPON_A,
            GENERIC_A,
            0,
            MappingKind::ThirdPerson,
            query_budget(),
            |mode| {
                seen.push(mode);
                Ok(true)
            },
        )
        .unwrap();
    assert_eq!(seen, [MODE_B]);
    assert_eq!(result.output, MODE_B);
    assert!(result.mapped);
}
#[test]
fn tp_normal_fallback_uses_tp_parent_and_actual_component_availability() {
    let mut fixture = Fixture::new();
    fixture.patch(0x14, 0);
    fixture.patch(0x10, WEAPON_B);
    let result = fixture
        .load()
        .unwrap()
        .map_action(
            WEAPON_A,
            GENERIC_B,
            0,
            MappingKind::ThirdPerson,
            query_budget(),
            |mode| Ok(mode == MODE_B),
        )
        .unwrap();
    assert_eq!(result.output, MODE_B);
    assert_eq!(
        result
            .steps
            .iter()
            .map(|step| step.weapon)
            .collect::<Vec<_>>(),
        [WEAPON_A, WEAPON_B]
    );
}
#[test]
fn secondary_with_parent_skips_current_normal_map_after_miss() {
    let mut fixture = Fixture::new();
    fixture.patch(0x10, WEAPON_B);
    fixture.patch(0x18, BASE + 0x140); // Empty secondary map.
    // TP override remains TEXT: a reached normal lookup would fail. Parent
    // exists, so source skips that map and uses the parent's normal fallback.
    let mut seen = Vec::new();
    let result = fixture
        .load()
        .unwrap()
        .map_action(
            WEAPON_A,
            GENERIC_B,
            0,
            MappingKind::ThirdPersonSecondary,
            query_budget(),
            |mode| {
                seen.push(mode);
                Ok(true)
            },
        )
        .unwrap();
    assert_eq!(seen, [MODE_B]);
    assert_eq!(result.output, MODE_B);
    assert_eq!(result.steps.len(), 3);
}
#[test]
fn secondary_without_parent_tries_secondary_then_normal_on_same_record() {
    let mut fixture = Fixture::new();
    fixture.patch(0x10, 0);
    fixture.patch(0x14, 0);
    fixture.patch(0x18, BASE + 0x140);
    fixture.patch(0x140, GENERIC_A);
    fixture.patch(0x144, MODE_B);
    let mut seen = Vec::new();
    let result = fixture
        .load()
        .unwrap()
        .map_action(
            WEAPON_A,
            GENERIC_A,
            0,
            MappingKind::ThirdPersonSecondary,
            query_budget(),
            |mode| {
                seen.push(mode);
                Ok(mode == MODE_A)
            },
        )
        .unwrap();
    assert_eq!(seen, [MODE_B, MODE_A]);
    assert_eq!(result.output, MODE_A);
    assert_eq!(result.steps.len(), 2);
    let mut budget = query_budget();
    budget.map_rows = 1;
    assert!(
        fixture
            .load()
            .unwrap()
            .map_action(
                WEAPON_A,
                GENERIC_A,
                0,
                MappingKind::ThirdPersonSecondary,
                budget,
                |_| Ok(false)
            )
            .is_err()
    );
}
#[test]
fn secondary_acceptance_stops_before_bad_normal_and_provider_failure_does_not_fall_back() {
    let mut fixture = Fixture::new();
    fixture.patch(0x18, BASE + 0x140);
    fixture.patch(0x140, GENERIC_A);
    fixture.patch(0x144, MODE_B);
    let tables = fixture.load().unwrap();
    assert_eq!(
        tables
            .map_action(
                WEAPON_A,
                GENERIC_A,
                0,
                MappingKind::ThirdPersonSecondary,
                query_budget(),
                |_| Ok(true)
            )
            .unwrap()
            .output,
        MODE_B
    );
    assert!(
        tables
            .map_action(
                WEAPON_A,
                GENERIC_A,
                0,
                MappingKind::ThirdPersonSecondary,
                query_budget(),
                |_| anyhow::bail!("actual target provider is absent")
            )
            .is_err()
    );
    let fallback = tables
        .map_action(
            0,
            GENERIC_A,
            0,
            MappingKind::ThirdPersonSecondary,
            query_budget(),
            |_| panic!("no record has no target query"),
        )
        .unwrap();
    assert_eq!(fallback.output, GENERIC_A);
    assert!(!fallback.mapped);
}
