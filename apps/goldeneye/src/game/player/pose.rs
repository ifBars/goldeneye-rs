//! Actual pose-helper providers on the retained player/session worker. This
//! changes source-owned components and Item state without publishing a player.
use super::{Owned as Player, loadout, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::mission::{
    player::{
        constructor::{
            CharacterBinding, continuation,
            pose::{self, CharacterField, Source},
        },
        identity::PlayerId,
        world::World,
    },
    session::{Session, UserId, local_users::Lookup},
};

pub(super) struct Owned {
    pub state: pose::Pending,
    pub sessionlookups: Vec<Lookup>,
}

pub(super) struct Sources<'a> {
    pub loadout: &'a loadout::Sources,
    pub session: &'a Session,
    pub sessionlookups: &'a mut Vec<Lookup>,
    pub hook: &'a world_hooks::Attempt,
}
impl Source for Sources<'_> {
    fn character_scalar(
        &mut self,
        binding: CharacterBinding,
        field: CharacterField,
    ) -> Result<u32> {
        self.hook.ensure_current()?;
        let row = self.loadout.selected_character();
        ensure!(
            binding.registry_resource == ge_assets::character::CATALOG_RESOURCE
                && binding.key == row.key()
                && binding.source_index == row.source_index(),
            "pose character binding differs from actual retained row"
        );
        Ok(row.words_raw()[match field {
            CharacterField::At10 => 4,
            CharacterField::At14 => 5,
        }])
    }
    fn multiple_local_users_in_mode_three(&mut self) -> Result<bool> {
        self.hook.ensure_current()?;
        ensure!(
            self.session.stage() == ge_core::mission::session::Stage::Active
                && self.session.active() == Some(self.session.identity()),
            "pose active session unavailable"
        );
        Ok(self.session.multiple_local_users_in_mode_three())
    }
    fn ones_vector(&mut self) -> Result<[u32; 3]> {
        self.hook.player_pose_ones_bits()
    }
    fn local_user_for_player(&mut self, target: PlayerId) -> Result<Option<UserId>> {
        self.hook.ensure_current()?;
        self.sessionlookups.try_reserve(1)?;
        let lookup = self.session.lookup_local_user_for_player(target)?;
        let matched = lookup.matched;
        self.sessionlookups.push(lookup);
        Ok(matched)
    }
    fn weapon_global_byte(&mut self) -> Result<Option<u8>> {
        self.hook.player_pose_weapon_byte()
    }
}

pub(super) fn initialize(
    player: &mut Player,
    session: &Session,
    world: &mut World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let Some(tail) = player
        .constructor_tail
        .as_ref()
        .filter(|tail| tail.stage() == continuation::Stage::AwaitingPose)
    else {
        return Ok(());
    };
    ensure!(
        session.identity() == player.request.session()
            && session
                .managers()
                .is_some_and(|m| m.user.identity == player.request.user())
            && player.pose_setup.is_none(),
        "pose session/user differs or entry replays"
    );
    let (repeat, constructor) = super::evidence::capture_constructor_entry(player)?;
    player.completed_repeat_evidence = Some(repeat);
    player.completed_constructor_evidence = Some(constructor);
    player.pose_setup = Some(Owned {
        state: pose::Pending::begin(tail)?,
        sessionlookups: Vec::new(),
    });
    let owner = player.pose_setup.as_mut().expect("retained pose helper");
    let item = world.item_mut(player.base.item().context("pose item absent")?)?;
    let mut source = Sources {
        loadout: &player.loadout,
        session,
        sessionlookups: &mut owner.sessionlookups,
        hook,
    };
    owner.state.advance(
        &mut player.base,
        &mut player.reset,
        &mut player.components.preparation,
        item,
        &mut source,
    )?;
    hook.ensure_current()?;
    Ok(())
}
