use super::*;
use crate::animation::{
    controls,
    nodes::{Ids, Kind, OwnerId},
    script_child,
    skeleton::LocalPose,
};
struct Body;
impl script_child::Provider for Body {
    fn file(&self) -> u32 {
        0x0102_00ab
    }
    fn generation(&mut self) -> Result<u32> {
        Ok(0)
    }
    fn resolve_script(&mut self, _: u32) -> Result<Option<script_child::Binding>> {
        Ok(Some(script_child::Binding {
            directory_index: 2,
            hash: 0x0400_00ab,
        }))
    }
    fn bind_body(&mut self, _: script_child::Binding) -> Result<bool> {
        Ok(false)
    }
}
struct Runtime {
    command: Pending,
    source: Fixture,
    layers: State,
    ids: Ids,
    controls: controls::Store,
    scripts: script_child::Store,
}
fn runtime(flags: u16, children: i16) -> Runtime {
    let owner = OwnerId::new(4).unwrap();
    let mut ids = Ids::from_constructor(owner, 8);
    let mut controls = controls::Store::from_constructor(owner, controls::Budget::default());
    let first = controls.allocate(&mut ids).unwrap();
    controls
        .capture(
            first,
            vec![LocalPose {
                translation_bits: [1f32.to_bits(); 4],
                quaternion_bits: [0, 0, 0, 1f32.to_bits()],
            }],
            vec![0x1234],
        )
        .unwrap();
    let mut layers = layers();
    layers
        .capture_fresh_slots(&mut controls, &mut ids, first, 15)
        .unwrap();
    let mut source = fixture();
    source.flags = flags;
    source.rows[0].slot = 2;
    source.rows[0].weight = 0;
    source.rows[0].children = children;
    let mut command = Pending::new(0x0c00_0002);
    command
        .prepare(&mut source, &mut layers, no_override())
        .unwrap();
    Runtime {
        command,
        source,
        layers,
        ids,
        controls,
        scripts: script_child::Store::from_constructor(owner, 8),
    }
}
fn allocate(r: &mut Runtime) -> super::super::super::NodeId {
    let request = r.command.begin_factory(&mut r.layers).unwrap();
    let id = r
        .scripts
        .allocate(&mut r.ids, 0x0102_00ab, request.reference)
        .unwrap();
    r.command.retain_factory_node(&mut r.layers, id).unwrap();
    r.scripts
        .bind_factory_context(id, request.slot, request.effective_flags)
        .unwrap();
    r.scripts.initialize_prefix(id, &mut Body).unwrap();
    id
}
fn finish(r: &mut Runtime) -> Result<()> {
    r.command.finish_script_child(
        &mut r.layers,
        &mut r.ids,
        &r.controls,
        &mut r.scripts,
        &mut r.source,
    )
}
pub(super) fn returned_body() -> Pending {
    let mut r = runtime(0x24, 1);
    allocate(&mut r);
    finish(&mut r).unwrap();
    r.command
        .advance_completed_child(&mut r.source, &mut r.layers, no_override())
        .unwrap();
    assert_eq!(r.command.stage(), Stage::Returned);
    r.command
}
#[test]
fn copy_uses_old_header_position_then_real_links_scalars_and_opaque_post_word() {
    let mut r = runtime(0x24, 1);
    let old = r.layers.slots()[2].nodes_head()[0];
    let id = allocate(&mut r);
    assert_eq!(
        r.scripts.get(id).unwrap().word(0x84),
        Some((-1f32).to_bits())
    );
    finish(&mut r).unwrap();
    let h = r.scripts.get(id).unwrap();
    assert_eq!(h.word(0x80), Some(0));
    assert_eq!(h.word(0x84), Some(0));
    assert_eq!(r.layers.slots()[2].nodes_head(), [id, old]);
    assert_eq!(r.layers.active_count(), 3);
    assert_eq!(r.ids.links(id).unwrap().next, Some(old));
    assert_eq!(r.ids.links(old).unwrap().previous, Some(id));
    assert_eq!(r.ids.kind(id), Some(Kind::Script));
    assert_eq!(h.word(0x34), Some(0));
    assert_eq!(h.word(0x3c), Some((1f32 / 3f32).to_bits()));
    assert_eq!(h.word(0x24), Some(0x7fc0_1234));
    assert_eq!(r.command.effects()[0].stage, EffectStage::Complete);
    r.command
        .advance_completed_child(&mut r.source, &mut r.layers, no_override())
        .unwrap();
    assert_eq!(r.command.stage(), Stage::Returned);
    assert_eq!(
        r.source.calls.iter().filter(|c| **c == "reference").count(),
        1
    );
    assert_eq!(r.source.calls.iter().filter(|c| **c == "weight").count(), 1);
}
#[test]
fn post_word_failure_keeps_inserted_child_active_count_and_scalars_without_replay() {
    let mut r = runtime(0x24, 1);
    let id = allocate(&mut r);
    r.source.fail = Some("post_word");
    assert!(finish(&mut r).is_err());
    assert_eq!(r.command.stage(), Stage::Failed);
    assert!(r.layers.failed());
    assert_eq!(r.layers.active_count(), 3);
    assert_eq!(r.layers.slots()[2].nodes_head()[0], id);
    assert_eq!(r.scripts.get(id).unwrap().word(0x34), Some(0));
    assert!(r.command.effects()[0].inserted);
    assert_eq!(r.command.effects()[0].stage, EffectStage::Scalars);
    assert_eq!(r.command.effects()[0].post_word, None);
    let calls = r.source.calls.len();
    assert!(finish(&mut r).is_err());
    assert_eq!(r.source.calls.len(), calls);
}
#[test]
fn wrapper_dependency_stops_after_phase_copy_before_membership_and_post_word() {
    let mut r = runtime(0x2c, 1);
    let id = allocate(&mut r);
    finish(&mut r).unwrap();
    assert_eq!(r.command.stage(), Stage::ChildWrapperPending);
    assert_eq!(r.layers.active_count(), 1);
    assert!(r.ids.links(id).is_none());
    assert_eq!(r.scripts.get(id).unwrap().word(0x84), Some(0));
    assert!(!r.source.calls.contains(&"post_word"));
}
#[test]
fn next_child_reuses_group_weight_and_source_reference_instead_of_replaying_the_prefix() {
    let mut r = runtime(0x24, 2);
    let first = allocate(&mut r);
    finish(&mut r).unwrap();
    r.command
        .advance_completed_child(&mut r.source, &mut r.layers, no_override())
        .unwrap();
    assert_eq!(r.command.child().unwrap().ordinal, 1);
    assert_eq!(r.command.factory_node(), None);
    let next = allocate(&mut r);
    finish(&mut r).unwrap();
    assert_eq!(r.ids.links(first).unwrap().previous, Some(next));
    assert_eq!(r.command.effects().len(), 2);
    assert_eq!(r.source.calls.iter().filter(|c| **c == "weight").count(), 1);
    r.command
        .advance_completed_child(&mut r.source, &mut r.layers, no_override())
        .unwrap();
    assert_eq!(r.command.stage(), Stage::Returned);
}
#[test]
fn unskipped_cleanup_and_later_count_failure_are_retained_after_actual_child_completion() {
    let mut r = runtime(4, 1);
    allocate(&mut r);
    finish(&mut r).unwrap();
    r.command
        .advance_completed_child(&mut r.source, &mut r.layers, no_override())
        .unwrap();
    assert_eq!(r.command.stage(), Stage::CleanupPending);
    let mut r = runtime(0x24, 1);
    let id = allocate(&mut r);
    finish(&mut r).unwrap();
    r.source.fail = Some("count");
    assert!(
        r.command
            .advance_completed_child(&mut r.source, &mut r.layers, no_override())
            .is_err()
    );
    assert_eq!(r.command.last_completed().unwrap().node, id);
    assert_eq!(r.command.stage(), Stage::Failed);
    assert!(r.ids.links(id).is_some());
}
