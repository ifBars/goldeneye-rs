use super::{Player, Sources, post_trigger_resources, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_core::{
    animation::skeleton::BoneInput,
    mission::player::{
        components::{ResourceBinding, SkeletonBudget, SkeletonInput, Stage},
        world::World,
    },
};

#[derive(Default)]
pub(in crate::game::player) struct Assets {
    pub skin_source: Option<post_trigger_resources::FileBinding>,
    pub component_source: Option<post_trigger_resources::FileBinding>,
    pub skin: Option<ge_assets::skin::Skin>,
}
pub(super) fn initialize(
    player: &mut Player,
    sources: Sources<'_>,
    world: &mut World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    ensure!(
        player.components.preparation.stage() == Stage::AwaitingFirstPerson,
        "FP live dispatch precedes TP helper return"
    );
    hook.ensure_current()?;
    let character = player
        .reset
        .character()
        .context("FP retained character is absent")?;
    let row = player.loadout.selected_character();
    ensure!(
        row.key() == character.key && row.source_index() == character.source_index,
        "FP character row differs from reset owner"
    );
    let file = row.fp_skin_file();
    let identity = sources.identities.allocate(player.base.player())?;
    player
        .components
        .preparation
        .begin_first_person(identity, character, file)?;
    // Source repeats the direct visual lookup even on flag1, then skips the
    // TP residency/postbind branch. No actor cursor or RNG selection occurs.
    let visual = sources.definitions.visual(character)?;
    player
        .components
        .preparation
        .first_person_mut()?
        .visual_lookup(visual)?;
    hook.ensure_current()?;
    let assets = &mut player.components.first_person;
    assets.skin_source = Some(sources.resources.lookup_player_file(file, sources.assets)?);
    hook.ensure_current()?;
    assets.component_source = Some(
        sources
            .resources
            .lookup_player_file(0x0102_0016, sources.assets)?,
    );
    hook.ensure_current()?;
    let skin_source = assets
        .skin_source
        .as_ref()
        .context("FP skin file is absent")?;
    let component = assets
        .component_source
        .as_ref()
        .context("FP component file is absent")?;
    assets.skin = Some(skin_source.skin(0x0d00_016f)?);
    let skin = assets.skin.as_ref().context("FP skin is absent")?;
    let auxiliary_words = skin_source.auxiliary_words(skin)?;
    let bones: Vec<_> = skin
        .bones()
        .iter()
        .map(|bone| BoneInput {
            parent: bone.parent_raw,
            range_end: bone.range_raw,
            auxiliary_raw: [bone.word_4_raw, bone.word_6_raw],
        })
        .collect();
    player
        .components
        .preparation
        .first_person_mut()?
        .bind_skeleton(
            SkeletonInput {
                primary: ResourceBinding {
                    file: skin_source.file(),
                    reference: 0x0d00_016f,
                },
                bones: &bones,
                auxiliary_words,
                captured_frame_counter: None,
            },
            ResourceBinding {
                file: component.file(),
                reference: 0,
            },
            SkeletonBudget::default(),
        )?;
    hook.finish_fp_bind(&mut player.components.preparation)?;
    hook.ensure_current()?;
    let item = player.base.item().context("FP actual item is absent")?;
    player.components.preparation.finish_first_person(
        &player.base,
        world.item_mut(item)?,
        hook.animation_callbacks()?,
        65_536,
    )?;
    hook.ensure_current()?;
    player
        .components
        .preparation
        .configure_first_person_masks(world.item(item)?)?;
    hook.ensure_current()?;
    Ok(())
}
