use super::*;
use crate::mission::player::{
    identity::{ComponentId, PhysicsId, WorldId},
    item_registry,
    world::{self, World},
};

struct NoComponents;
impl world::Components for NoComponents {
    fn activate(&mut self, _: ItemId, _: ComponentId, _: bool) -> Result<()> {
        anyhow::bail!("authored fresh item has no components")
    }
}

fn fixture(
    activate: bool,
) -> (
    Owner,
    World,
    item_registry::Registry,
    physics::Owner,
    ItemId,
) {
    // Authored entry snapshot; the real pure factory/list/motion/physics paths
    // below produce the suffix prerequisites rather than supplied ready flags.
    let mut owner = Owner::new(PlayerId::new(7).unwrap());
    owner.stage = Stage::ConstructorFlagComplete;
    let mut world = World::from_constructor(
        WorldId::new(21).unwrap(),
        1,
        10,
        world::Budget {
            items: 4,
            memberships: 10,
        },
    )
    .unwrap();
    let id = world.construct_item().unwrap();
    let mut registry = item_registry::Registry::from_constructor();
    world
        .item_mut(id)
        .unwrap()
        .begin_wrapperless_factory()
        .unwrap();
    let receipt = registry
        .register(
            id,
            item_registry::Budget {
                records: 16,
                native_bytes: 128,
                growth_steps: 2,
            },
        )
        .unwrap();
    let item = world.item_mut(id).unwrap();
    item.finish_wrapperless_factory(&receipt).unwrap();
    item.prepare_player_owner();
    item.bind_handler(Some(owner.player()));
    owner.bind_item(item).unwrap();
    world
        .register_if_absent(id, 0x2d, &mut NoComponents)
        .unwrap();
    owner
        .initialize_virtual_motion(world.item(id).unwrap())
        .unwrap();
    let mut physics = physics::Owner::from_constructor();
    physics.bind_item_identity(id).unwrap();
    world
        .item_mut(id)
        .unwrap()
        .bind_physics(Some(PhysicsId::new(7).unwrap()));
    if activate {
        let mask = world.item(id).unwrap().membership_mask();
        physics
            .apply_player_initialization(None, || {
                world.update_membership(id, mask | 0x10)?;
                Ok(())
            })
            .unwrap();
    }
    owner.w(0x574, 91);
    owner.b(0x618, 53);
    owner.b(0x619, 0xab);
    (owner, world, registry, physics, id)
}

struct Sessions {
    present: bool,
    member_states: Vec<u32>,
    fallback_raw: Option<u32>,
    fail: Option<&'static str>,
    calls: Vec<&'static str>,
}
impl Sessions {
    fn active(states: &[u32]) -> Self {
        Self {
            present: true,
            member_states: states.to_vec(),
            fallback_raw: None,
            fail: None,
            calls: Vec::new(),
        }
    }
}
impl VirtualSessionProvider for Sessions {
    fn session_present(&mut self) -> Result<bool> {
        self.calls.push("session");
        ensure!(
            self.fail != Some("session"),
            "authored session lookup failure"
        );
        Ok(self.present)
    }
    fn session_user_count(&mut self) -> Result<u32> {
        self.calls.push("count");
        ensure!(
            self.fail != Some("count"),
            "actual qualifying-member count unavailable"
        );
        Ok(self.fallback_raw.unwrap_or_else(|| {
            self.member_states
                .iter()
                .filter(|&&state| state == 2)
                .count() as u32
        }))
    }
}
fn state(owner: &Owner) -> Vec<u32> {
    let mut words = Vec::new();
    owner.visit_state_words(|word| words.push(word));
    words
}

#[test]
fn activated_physics_with_unknown_scalar_allows_only_the_independent_basic_return() {
    let (mut owner, mut world, _registry, physics, id) = fixture(true);
    assert_eq!(physics.word(0x168), None);
    let mut sessions = Sessions::active(&[0, 2, 1]);
    let receipt = owner
        .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
        .unwrap();
    assert_eq!(receipt.player(), owner.player());
    assert_eq!(receipt.item(), id);
    assert_eq!(owner.stage(), Stage::VirtualBasicComplete);
    assert_eq!(owner.word(0x574), Some(0));
    assert_eq!(owner.byte(0x618), Some(1));
    assert_eq!(owner.byte(0x619), Some(0xab));
    assert_eq!(owner.word(0x618), None); // source writes only one byte here
    assert_eq!(owner.word(0x224), None);
    assert_eq!(physics.word(0x168), None); // never read or replaced by this suffix
    assert_eq!(world.item(id).unwrap().flags(), 0x2b);
    assert_eq!(sessions.calls, ["session", "count"]);
    let before = state(&owner);
    assert!(
        owner
            .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
            .is_err()
    );
    assert_eq!(state(&owner), before);
    assert_eq!(sessions.calls, ["session", "count"]);
}

