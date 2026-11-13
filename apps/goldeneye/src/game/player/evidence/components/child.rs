use anyhow::{Context, Result, ensure};
use ge_core::animation::{
    nodes::Kind,
    script_child::{self, Stage},
};
use serde::Serialize;
mod phase_zero;
mod position;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildHeader {
    pub native_header_bytes: usize,
    pub resource_reused: bool,
    pub context_bound: bool,
    pub slot: u8,
    pub generation_observed: u32,
    pub cache_generation: u32,
    pub metadata_cached: bool,
    pub descriptor_index: Option<usize>,
    pub body_bound: bool,
    pub phase_zero: Option<phase_zero::PhaseZero>,
    pub phase_zero_initialized: bool,
    pub initializer_returned: bool,
    pub factory_returned: bool,
    pub layer_inserted: bool,
    pub parent_phase_copied: bool,
    pub parent_scalars_initialized: bool,
    pub parent_word_stored: bool,
    pub state_sha256: String,
}
pub(super) fn current(
    animator: &ge_core::mission::player::components::Animator,
    command: &ge_core::animation::layers::command::Pending,
) -> Result<Option<ChildHeader>> {
    let Some(id) = command.factory_node() else {
        return Ok(None);
    };
    let request = command.child().context("child factory request absent")?;
    build_node(animator, command, id, request)
}
pub(super) fn completed_for(
    animator: &ge_core::mission::player::components::Animator,
    command: &ge_core::animation::layers::command::Pending,
) -> Result<Option<ChildHeader>> {
    let Some(effect) = command.last_completed() else {
        return Ok(None);
    };
    build_node(animator, command, effect.node, effect.request)
}
fn build_node(
    animator: &ge_core::mission::player::components::Animator,
    command: &ge_core::animation::layers::command::Pending,
    id: ge_core::animation::layers::NodeId,
    request: ge_core::animation::layers::command::ChildRequest,
) -> Result<Option<ChildHeader>> {
    use ge_core::animation::layers::command::EffectStage;
    let effect = command
        .effects()
        .iter()
        .find(|e| e.node == id)
        .context("child effect allocation history absent")?;
    let header = animator
        .script_children()
        .get(id)
        .context("retained TP child header absent")?;
    let binding = animator.component().context("TP current resource absent")?;
    let latest = command
        .effects()
        .last()
        .context("TP latest allocation absent")?;
    let layers = animator.layers().context("TP child layers absent")?;
    let link = animator.node_ids().links(id);
    let inserted = link.is_some();
    ensure!(
        inserted == effect.inserted
            && inserted
                == layers.slots()[usize::from(request.slot)]
                    .nodes_head()
                    .contains(&id)
            && link.is_none_or(|link| link.slot == request.slot),
        "TP typed intrusive membership differs"
    );
    if inserted {
        let start = 0x138 + usize::from(request.slot) * 0x20;
        ensure!(
            animator.word(start).is_none() && animator.word(start + 4).is_none(),
            "typed script membership contradicts raw NULL list head/tail"
        );
    }
    let scalar = matches!(effect.stage, EffectStage::Scalars | EffectStage::Complete);
    if scalar {
        ensure!(
            header.word(0x34) == Some(0)
                && header.word(0x3c)
                    == Some(
                        if request.divisor == 0 {
                            1f32
                        } else {
                            1f32 / request.divisor as f32
                        }
                        .to_bits()
                    ),
            "TP child scalar writes differ"
        );
    }
    if let Some(bits) = effect.copied_bits {
        ensure!(
            header.word(0x80) == Some(bits) && header.word(0x84) == Some(bits),
            "TP copied parent phase differs"
        );
    }
    if let Some(word) = effect.post_word {
        ensure!(
            header.word(0x24) == Some(word),
            "TP child post-word differs"
        );
    }
    let returned = matches!(
        header.stage(),
        Stage::ReturnedWithoutBody | Stage::ReturnedWithoutPlay | Stage::ReturnedInitialized
    );
    let phase_zero = phase_zero::build(header)?;
    let expected_flags = if let Some(flags) = header.initialization().and_then(|t| t.flags_raw) {
        if flags & 1 != 0 {
            u32::from(request.effective_flags) | 0x80
        } else {
            u32::from(request.effective_flags) & !0x80
        }
    } else {
        u32::from(request.effective_flags)
    };
    ensure!(
        (returned || header.stage() == Stage::AwaitingInitializer || phase_zero.is_some())
            && header.owner().get() == animator.identity().id().get()
            && animator.node_ids().kind(id) == Some(Kind::Script)
            && header.file() == binding.file
            && header.reference() == request.reference
            && header.slot() == Some(request.slot)
            && header.word(0x20) == Some(expected_flags)
            && header.word(0x1c).is_none()
            && header.word(0).is_none()
            && header.word(4).is_none()
            && binding.reference == latest.request.reference
            && animator.word(0x20) == Some(latest.request.reference)
            && animator.word(0x24) == Some(u32::MAX)
            && animator.script_children().len() >= command.effects().len(),
        "TP child factory prefix or ownership differs"
    );
    Ok(Some(ChildHeader {
        native_header_bytes: script_child::HEADER_BYTES,
        resource_reused: true,
        context_bound: true,
        slot: request.slot,
        generation_observed: header
            .observed_generation()
            .context("TP child generation read absent")?,
        cache_generation: header
            .word(0x14)
            .context("TP child cache generation unknown")?,
        metadata_cached: header.cached().is_some(),
        descriptor_index: header.cached().map(|binding| binding.directory_index),
        body_bound: header
            .body_bound()
            .context("TP script body binding absent")?,
        phase_zero,
        phase_zero_initialized: matches!(
            header.stage(),
            Stage::ReturnedWithoutPlay | Stage::ReturnedInitialized
        ),
        initializer_returned: returned,
        factory_returned: returned,
        layer_inserted: inserted,
        parent_phase_copied: effect.copied_bits.is_some(),
        parent_scalars_initialized: scalar,
        parent_word_stored: effect.post_word.is_some(),
        state_sha256: super::super::words(b"ge_player_tp_class04_header_v4\0", |mut emit| {
            header.visit_state_words(&mut emit);
            emit(u32::from(link.is_some()));
            if let Some(link) = link {
                emit(u32::from(link.slot));
                for node in [link.previous, link.next] {
                    emit(u32::from(node.is_some()));
                    if let Some(node) = node {
                        emit((node.get() >> 32) as u32);
                        emit(node.get() as u32);
                    }
                }
            }
        }),
    }))
}
