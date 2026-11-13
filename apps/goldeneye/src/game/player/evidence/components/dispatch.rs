use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::{
    animation::script_child::{self, update},
    mission::player::actions::DispatchStage,
};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThirdPersonDispatch {
    pub stage: &'static str,
    pub captured_position_bits: u32,
    pub setter_return_word: Option<u32>,
    pub history_updated: bool,
    pub primary_changed: Option<bool>,
    pub scalar_visits: usize,
    pub scalar_writes: usize,
    pub updates_started: usize,
    pub updates_completed: usize,
    pub pending_slot: Option<u8>,
    pub script_invocations: usize,
    pub rate_queries: usize,
    pub rate_returns: usize,
    pub clip_reuses: usize,
    pub position_returns: usize,
    pub pending_dependency: Option<&'static str>,
    pub rate_bits: [Option<u32>; 2],
    pub state_sha256: String,
}
fn dependency(t: &update::Trace) -> Option<&'static str> {
    match t.stage {
        update::Stage::AwaitingRate => Some("clip_rate"),
        update::Stage::AwaitingSelection => Some("script_selection"),
        update::Stage::AwaitingPosition => Some("clip_position"),
        update::Stage::Pending => match t.boundary? {
            script_child::Stage::AwaitingAnimationOwner => Some("animation_owner"),
            script_child::Stage::AwaitingClipCache => Some("clip_cache"),
            script_child::Stage::AwaitingAnimationBody => Some("animation_body"),
            script_child::Stage::AwaitingTimingCurve => Some("timing_curve"),
            script_child::Stage::AwaitingRootMotion => Some("root_motion"),
            script_child::Stage::AwaitingClipFile => Some("clip_file"),
            script_child::Stage::AwaitingClipRange => Some("clip_range"),
            script_child::Stage::AwaitingFrameCount => Some("frame_count"),
            script_child::Stage::AwaitingClipReplacement => Some("clip_replacement"),
            _ => None,
        },
        _ => None,
    }
}
pub(super) fn build(player: &Owned) -> Result<ThirdPersonDispatch> {
    let action = player
        .action
        .as_ref()
        .context("TP dispatch action absent")?;
    let d = action
        .state
        .third_person_dispatch()
        .context("TP dispatch entry snapshot absent")?;
    let stage = match d.stage() {
        DispatchStage::AwaitingSetter => "awaiting_setter",
        DispatchStage::Updating => "script_update_pending",
        DispatchStage::Returned => "returned",
        _ => anyhow::bail!("TP dispatch has no reportable retained boundary"),
    };
    let owner = player
        .components
        .preparation
        .animator()
        .context("TP dispatch animator absent")?;
    let layers = owner.layers().context("TP dispatch layers absent")?;
    let mut updates = Vec::new();
    let mut rate_bits = [None; 2];
    let mut pending = None;
    for (i, slot) in [2, 3].into_iter().enumerate() {
        if let Some(id) = layers.slots()[slot].nodes_head().first()
            && let Some(header) = owner.script_children().get(*id)
        {
            ensure!(
                header.updates().len() <= 1,
                "fresh TP dispatcher replayed a script update"
            );
            if let Some(t) = header.updates().last() {
                ensure!(
                    t.stage != update::Stage::Failed,
                    "failed script update cannot report a successful boundary"
                );
                rate_bits[i] = t.rate.as_ref().and_then(|r| r.rate_bits);
                if d.pending_slot() == Some(slot as u8) {
                    pending = dependency(t);
                }
                updates.push(t);
            }
        }
    }
    let setter = d.setter();
    let result = ThirdPersonDispatch {
        stage,
        captured_position_bits: d.captured_position_bits(),
        setter_return_word: setter.map(|s| s.return_word),
        history_updated: setter.is_some_and(|s| s.history_updated),
        primary_changed: setter.and_then(|s| s.primary_changed),
        scalar_visits: d.scalar_visits(),
        scalar_writes: d.scalar_writes(),
        updates_started: d.updates_started(),
        updates_completed: d.updates_completed(),
        pending_slot: d.pending_slot(),
        script_invocations: updates.len(),
        rate_queries: updates.iter().filter(|t| t.rate.is_some()).count(),
        rate_returns: updates
            .iter()
            .filter(|t| t.rate.as_ref().is_some_and(|r| r.rate_bits.is_some()))
            .count(),
        clip_reuses: updates
            .iter()
            .filter(|t| {
                t.selection
                    .as_ref()
                    .is_some_and(|s| s.selected_offset.is_some() && s.requested.is_none())
            })
            .count(),
        position_returns: updates
            .iter()
            .filter(|t| {
                t.stage == update::Stage::Returned
                    && t.position
                        .as_ref()
                        .is_some_and(|p| p.position_bits.is_some() && !p.root_motion_pending)
            })
            .count(),
        pending_dependency: pending,
        rate_bits,
        state_sha256: super::super::words(b"ge_player_tp_dispatch_v1\0", |mut emit| {
            d.visit_state_words(&mut emit);
            for t in &updates {
                t.visit_state_words(&mut emit);
            }
        }),
    };
    ensure!(
        result.stage != "script_update_pending" || result.pending_dependency.is_some(),
        "TP dispatch pending provider was skipped"
    );
    Ok(result)
}