#[test]
fn only_actual_qualifying_members_above_one_add_the_owner_flag_without_changing_lists() {
    for (records, multiple) in [
        (vec![0, 1, 3], false),
        (vec![2, 4], false),
        (vec![2, 0, 2], true),
    ] {
        let (mut owner, mut world, _registry, physics, id) = fixture(true);
        let mask = world.item(id).unwrap().membership_mask();
        let mut sessions = Sessions::active(&records);
        owner
            .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
            .unwrap();
        assert_eq!(
            world.item(id).unwrap().flags(),
            if multiple { 0x22b } else { 0x2b }
        );
        assert_eq!(world.item(id).unwrap().membership_mask(), mask);
        assert_eq!(world.actors().unwrap(), [id]);
    }
}

#[test]
fn known_absent_session_skips_count_instead_of_manufacturing_zero() {
    let (mut owner, mut world, _registry, physics, id) = fixture(true);
    let mut sessions = Sessions::active(&[2, 2]);
    sessions.present = false;
    sessions.fail = Some("count");
    owner
        .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
        .unwrap();
    assert_eq!(sessions.calls, ["session"]);
    assert_eq!(world.item(id).unwrap().flags(), 0x2b);
    assert_eq!(owner.byte(0x618), Some(1));
}

#[test]
fn unavailable_session_or_member_count_keeps_574_store_and_skips_late_618_with_no_replay() {
    for fail in ["session", "count"] {
        let (mut owner, mut world, _registry, physics, id) = fixture(true);
        let mut sessions = Sessions::active(&[2, 2]);
        sessions.fail = Some(fail);
        assert!(
            owner
                .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
                .is_err()
        );
        assert_eq!(owner.stage(), Stage::VirtualBasicSuffix);
        assert_eq!(owner.failure(), Some(Failure::VirtualSession));
        assert_eq!(owner.word(0x574), Some(0));
        assert_eq!(owner.byte(0x618), Some(53));
        assert_eq!(world.item(id).unwrap().flags(), 0x2b);
        let calls = sessions.calls.clone();
        assert!(
            owner
                .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
                .is_err()
        );
        assert_eq!(sessions.calls, calls);
    }
}

#[test]
fn inactive_missing_or_ordinal_only_physics_and_wrong_handler_fail_before_any_suffix_store() {
    for case in 0..6 {
        let (mut owner, mut world, _registry, mut physics, id) = fixture(case != 0 && case != 5);
        match case {
            1 => {
                physics.bind_item(id.get()).unwrap();
            }
            2 => {
                world
                    .item_mut(id)
                    .unwrap()
                    .bind_handler(Some(PlayerId::new(8).unwrap()));
            }
            3 => {
                world.item_mut(id).unwrap().bind_physics(None);
            }
            4 => owner.stage = Stage::VirtualItemBound,
            5 => {
                // Reached capsule/scalar writes do not imply successful
                // activation when its real membership provider fails.
                assert!(
                    physics
                        .apply_player_initialization(None, || anyhow::bail!(
                            "authored membership failure"
                        ))
                        .is_err()
                );
            }
            _ => {}
        }
        let before = state(&owner);
        let mut sessions = Sessions::active(&[2]);
        assert!(
            owner
                .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
                .is_err()
        );
        assert_eq!(state(&owner), before);
        assert!(sessions.calls.is_empty());
    }
}

#[test]
fn signed_count_comparison_and_cross_world_typed_identity_are_preserved() {
    for raw in [u32::MAX, 0x8000_0000] {
        let (mut owner, mut world, _registry, physics, id) = fixture(true);
        let mut sessions = Sessions::active(&[]);
        sessions.fallback_raw = Some(raw); // authored nonactive-descriptor+34 result
        owner
            .finish_virtual_basic(world.item_mut(id).unwrap(), &physics, &mut sessions)
            .unwrap();
        assert_eq!(world.item(id).unwrap().flags(), 0x2b);
    }
    let (mut owner, mut world, _registry, _physics, id) = fixture(true);
    let mut other = World::from_constructor(
        WorldId::new(22).unwrap(),
        1,
        10,
        world::Budget {
            items: 1,
            memberships: 10,
        },
    )
    .unwrap();
    let foreign = other.construct_item().unwrap();
    assert_eq!(foreign.get(), id.get());
    let mut physics = physics::Owner::from_constructor();
    physics.bind_item_identity(foreign).unwrap();
    physics
        .apply_player_initialization(None, || {
            other.update_membership(foreign, 0x10)?;
            Ok(())
        })
        .unwrap();
    let before = state(&owner);
    assert!(
        owner
            .finish_virtual_basic(
                world.item_mut(id).unwrap(),
                &physics,
                &mut Sessions::active(&[2])
            )
            .is_err()
    );
    assert_eq!(state(&owner), before);
}
