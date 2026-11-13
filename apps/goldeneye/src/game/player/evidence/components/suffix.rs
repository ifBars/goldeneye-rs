use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::actions::{
    Target,
    suffix::{MaskHelper, MaskLookup, SuffixStage},
};
use ge_core::{animation::nodes::Kind, mission::player::components::Animator};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lookup {
    pub outcome: &'static str,
    pub directory_index: Option<usize>,
    pub attribute_present: Option<bool>,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Helper {
    pub target: &'static str,
    pub receiver_searched: bool,
    pub receiver_bound: bool,
    pub lookups: [Option<Lookup>; 2],
    pub applied_slots: Vec<u8>,
    pub returned: bool,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionSuffix {
    pub stage: &'static str,
    pub classification: u8,
    pub mask_helpers: [Helper; 2],
    pub parameter_visits: usize,
    pub parameter_writes: usize,
    pub pending_dependency: Option<&'static str>,
    pub repeat_generic: Option<u32>,
    pub repeat_parameter_bits: Option<u32>,
    pub state_sha256: String,
}
fn lookup(value: MaskLookup) -> Lookup {
    let (outcome, directory_index, attribute_present, tag) = match value {
        MaskLookup::MissingDescriptor => ("missing_descriptor", None, None, 1),
        MaskLookup::MissingBody => ("missing_body", None, None, 2),
        MaskLookup::Pending => ("pending", None, None, 3),
        MaskLookup::Resident(value) => (
            "resident",
            Some(value.directory_index),
            Some(value.attribute_present),
            4,
        ),
    };
    Lookup {
        outcome,
        directory_index,
        attribute_present,
        state_sha256: super::super::words(b"ge_player_suffix_mask_lookup_v1\0", |mut emit| {
            emit(tag);
            if let MaskLookup::Resident(value) = value {
                value.visit_state_words(&mut emit);
            }
        }),
    }
}
fn helper(player: &Owned, h: &MaskHelper) -> Result<Helper> {
    let target = match h.target() {
        Target::FirstPerson => "first_person",
        Target::ThirdPerson => "third_person",
        _ => anyhow::bail!("secondary target cannot own a suffix mask helper"),
    };
    if let Some(receiver) = h.receiver() {
        let owner = player.components.preparation.mode_animator(receiver)?;
        ensure!(
            owner.player() == player.base.player()
                && owner.component().is_some_and(|b| b.file
                    == if h.target() == Target::FirstPerson {
                        0x0102_0016
                    } else {
                        0x0102_0017
                    }),
            "suffix mask receiver differs"
        );
        for &slot in h.applied_slots() {
            let index = usize::from(slot);
            let layers = owner.layers().context("applied mask layers absent")?;
            let layer = layers
                .slots()
                .get(index)
                .context("applied mask physical slot exceeds four")?;
            let words = layer
                .bone_mask_words()
                .context("applied mask words absent from actual layer")?;
            let start = 0x138 + index * 0x20;
            ensure!(
                words
                    .into_iter()
                    .enumerate()
                    .all(|(i, word)| owner.word(start + 0x10 + i * 4) == Some(word))
                    && layer.flags_raw() & 2 != 0
                    && owner
                        .byte(start + 8)
                        .zip(owner.byte(start + 9))
                        .map(|(a, b)| u16::from_be_bytes([a, b]))
                        == Some(layer.flags_raw()),
                "applied mask words/flags differ from current owner"
            );
        }
    } else {
        ensure!(
            h.applied_slots().is_empty(),
            "unbound suffix receiver has mask writes"
        );
    }
    Ok(Helper {
        target,
        receiver_searched: h.searched(),
        receiver_bound: h.receiver().is_some(),
        lookups: h.lookups().map(|v| v.map(lookup)),
        applied_slots: h.applied_slots().to_vec(),
        returned: h.returned(),
        state_sha256: super::super::words(b"ge_player_suffix_mask_helper_v1\0", |mut emit| {
            h.visit_state_words(&mut emit)
        }),
    })
}
fn weight(owner: &Animator, slot: usize, bits: u32) -> Result<()> {
    ensure!(
        owner
            .layers()
            .context("suffix weight layers absent")?
            .slots()[slot]
            .weight_bits()
            == bits
            && owner.word(0x144 + slot * 0x20) == Some(bits),
        "returned suffix weight differs from raw/typed owner"
    );
    Ok(())
}
fn script_word(owner: &Animator, slot: usize, at: usize) -> Result<u32> {
    let id = owner
        .layers()
        .context("suffix script layers absent")?
        .slots()[slot]
        .nodes_head()
        .first()
        .copied()
        .context("suffix script head absent")?;
    let header = owner
        .script_children()
        .get(id)
        .context("suffix script owner absent")?;
    ensure!(
        owner.node_ids().kind(id) == Some(Kind::Script)
            && header.owner().get() == owner.identity().id().get()
            && header.slot() == Some(slot as u8),
        "suffix script head identity differs"
    );
    header
        .word(at)
        .context("returned suffix script scalar unknown")
}
pub(super) fn build(player: &Owned) -> Result<Option<ActionSuffix>> {
    let state = &player
        .action
        .as_ref()
        .context("suffix action owner absent")?
        .state;
    let Some(s) = state.action_suffix() else {
        return Ok(None);
    };
    if s.stage() == SuffixStage::Returned
        && s.classification() == 1
        && state.generic() == 0x0900_071e
        && [0x99c, 0x9a0, 0x9a4, 0xc8c]
            .into_iter()
            .all(|at| player.reset.word(at) == Some(0))
        && player.reset.word(0x70c) == Some(0x0900_071e)
    {
        let fp = player
            .components
            .preparation
            .first_person()
            .context("returned suffix FP absent")?
            .animator();
        let tp = player
            .components
            .preparation
            .animator()
            .context("returned suffix TP absent")?;
        ensure!(
            fp.word(0x108) == Some(1f32.to_bits()) && tp.word(0x108) == Some(1f32.to_bits()),
            "returned suffix component speed differs"
        );
        weight(fp, 1, 1f32.to_bits())?;
        weight(tp, 0, 1f32.to_bits())?;
        for slot in [1, 2, 3] {
            weight(tp, slot, 0)?;
        }
        ensure!(
            script_word(tp, 2, 0x2c)? == 1f32.to_bits()
                && script_word(tp, 3, 0x2c)? == 0x3f2a_aaab
                && script_word(tp, 2, 0x64)? == 0
                && script_word(tp, 3, 0x64)? == 0
                && script_word(fp, 1, 0x64)? == 0,
            "returned suffix script speed/coordinate differs"
        );
    }
    let stage = match s.stage() {
        SuffixStage::FirstPersonMasks => "first_person_masks",
        SuffixStage::ThirdPersonMasks => "third_person_masks",
        SuffixStage::FirstPersonParameters => "first_person_parameters",
        SuffixStage::ThirdPersonParameters => "third_person_parameters",
        SuffixStage::Returned => "returned",
        SuffixStage::Failed => anyhow::bail!("failed suffix cannot publish a returned action"),
    };
    let [fp, tp] = s.mask_helpers();
    let pending_dependency = match s.stage() {
        SuffixStage::FirstPersonMasks | SuffixStage::ThirdPersonMasks => Some("bone_masks"),
        SuffixStage::FirstPersonParameters | SuffixStage::ThirdPersonParameters => {
            Some(s.parameter_dependency().unwrap_or("action_parameters"))
        }
        _ => None,
    };
    let repeat = s.repeat_request();
    Ok(Some(ActionSuffix {
        stage,
        classification: s.classification(),
        mask_helpers: [helper(player, fp)?, helper(player, tp)?],
        parameter_visits: s.parameter_visits(),
        parameter_writes: s.parameter_writes(),
        pending_dependency,
        repeat_generic: repeat.map(|r| r.generic),
        repeat_parameter_bits: repeat.map(|r| r.parameter_bits),
        state_sha256: super::super::words(b"ge_player_action_suffix_v1\0", |mut emit| {
            s.visit_state_words(&mut emit);
            for h in s.mask_helpers() {
                if let Some(receiver) = h.receiver() {
                    player
                        .components
                        .preparation
                        .mode_animator(receiver)
                        .expect("validated suffix receiver")
                        .visit_state_words(&mut emit);
                }
            }
        }),
    }))
}
