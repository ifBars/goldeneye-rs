use super::{Owned, post_trigger_resources, world_hooks};
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::{
    animation::skeleton::BoneInput,
    mission::player::components::{ResourceBinding, SkeletonBudget, SkeletonInput, Stage},
};

const DEFAULT_FILE: u32 = 0x010a_0056;
const DEFAULT_SKIN: u32 = 0x0d00_0001;
const TP_FILE: u32 = 0x0102_0017;

pub(super) fn bind(
    owner: &mut Owned,
    resources: &mut post_trigger_resources::Owned,
    assets: &mut GameAssets,
    hook: &world_hooks::Attempt,
) -> Result<()> {
    ensure!(
        owner.preparation.stage() == Stage::AwaitingSkeleton,
        "TP default skeleton lookup is out of order"
    );
    hook.ensure_current()?;
    // Source looks up both wrappers before entering the animator bind. Retain
    // each actual owner before the next cancellation check or asset read.
    owner.skeleton_source = Some(resources.lookup_player_file(DEFAULT_FILE, assets)?);
    hook.ensure_current()?;
    owner.component_source = Some(resources.lookup_player_file(TP_FILE, assets)?);
    hook.ensure_current()?;
    let source = owner
        .skeleton_source
        .as_ref()
        .context("default skeleton file is absent")?;
    let component = owner
        .component_source
        .as_ref()
        .context("TP component file is absent")?;
    owner.skin = Some(source.skin(DEFAULT_SKIN)?);
    let skin = owner
        .skin
        .as_ref()
        .context("default skin owner is absent")?;
    let auxiliary_words = source.auxiliary_words(skin)?;
    let bones: Vec<_> = skin
        .bones()
        .iter()
        .map(|bone| BoneInput {
            parent: bone.parent_raw,
            range_end: bone.range_raw,
            auxiliary_raw: [bone.word_4_raw, bone.word_6_raw],
        })
        .collect();
    owner.preparation.bind_default_skeleton(
        SkeletonInput {
            primary: ResourceBinding {
                file: source.file(),
                reference: DEFAULT_SKIN,
            },
            bones: &bones,
            auxiliary_words,
            // Host frame updates have not established the source engine counter's
            // gated dispatch history. Preserve this captured unknown, not zero.
            captured_frame_counter: None,
        },
        ResourceBinding {
            file: component.file(),
            reference: 0,
        },
        SkeletonBudget::default(),
    )?;
    hook.ensure_current()?;
    Ok(())
}
