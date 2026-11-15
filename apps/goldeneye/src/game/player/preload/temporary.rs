//! Direct preload factory on a distinct owner; no live FP list is reused.
use super::*;
use ge_core::{
    animation::{script_child, skeleton::BoneInput},
    mission::player::components::{Registry, SkeletonBudget, SkeletonInput, temporary::Pending},
};

pub(super) fn initialize(
    player: &mut Player,
    identities: &mut Registry,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let owned = player.preload.as_mut().context("preload owner absent")?;
    if owned.state.stage() != preload::Stage::AwaitingTemporary {
        return Ok(());
    }
    let call = owned
        .state
        .current_call()
        .context("preload current call absent")?;
    ensure!(
        call.extraction_returned() && owned.temporary.is_none(),
        "temporary entry precedes extraction or replays"
    );
    let fp = player
        .components
        .preparation
        .mode_animator(call.request().receiver)?;
    owned.temporary = Some(Pending::begin(
        fp,
        identities,
        call.reference().unwrap_or(0),
    )?);
    let temporary = owned.temporary.as_mut().expect("retained temporary");
    if temporary.skin().reference >> 24 != 0x0d {
        // A different class needs its real remapping provider before a skin
        // directory lookup. Keep the new owner at that dependency.
        return Ok(());
    }
    // The source supplies the CURRENT skeletal secondary pair. Read the skin
    // from its actual retained file rather than assuming FP's primary binding.
    let skin_source = player
        .components
        .first_person
        .skin_source
        .as_ref()
        .context("temporary secondary skin file absent")?;
    ensure!(
        skin_source.file() == temporary.skin().file,
        "temporary secondary skin wrapper differs from retained FP file"
    );
    let skin = skin_source.skin(temporary.skin().reference)?;
    hook.ensure_current()?;
    let auxiliary_words = skin_source.auxiliary_words(&skin)?;
    let bones: Vec<_> = skin
        .bones()
        .iter()
        .map(|bone| BoneInput {
            parent: bone.parent_raw,
            range_end: bone.range_raw,
            auxiliary_raw: [bone.word_4_raw, bone.word_6_raw],
        })
        .collect();
    temporary.bind_skeleton(
        SkeletonInput {
            primary: temporary.skin(),
            bones: &bones,
            auxiliary_words,
            captured_frame_counter: None,
        },
        SkeletonBudget::default(),
    )?;
    hook.ensure_current()?;
    let source = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("temporary component file absent")?;
    ensure!(
        source.file() == temporary.component().file,
        "temporary component wrapper differs from actual FP wrapper"
    );
    let mut script = source.shared_script_source();
    temporary.initialize_factory(&mut script)?;
    hook.ensure_current()?;
    if temporary
        .factory_header()
        .is_some_and(|h| h.stage() == script_child::Stage::AwaitingInitializer)
    {
        let binding = temporary
            .factory_header()
            .and_then(|h| h.cached())
            .context("temporary factory script binding absent")?;
        temporary.initialize_phase_zero(
            &mut source.phase_zero_source(binding)?,
            hook.animation_step()?,
            4096,
        )?;
        hook.ensure_current()?;
    }
    if temporary
        .factory_header()
        .is_some_and(|h| h.stage() == script_child::Stage::AwaitingAnimation)
    {
        let binding = temporary
            .factory_header()
            .and_then(|h| h.cached())
            .context("temporary position script binding absent")?;
        temporary.prepare_position(&mut source.script_position_source(binding)?)?;
        hook.ensure_current()?;
    }
    if temporary.stage()
        == ge_core::mission::player::components::temporary::Stage::AwaitingInsertion
    {
        temporary.finish_factory()?;
    }
    if temporary.stage()
        == ge_core::mission::player::components::temporary::Stage::AwaitingForwardTick
    {
        temporary.forward_zero(&mut script)?;
        hook.ensure_current()?;
        if temporary.stage()
            == ge_core::mission::player::components::temporary::Stage::AwaitingEvaluation
        {
            hook.finish_preload_bind(temporary)?;
        }
    }
    hook.ensure_current()?;
    Ok(())
}
