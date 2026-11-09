use crate::mission::{
    Difficulty,
    loader::phase::{AttemptId, PhaseState},
    session::{LoadBinding, NewGameRequest, OperationResult, Session, SessionId, UserId},
    triggers::*,
    world::InstanceId,
};

pub(super) fn admitted() -> Session {
    let id = SessionId::new(37).unwrap();
    let mut session = Session::begin(
        NewGameRequest::new(Difficulty::Agent),
        id,
        UserId::new(94).unwrap(),
    );
    session.mode_result(OperationResult::Succeeded).unwrap();
    session.begin_create().unwrap();
    session.create_result(OperationResult::Succeeded).unwrap();
    session.select_pending(id).unwrap();
    session
        .admission_result(OperationResult::Succeeded)
        .unwrap();
    session
}

pub(super) fn ready() -> (Session, LoadBinding) {
    let mut session = admitted();
    let attempt = AttemptId::new(7).unwrap();
    let binding = session.bind_loader(attempt).unwrap();
    // Authored fixture drives the real identity/event application. This explicit
    // phase snapshot is not an app claim that source loader providers ran.
    session
        .finish_tutorial_phase19(&binding, &mut PhaseState::new(attempt, 19).unwrap())
        .unwrap();
    (session, binding)
}

struct FactoryProbe;
impl Factory for FactoryProbe {
    fn initialize_animator(&mut self, request: AnimatorRequest) -> anyhow::Result<AnimatorId> {
        AnimatorId::new(request.trigger.get())
    }
}

pub(super) fn registry(ids: &[u32], rebuild: bool) -> Registry {
    let mut registry = Registry::new(Budget {
        maps: 1,
        rows: ids.len(),
        objects: ids.len(),
        filter_words: 0,
        fixups: ids.len() * 8,
        catalog_entries: ids.len(),
    });
    registry
        .begin_map(
            MapContext {
                instance: InstanceId::new(29).unwrap(),
                map_selector: 3,
            },
            ids.len(),
            None,
        )
        .unwrap();
    for (ordinal, &id) in ids.iter().enumerate() {
        let mut numeric = [0; 16];
        numeric[0] = id;
        registry
            .visit(
                RowInput {
                    ordinal,
                    zone: 4,
                    descriptor: Descriptor {
                        native_type_index: 28,
                        class: 0x1e00_0003,
                        subtype: 0x1f00_0081,
                        enable_byte_raw: 1,
                    },
                },
                || {
                    Ok(CommonInput {
                        header_flags_raw: 0,
                        property_mask_raw: 1,
                        position_bits: [0x3f80_0000 + ordinal as u32, 0x8000_0000, 7, 0],
                        rotation_bits: [9, 0xbe80_0000, 11, 0],
                        numeric,
                        reference_ordinals: [u32::MAX; 8],
                        word_74_raw: 0,
                        word_78_raw: 0,
                        animator_present: false,
                        animator_index_raw: 0,
                        shape_index_raw: None,
                    })
                },
                &mut FactoryProbe,
            )
            .unwrap();
    }
    registry.fix_references().unwrap();
    registry.link_parents().unwrap();
    registry.initialize_map().unwrap();
    if rebuild {
        registry.rebuild_catalog().unwrap();
    }
    registry
}
