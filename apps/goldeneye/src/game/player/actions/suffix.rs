use super::*;
use crate::game::world::post_trigger_resources::FileBinding;
use ge_core::mission::player::{
    actions::suffix::{MaskLookup, MaskSource, SuffixStage},
    components::Identity,
};

struct Sources {
    fp: (Identity, FileBinding),
    tp: (Identity, FileBinding),
}
impl MaskSource for Sources {
    fn mask(&mut self, receiver: Identity, reference: u32) -> Result<MaskLookup> {
        let (identity, source) = [&self.fp, &self.tp]
            .into_iter()
            .find(|(identity, _)| *identity == receiver)
            .context("suffix mask wrapper owner absent")?;
        ensure!(
            identity.player() == receiver.player(),
            "suffix wrapper player differs"
        );
        source.animation_bone_mask(reference)
    }
}

pub(super) fn initialize(
    player: &mut Player,
    world: &World,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    hook.ensure_current()?;
    let preparation = &mut player.components.preparation;
    let fp = preparation
        .first_person()
        .context("suffix FP owner absent")?
        .animator();
    let tp = preparation.animator().context("suffix TP owner absent")?;
    let fp_source = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("suffix FP wrapper absent")?;
    let tp_source = player
        .components
        .component_source
        .as_ref()
        .context("suffix TP wrapper absent")?;
    for (owner, source) in [(fp, fp_source), (tp, tp_source)] {
        ensure!(
            owner
                .component()
                .is_some_and(|binding| binding.file == source.file()),
            "suffix component/wrapper file differs"
        );
    }
    let mut sources = Sources {
        fp: (fp.identity(), fp_source.shared_script_source()),
        tp: (tp.identity(), tp_source.shared_script_source()),
    };
    let action = &mut player
        .action
        .as_mut()
        .context("suffix action owner absent")?
        .state;
    action.begin_action_suffix(&player.reset)?;
    action.advance_action_masks(
        preparation,
        world.item(player.base.item().context("suffix item absent")?)?,
        &mut sources,
    )?;
    hook.ensure_current()?;
    if action
        .action_suffix()
        .is_some_and(|s| s.stage() == SuffixStage::FirstPersonParameters)
    {
        action.finish_action_parameters(
            &player.reset,
            preparation,
            world.item(player.base.item().context("suffix parameter item absent")?)?,
        )?;
        hook.ensure_current()?;
    }
    if action.stage() == actions::Stage::Returned {
        action.prepare_repeated_action(&player.reset)?;
        hook.ensure_current()?;
    }
    Ok(())
}
