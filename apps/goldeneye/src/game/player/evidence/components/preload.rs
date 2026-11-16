//! Current preload writes are distinct from the historical initializer snapshot.
//! Public receipts contain counts and semantic references, never cache payloads.
use super::super::super::Owned;
use super::initializer_prefix::{self, CacheState};
use anyhow::{Context, Result, ensure};
use ge_assets::{animation_modes::ClassFourReference, weapon_static::Remap};
use ge_core::mission::player::{
    constructor::{
        self,
        preload::{self, CallStage, ReturnReason, Stage},
    },
    world::World,
};
use serde::Serialize;
mod temporary;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AllocationState {
    pub kind: &'static str,
    pub native_bytes: usize,
    pub payload_bytes: usize,
    pub known_payload_bytes: usize,
    pub header_words: [Option<u32>; 4],
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mapping {
    pub output: u32,
    pub mapped: bool,
    pub steps: usize,
    pub map_rows_read: usize,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceExtraction {
    pub requested_index: usize,
    pub baseline_index: usize,
    pub transition_ordinal: usize,
    pub command_ordinal: usize,
    pub set_index: usize,
    pub group_ordinal: usize,
    pub child_ordinal: usize,
    pub transitions_read: usize,
    pub commands_read: usize,
    pub groups_read: usize,
    pub children_read: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Call {
    pub stage: &'static str,
    pub ordinal: usize,
    pub source_ordinal: usize,
    pub weapon: u32,
    pub generic: u32,
    pub alternate: bool,
    pub variant: u8,
    pub mapping: Option<Mapping>,
    pub raw_flags_read: usize,
    pub raw_keys_read: usize,
    pub raw_index: Option<u8>,
    pub raw_indices_written: bool,
    pub entry_keys_read: usize,
    pub entry_index: Option<u8>,
    pub occupied_written: bool,
    pub weapon_written: bool,
    pub mapping_written: bool,
    pub selected_key_empty: Option<bool>,
    pub invalidated: bool,
    pub extraction_returned: bool,
    pub reference_outcome: Option<&'static str>,
    pub reference: Option<u32>,
    pub extraction: Option<ReferenceExtraction>,
    pub return_word: Option<u32>,
    pub return_reason: Option<&'static str>,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preload {
    pub stage: &'static str,
    pub pending_dependency: &'static str,
    pub entry_flag: Option<u8>,
    pub cache_enabled: u8,
    pub raw_flags_written: u8,
    pub entry_keys_written: u8,
    pub registry_count: Option<usize>,
    pub source_row: usize,
    pub rows_visited: usize,
    pub enabled_rows: usize,
    pub current_weapon: Option<u32>,
    pub call_in_row: u8,
    pub completed_calls: Vec<Call>,
    pub current_call: Option<Call>,
    pub temporary: Option<temporary::Temporary>,
    pub script_publication: Option<crate::game::world::post_trigger_resources::ScriptPublication>,
    pub allocations: [Option<AllocationState>; 3],
    pub caches: CacheState,
    pub flag_cleared: bool,
    pub helper_returned: bool,
    pub state_sha256: String,
}
fn size(value: usize, emit: &mut impl FnMut(u32)) {
    emit((value as u64 >> 32) as u32);
    emit(value as u32);
}
fn optional(value: Option<u32>, emit: &mut impl FnMut(u32)) {
    emit(u32::from(value.is_some()));
    if let Some(value) = value {
        emit(value);
    }
}
fn mapping_words(mapping: &Remap, emit: &mut impl FnMut(u32)) {
    for word in [
        mapping.source_weapon,
        mapping.generic,
        mapping.variant,
        mapping.output,
        u32::from(mapping.mapped),
    ] {
        emit(word);
    }
    size(mapping.map_rows_read, emit);
    size(mapping.steps.len(), emit);
    for step in &mapping.steps {
        emit(step.weapon);
        for index in [step.record_source_index, step.matched_source_ordinal] {
            emit(u32::from(index.is_some()));
            if let Some(index) = index {
                size(index, emit);
            }
        }
        emit(step.candidate_raw);
        optional(step.available.map(u32::from), emit);
    }
}
fn extraction(found: ClassFourReference) -> ReferenceExtraction {
    ReferenceExtraction {
        requested_index: found.requested_index,
        baseline_index: found.baseline_index,
        transition_ordinal: found.transition_ordinal,
        command_ordinal: found.command_ordinal,
        set_index: found.set_index,
        group_ordinal: found.group_ordinal,
        child_ordinal: found.child_ordinal,
        transitions_read: found.transitions_read,
        commands_read: found.commands_read,
        groups_read: found.groups_read,
        children_read: found.children_read,
    }
}
fn extraction_words(found: ClassFourReference, emit: &mut impl FnMut(u32)) {
    emit(found.reference);
    for count in [
        found.requested_index,
        found.baseline_index,
        found.transition_ordinal,
        found.command_ordinal,
        found.set_index,
        found.group_ordinal,
        found.child_ordinal,
        found.transitions_read,
        found.commands_read,
        found.groups_read,
        found.children_read,
    ] {
        size(count, emit);
    }
}
fn call(player: &Owned, state: &preload::Call) -> Result<Call> {
    let request = state.request();
    let owner = player
        .preload
        .as_ref()
        .context("preload call owner absent")?;
    let actual = owner
        .calls
        .iter()
        .find(|call| call.request.ordinal == request.ordinal);
    let mapping = match state.mapping() {
        None => {
            ensure!(
                actual.is_none(),
                "preload retained a map before its source return"
            );
            None
        }
        Some(mapped) => {
            let actual = actual.context("preload source mapping receipt absent")?;
            let mapping = &actual.mapping;
            ensure!(
                actual.request == request
                    && mapping.source_weapon == request.weapon
                    && mapping.generic == request.generic()
                    && mapping.variant == u32::from(request.variant)
                    && mapping.output == mapped.output
                    && mapping.mapped == mapped.accepted,
                "preload mapping changed actual request or source result"
            );
            Some(Mapping {
                output: mapping.output,
                mapped: mapping.mapped,
                steps: mapping.steps.len(),
                map_rows_read: mapping.map_rows_read,
                state_sha256: super::super::words(b"ge_player_preload_mapping_v1\0", |mut emit| {
                    mapping_words(mapping, &mut emit)
                }),
            })
        }
    };
    let retained_extraction = actual.and_then(|actual| actual.extraction);
    ensure!(
        retained_extraction.is_some() == state.extraction_returned(),
        "preload extraction completion differs from retained provider receipt"
    );
    if let Some(result) = retained_extraction {
        ensure!(
            result.map(|found| found.reference) == state.reference(),
            "preload extracted reference differs from actual asset reader"
        );
    }
    let stage = match state.stage() {
        CallStage::Remap => "remap",
        CallStage::RawFlag => "raw_flag",
        CallStage::RawKey => "raw_key",
        CallStage::RawIndices => "raw_indices",
        CallStage::EntryScan => "entry_scan",
        CallStage::RawOccupied => "raw_occupied",
        CallStage::RawWeapon => "raw_weapon",
        CallStage::RawMapping => "raw_mapping",
        CallStage::EntryKey => "entry_key",
        CallStage::Invalidate => "invalidate",
        CallStage::Reference => "reference",
        CallStage::AwaitingTemporary => "awaiting_temporary",
        CallStage::Returned => "returned",
    };
    Ok(Call {
        stage,
        ordinal: request.ordinal,
        source_ordinal: request.source_ordinal,
        weapon: request.weapon,
        generic: request.generic(),
        alternate: request.alternate,
        variant: request.variant,
        mapping,
        raw_flags_read: state.raw_flags_read(),
        raw_keys_read: state.raw_keys_read(),
        raw_index: state.raw_index(),
        raw_indices_written: state.raw_indices_written(),
        entry_keys_read: state.entry_keys_read(),
        entry_index: state.entry_index(),
        occupied_written: state.occupied_written(),
        weapon_written: state.weapon_written(),
        mapping_written: state.mapping_written(),
        selected_key_empty: state.selected_key().map(|key| key == 0),
        invalidated: state.invalidated(),
        extraction_returned: state.extraction_returned(),
        reference_outcome: retained_extraction.map(|result| {
            if result.is_some() {
                "class_four"
            } else {
                "no_reference"
            }
        }),
        reference: state.reference(),
        extraction: retained_extraction.flatten().map(extraction),
        return_word: state.return_word(),
        return_reason: state.return_reason().map(|reason| match reason {
            ReturnReason::VariantUnavailable => "variant_unavailable",
            ReturnReason::RawFull => "raw_full",
            ReturnReason::EntriesFull => "entries_full",
            ReturnReason::ExistingEntry => "existing_entry",
        }),
        state_sha256: super::super::words(b"ge_player_preload_call_v1\0", |mut emit| {
            state.visit_state_words(&mut emit);
            if let Some(actual) = actual {
                mapping_words(&actual.mapping, &mut emit);
                emit(u32::from(actual.extraction.is_some()));
                if let Some(result) = actual.extraction {
                    emit(u32::from(result.is_some()));
                    if let Some(found) = result {
                        extraction_words(found, &mut emit);
                    }
                }
            }
        }),
    })
}
fn allocation(allocation: Option<&constructor::Allocation>) -> Option<AllocationState> {
    allocation.map(|allocation| AllocationState {
        kind: match allocation.kind() {
            constructor::Kind::Raw => "raw",
            constructor::Kind::Array34 => "array34",
            constructor::Kind::Array64 => "array64",
        },
        native_bytes: allocation.native_bytes(),
        payload_bytes: allocation.payload_bytes(),
        known_payload_bytes: allocation.known_payload_bytes(),
        header_words: [0, 4, 8, 12].map(|offset| allocation.header_word(offset)),
    })
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    hook: &crate::game::world_hooks::Attempt,
) -> Result<Option<Preload>> {
    let Some(owner) = &player.preload else {
        return Ok(None);
    };
    let state = &owner.state;
    let prefix = player
        .initializer_prefix
        .as_ref()
        .context("preload initializer owner absent")?;
    let snapshot = player
        .completed_initializer_evidence
        .as_ref()
        .context("preload historical initializer receipt absent")?;
    ensure!(
        state.player() == player.base.player()
            && state.lease() == player.lease
            && state.receiver() == prefix.receivers()[0].context("preload FP receiver absent")?
            && snapshot.stage == "awaiting_preload"
            && snapshot.cache_enabled == Some(1),
        "preload ownership or actual source entry gate differs"
    );
    let receiver = player
        .components
        .preparation
        .mode_animator(state.receiver())?;
    let item = world.item(player.base.item().context("preload current item absent")?)?;
    let file = player
        .components
        .first_person
        .component_source
        .as_ref()
        .context("preload actual FP file absent")?;
    ensure!(
        receiver.player() == state.player()
            && receiver.item() == Some(item.id())
            && item.components().contains(&receiver.identity().id())
            && file.file() == 0x0102_0016
            && receiver
                .component()
                .is_some_and(|binding| binding.file == file.file()),
        "preload actual FP receiver/file/item binding differs"
    );
    ensure!(
        state.row_flags().len() <= state.registry_count().unwrap_or(0)
            && state.enabled_rows() == state.row_flags().iter().filter(|&&flag| flag != 0).count(),
        "preload reached row flags differ from enabled visit count"
    );
    let completed_calls = state
        .completed_calls()
        .iter()
        .map(|state| call(player, state))
        .collect::<Result<Vec<_>>>()?;
    let current_call = state
        .current_call()
        .map(|state| call(player, state))
        .transpose()?;
    ensure!(
        owner.calls.len()
            == state
                .completed_calls()
                .iter()
                .chain(state.current_call())
                .filter(|call| call.mapping().is_some())
                .count(),
        "preload retained provider calls differ from reached source mappings"
    );
    let stage = match state.stage() {
        Stage::Entry => "entry",
        Stage::RawFlags => "raw_flags",
        Stage::EntryKeys => "entry_keys",
        Stage::RegistryCount => "registry_count",
        Stage::Row => "row",
        Stage::Weapon => "weapon",
        Stage::Call => "call",
        Stage::AwaitingTemporary => "awaiting_temporary",
        Stage::FinalFlag => "final_flag",
        Stage::Returned => "returned",
    };
    let script_publication = owner
        .temporary
        .as_ref()
        .and_then(|temporary| temporary.factory_header())
        .and_then(|header| header.cached())
        .map(|binding| file.script_publication_evidence(binding))
        .transpose()?
        .flatten();
    hook.with_player_preload(|shared, caches| {
        ensure!(
            shared.reset_stage(state.lease())? == constructor::ResetStage::Complete,
            "preload evidence lost the actual completed constructor lease"
        );
        if let Some(current) = state.current_call() {
            if let Some(raw) = current.raw_index() {
                if current.occupied_written() {
                    ensure!(
                        shared.preload_raw_flag(state.lease(), raw)? == 1,
                        "preload occupied store differs"
                    );
                }
                if current.weapon_written() {
                    ensure!(
                        shared.preload_raw_key(state.lease(), raw)? == current.request().weapon,
                        "preload weapon store differs"
                    );
                }
                if current.mapping_written() {
                    let field = if current.request().alternate { 9 } else { 5 }
                        + usize::from(current.request().variant);
                    let byte = shared
                        .allocation(constructor::Kind::Raw)
                        .context("preload Raw backing absent")?
                        .payload_byte(usize::from(raw) * 16 + field)?;
                    ensure!(
                        byte == current.entry_index(),
                        "preload actual mapping byte differs"
                    );
                }
            }
            if let (Some(index), Some(key)) = (current.entry_index(), current.selected_key()) {
                ensure!(
                    shared.preload_entry_key(state.lease(), index)? == key,
                    "preload selected entry changed before temporary return/key commit"
                );
            }
        }
        let allocations = [
            constructor::Kind::Raw,
            constructor::Kind::Array34,
            constructor::Kind::Array64,
        ]
        .map(|kind| allocation(shared.allocation(kind)));
        Ok(Some(Preload {
            stage,
            pending_dependency: if owner
                .temporary
                .as_ref()
                .is_some_and(|t| t.bind_return() == Some(1))
            {
                "preload_temporary_post_bind"
            } else {
                state.dependency()
            },
            entry_flag: state.entry_flag(),
            cache_enabled: shared.cache_enabled(),
            raw_flags_written: state.raw_flags_written(),
            entry_keys_written: state.entry_keys_written(),
            registry_count: state.registry_count(),
            source_row: state.source_row(),
            rows_visited: state.row_flags().len(),
            enabled_rows: state.enabled_rows(),
            current_weapon: state.current_weapon(),
            call_in_row: state.call_in_row(),
            completed_calls,
            current_call,
            temporary: owner.temporary.as_ref().map(temporary::build).transpose()?,
            script_publication: script_publication.clone(),
            allocations,
            caches: initializer_prefix::current_caches(caches)?,
            flag_cleared: state.flag_cleared(),
            helper_returned: state.helper_returned(),
            state_sha256: super::super::words(b"ge_player_preload_v2\0", |mut emit| {
                state.visit_state_words(&mut emit);
                shared.visit_state_words(&mut emit);
                caches.visit_state_words(&mut emit);
                emit(u32::from(script_publication.is_some()));
                if let Some(publication) = &script_publication {
                    emit(publication.file);
                    emit(publication.generation);
                    for count in [
                        publication.descriptor_index,
                        publication.section_index,
                        publication.active_requests,
                        publication.retained_sections,
                    ] {
                        size(count, &mut emit);
                    }
                    emit(publication.section_key);
                    emit(u32::from(publication.callback_mask.is_some()));
                    if let Some(mask) = publication.callback_mask {
                        emit(mask);
                    }
                    emit(u32::from(publication.fresh_base));
                    emit((publication.request_count >> 32) as u32);
                    emit(publication.request_count as u32);
                    emit(u32::from(publication.history_unknown));
                    emit(u32::from(publication.raw_job.is_some()));
                    if let Some(job) = publication.raw_job {
                        emit(job);
                    }
                    emit(u32::from(publication.body_state == "known_null"));
                    emit(u32::from(publication.native_history_verified));
                }
                emit(u32::from(owner.temporary.is_some()));
                if let Some(temporary) = &owner.temporary {
                    temporary.visit_state_words(&mut emit);
                }
                for actual in &owner.calls {
                    actual.request.visit_state_words(&mut emit);
                    mapping_words(&actual.mapping, &mut emit);
                    emit(u32::from(actual.extraction.is_some()));
                    if let Some(result) = actual.extraction {
                        emit(u32::from(result.is_some()));
                        if let Some(found) = result {
                            extraction_words(found, &mut emit);
                        }
                    }
                }
            }),
        }))
    })
}
