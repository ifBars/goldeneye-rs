use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::actions::{Stage, Target};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitialAction {
    pub stage: &'static str,
    pub generic_request: u32,
    pub parameter_bits: u32,
    pub direction_raw: u8,
    pub weapon_key: u32,
    pub variant: u32,
    pub remap_calls: usize,
    pub map_rows_read: usize,
    pub fp_mapped: u32,
    pub tp_mapped: u32,
    pub tp_secondary_mapped: u32,
    pub source_tables_reused: bool,
    pub mode_controls_created: usize,
    pub mode_history_updated: bool,
    pub callback_returned: bool,
    pub state_sha256: String,
    pub mapping_state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<InitialAction> {
    let owner = player
        .action
        .as_ref()
        .context("initial action owner is absent")?;
    let state = &owner.state;
    let tp = player
        .components
        .preparation
        .animator()
        .context("initial action TP owner is absent")?;
    let fp = player
        .components
        .preparation
        .first_person()
        .context("initial action FP owner is absent")?
        .animator();
    for animator in [tp, fp] {
        ensure!(
            [0x138, 0x158, 0x178, 0x198]
                .into_iter()
                .all(|at| animator.word(at) == Some(0) && animator.word(at + 4) == Some(0)),
            "initial action mapping boundary unexpectedly owns control heads"
        );
    }
    ensure!(
        state.stage() == Stage::AwaitingThirdPersonDispatch
            && state.player() == player.base.player()
            && state.resolved().len() == 3
            && owner.mappings.len() == 3
            && state.generic() == 0x0900_071e
            && player.reset.byte(0xc7c) == Some(0xff)
            && player.base.word(0x704) == Some(0)
            && [0x70c, 0x710, 0x714, 0x718]
                .into_iter()
                .all(|at| player.reset.word(at) == Some(0)),
        "initial action caller boundary differs"
    );
    let mut hash = Sha256::new();
    hash.update(b"ge_player_initial_action_maps_v1\0");
    hash.update(player.loadout.provenance().state_sha256.as_bytes());
    for ((resolved, mapped), expected) in state.resolved().iter().zip(&owner.mappings).zip([
        Target::FirstPerson,
        Target::ThirdPerson,
        Target::ThirdPersonSecondary,
    ]) {
        ensure!(
            resolved.request.target == expected
                && resolved.request.generic == mapped.generic
                && Some(mapped.source_weapon) == state.weapon()
                && Some(mapped.variant) == state.variant()
                && resolved.output == mapped.output
                && resolved.mapped == mapped.mapped,
            "initial action remap order or retained table result differs"
        );
        for word in [
            mapped.source_weapon,
            mapped.generic,
            mapped.variant,
            mapped.output,
            u32::from(mapped.mapped),
            mapped.map_rows_read as u32,
        ] {
            hash.update(word.to_be_bytes());
        }
        hash.update((mapped.steps.len() as u64).to_be_bytes());
        for step in &mapped.steps {
            hash.update(step.weapon.to_be_bytes());
            hash.update(step.candidate_raw.to_be_bytes());
            for value in [step.record_source_index, step.matched_source_ordinal] {
                hash.update([u8::from(value.is_some())]);
                if let Some(value) = value {
                    hash.update((value as u64).to_be_bytes());
                }
            }
            hash.update([match step.available {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }]);
        }
    }
    Ok(InitialAction {
        stage: "tp_dispatch_pending",
        generic_request: state.generic(),
        parameter_bits: 0,
        direction_raw: 0xff,
        weapon_key: state.weapon().context("initial weapon is unknown")?,
        variant: state.variant().context("initial variant is unknown")?,
        remap_calls: owner.mappings.len(),
        map_rows_read: owner
            .mappings
            .iter()
            .map(|result| result.map_rows_read)
            .sum(),
        fp_mapped: state.resolved()[0].output,
        tp_mapped: state.resolved()[1].output,
        tp_secondary_mapped: state.resolved()[2].output,
        source_tables_reused: true,
        mode_controls_created: 0,
        mode_history_updated: false,
        callback_returned: false,
        state_sha256: super::super::words(b"ge_player_initial_action_v1\0", |emit| {
            state.visit_state_words(emit)
        }),
        mapping_state_sha256: format!("{:x}", hash.finalize()),
    })
}
