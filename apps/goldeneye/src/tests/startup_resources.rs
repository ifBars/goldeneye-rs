use super::*;

#[test]
fn ordered_raw_rows_keep_duplicates_zero_and_missing_bindings() {
    let actions = vec![
        [0, 1, 2, 3, u32::MAX],
        [7, 8, 9, 10, 11],
        [7, 90, 91, 92, 93],
    ];
    let composite = [
        99,
        7,
        100,
        0x1234,
        8,
        u32::MAX,
        0xabcd,
        0,
        7,
        u32::MAX,
        7,
        0,
        0x100,
    ];
    let tables = bind_rows(actions.clone(), vec![composite]).unwrap();
    assert_eq!(
        tables
            .actions()
            .iter()
            .map(|row| row.words_raw)
            .collect::<Vec<_>>(),
        actions
    );
    assert_eq!(tables.composites()[0].words_raw, composite);
    let slots = &tables.composites()[0].slots;
    assert_eq!(
        slots.map(|slot| slot.action_index),
        [Some(1), None, Some(0), Some(1)]
    );
    assert_eq!(slots[0].low_byte(), 0x34);
    assert_eq!(slots[1].word1_raw, u32::MAX);
    assert_eq!(slots[2].word2_raw, u32::MAX);
    assert_eq!(counts(&tables), (3, 1, 3, 1));
}

#[test]
fn initializer_does_no_io_before_legal_fade_completion() {
    let mut resources = StartupResources::new(PathBuf::from("authored-nonexistent-image.rvz"));
    for stage in [
        Stage::Strap,
        Stage::Zapper,
        Stage::Activision,
        Stage::Eurocom,
        Stage::Mgm,
        Stage::Title,
        Stage::MainMenu,
    ] {
        resources.tick(stage, false).unwrap();
    }
    resources.tick(Stage::Legal, true).unwrap();
    assert_eq!(resources.evidence().state, State::NotStarted);
    assert!(!resources.actions_ready());
    assert!(resources.worker.is_none());
}

#[test]
fn disconnected_worker_fails_without_retrying_or_marking_ready() {
    let mut resources = StartupResources::new(PathBuf::new());
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(sender);
    resources.response = Some(receiver);
    resources.state = State::Loading;
    assert!(resources.tick(Stage::Legal, false).is_err());
    assert!(!resources.actions_ready());
    assert_eq!(resources.evidence().state, State::Failed);
    assert!(resources.tick(Stage::Legal, false).is_err());
    assert!(resources.worker.is_none());
}

#[test]
fn worker_error_and_pending_work_cannot_be_reported_ready() {
    let mut resources = StartupResources::new(PathBuf::new());
    let (sender, receiver) = mpsc::sync_channel(1);
    resources.response = Some(receiver);
    resources.state = State::Loading;
    resources.tick(Stage::Legal, false).unwrap();
    assert_eq!(resources.evidence().state, State::Loading);
    assert!(!resources.actions_ready());
    sender
        .send(Err(anyhow::anyhow!("authored read failure")))
        .unwrap();
    assert!(resources.tick(Stage::Title, false).is_err());
    assert_eq!(resources.evidence().state, State::Failed);
    assert!(!resources.actions_ready());
    assert!(
        resources
            .evidence()
            .error
            .unwrap()
            .contains("authored read failure")
    );
}

#[test]
fn successful_channel_retains_tables_and_serializes_only_metadata() {
    let mut resources = StartupResources::new(PathBuf::new());
    let tables = bind_rows(
        vec![[7, 1, 2, 3, 4]],
        vec![[0, 7, 0, 0, 8, 0, 0, 9, 0, 0, 10, 0, 0]],
    )
    .unwrap();
    let (sender, receiver) = mpsc::sync_channel(1);
    sender.send(Ok(tables)).unwrap();
    resources.response = Some(receiver);
    resources.state = State::Loading;
    resources.tick(Stage::Legal, false).unwrap();
    assert!(resources.actions_ready());
    assert_eq!(
        resources.tables.as_ref().unwrap().actions()[0].words_raw,
        [7, 1, 2, 3, 4]
    );
    let value = serde_json::to_value(resources.evidence()).unwrap();
    assert_eq!(value["state"], "initialized");
    assert_eq!(value["resolved_slots"], 1);
    assert_eq!(value["unresolved_slots"], 3);
    assert!(value.get("tables").is_none());
    resources.tick(Stage::Legal, false).unwrap();
    assert!(resources.worker.is_none());
}

#[test]
fn binding_rejects_host_budget_exhaustion() {
    assert!(bind_rows(vec![[0; 5]; BUDGET.actions + 1], vec![]).is_err());
    assert!(bind_rows(vec![], vec![[0; 13]; BUDGET.composites + 1]).is_err());
}
