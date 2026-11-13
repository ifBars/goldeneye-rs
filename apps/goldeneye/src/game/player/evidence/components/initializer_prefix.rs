use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::{
    constructor::initializer_prefix::Stage,
    transform_cache::{self, Backing, BackingKind, BuildStage, Header, Slot},
    world::World,
};
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackingState {
    pub kind: &'static str,
    pub native_bytes: usize,
    pub known_bytes: usize,
    pub matrix_payload_known_bytes: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderState {
    pub count: u32,
    pub association_bound: bool,
    pub backings: [Option<BackingState>; 5],
    pub invalidation_count_reads: u64,
    pub invalidation_pointer_reads: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildTrace {
    pub slot: &'static str,
    pub stage: &'static str,
    pub bone_count: Option<u32>,
    pub roots_completed: usize,
    pub descendant_visits: usize,
    pub descendant_bytes: usize,
    pub offsets_written: usize,
    pub allocations: usize,
    pub releases: usize,
    pub waiting_skeleton: bool,
    pub failed: bool,
    pub returned: bool,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheTrace {
    pub ordinal: u64,
    pub stage: &'static str,
    pub receivers_found: [bool; 2],
    pub selected_before: Option<&'static str>,
    pub builds: [Option<BuildTrace>; 2],
    pub missing_receiver_skip: bool,
    pub reused_selected: bool,
    pub selection_returned: bool,
    pub selected_after: Option<&'static str>,
    pub failed: bool,
    pub returned: bool,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheState {
    pub selected: Option<&'static str>,
    pub counts: [u32; 2],
    pub matrix_backings: [bool; 2],
    pub associations: [bool; 2],
    pub native_bytes: usize,
    pub allocations: usize,
    pub invocation_count: u64,
    pub headers: [HeaderState; 2],
    pub trace: Option<CacheTrace>,
    pub state_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitializerPrefix {
    pub stage: &'static str,
    pub pending_dependency: &'static str,
    pub item_flags_before: Option<u16>,
    pub item_flags_after: Option<u16>,
    pub mode_written: bool,
    pub dirty_cleared: bool,
    pub scalars_written: bool,
    pub receiver_searches: usize,
    pub receivers_found: [bool; 2],
    pub cache_enabled: Option<u8>,
    pub invalidated: bool,
    pub caches: Option<CacheState>,
    pub state_sha256: String,
}
fn slot(slot: Slot) -> &'static str {
    match slot {
        Slot::FirstPerson => "first_person",
        Slot::ThirdPerson => "third_person",
    }
}
fn backing(b: Option<&Backing>) -> Result<Option<BackingState>> {
    let Some(b) = b else { return Ok(None) };
    let mut known_bytes = 0;
    let mut payload = 0;
    for i in 0..b.native_bytes() {
        if b.byte(i)?.is_some() {
            known_bytes += 1;
            if i >= 16 {
                payload += 1;
            }
        }
    }
    Ok(Some(BackingState {
        kind: match b.kind() {
            BackingKind::Matrix => "matrix",
            BackingKind::Validity => "validity",
            BackingKind::Offsets => "offsets",
            BackingKind::Temporary => "temporary",
            BackingKind::Descendants => "descendants",
        },
        native_bytes: b.native_bytes(),
        known_bytes,
        matrix_payload_known_bytes: (b.kind() == BackingKind::Matrix).then_some(payload),
    }))
}
fn header(h: &Header) -> Result<HeaderState> {
    let (count, pointer) = h.invalidation_reads();
    Ok(HeaderState {
        count: h.count(),
        association_bound: h.association().is_some(),
        backings: [
            backing(h.matrix())?,
            backing(h.validity())?,
            backing(h.offsets())?,
            backing(h.descendants())?,
            backing(h.temporary())?,
        ],
        invalidation_count_reads: count,
        invalidation_pointer_reads: pointer,
    })
}
fn build_trace(t: &transform_cache::BuildTrace) -> BuildTrace {
    BuildTrace {
        slot: slot(t.slot),
        stage: match t.stage {
            BuildStage::CleanupGate => "cleanup_gate",
            BuildStage::Skeleton => "skeleton",
            BuildStage::MatrixAllocation => "matrix_allocation",
            BuildStage::ValidityAllocation => "validity_allocation",
            BuildStage::ValidityClear => "validity_clear",
            BuildStage::OffsetsAllocation => "offsets_allocation",
            BuildStage::TemporaryAllocation => "temporary_allocation",
            BuildStage::Descendants => "descendants",
            BuildStage::CompactAllocation => "compact_allocation",
            BuildStage::CompactCopy => "compact_copy",
            BuildStage::TemporaryRelease => "temporary_release",
            BuildStage::Returned => "returned",
        },
        bone_count: t.bone_count,
        roots_completed: t.roots_completed,
        descendant_visits: t.descendant_visits,
        descendant_bytes: t.descendant_bytes,
        offsets_written: t.offsets_written,
        allocations: t.allocations.len(),
        releases: t.releases.len(),
        waiting_skeleton: t.waiting_skeleton,
        failed: t.failed,
        returned: t.returned,
        state_sha256: super::super::words(b"ge_player_transform_cache_build_v1\0", |emit| {
            t.visit_state_words(emit)
        }),
    }
}
fn trace(t: &transform_cache::Trace) -> CacheTrace {
    CacheTrace {
        ordinal: t.ordinal,
        stage: match t.stage {
            transform_cache::Stage::Entry => "entry",
            transform_cache::Stage::FirstPerson => "first_person",
            transform_cache::Stage::ThirdPerson => "third_person",
            transform_cache::Stage::Selecting => "selecting",
            transform_cache::Stage::Returned => "returned",
        },
        receivers_found: t.receivers.map(|r| r.is_some()),
        selected_before: t.selected_before.map(slot),
        builds: t.builds.each_ref().map(|b| b.as_ref().map(build_trace)),
        missing_receiver_skip: t.missing_receiver_skip,
        reused_selected: t.reused_selected,
        selection_returned: t.selection_returned,
        selected_after: t.selected_after.map(slot),
        failed: t.failed,
        returned: t.returned,
        state_sha256: super::super::words(b"ge_player_transform_cache_call_v1\0", |emit| {
            t.visit_state_words(emit)
        }),
    }
}
pub(super) fn build(
    player: &Owned,
    world: &World,
    hook: &crate::game::world_hooks::Attempt,
) -> Result<Option<InitializerPrefix>> {
    let Some(prefix) = &player.initializer_prefix else {
        return Ok(None);
    };
    ensure!(
        prefix.player() == player.base.player()
            && player
                .completed_component_evidence
                .as_ref()
                .is_some_and(|t| t.helper_returned && t.helper_return == Some(1)),
        "initializer prefix lacks the actual outer component-helper success snapshot"
    );
    let item = world.item(player.base.item().context("initializer item absent")?)?;
    if let Some((before, after)) = prefix.flags() {
        ensure!(
            after == (before | 0xc) && item.transform_flags_90() == after,
            "initializer actual U16 transform flags differ"
        );
    }
    if prefix.mode_written() {
        ensure!(
            item.rotation_mode_93() == Some(3),
            "initializer actual rotation mode differs"
        );
    }
    if prefix.dirty_cleared() {
        ensure!(
            item.factory().is_some_and(|f| f.byte_8e() == 0),
            "initializer actual dirty clear differs"
        );
    }
    if prefix.scalars_written() {
        for (at, bits) in [
            (0x758, 0),
            (0x75c, 0),
            (0x760, 0x4300_0000),
            (0x768, f32::MAX.to_bits()),
            (0x6b8, 0),
            (0x6dc, 0),
        ] {
            ensure!(
                player.base.word(at) == Some(bits),
                "initializer actual narrow base scalar store differs"
            );
        }
    }
    for (i, receiver) in prefix.receivers().into_iter().enumerate() {
        if let Some(receiver) = receiver {
            let owner = player.components.preparation.mode_animator(receiver)?;
            ensure!(
                owner.player() == prefix.player()
                    && owner.item() == Some(item.id())
                    && item.components().contains(&owner.identity().id())
                    && owner
                        .component()
                        .is_some_and(|b| b.file == [0x0102_0016, 0x0102_0017][i]),
                "initializer actual FP/TP receiver search differs"
            );
        }
    }
    let caches = hook.with_player_transform_caches(|owner, shared| {
        if let Some(flag) = prefix.cache_enabled() {
            ensure!(
                shared.cache_enabled() == flag,
                "initializer shared source cache flag differs"
            );
        }
        let Some(snapshot) = prefix.cache_snapshot() else {
            return Ok(None);
        };
        ensure!(
            owner.snapshot() == snapshot,
            "initializer retained cache snapshot differs from the actual controller owner"
        );
        if let Some(t) = prefix.cache_trace() {
            ensure!(
                owner.trace() == Some(t) && t.receivers == prefix.receivers(),
                "initializer retained cache call differs from the actual source trace"
            );
        }
        Ok(Some(CacheState {
            selected: snapshot.selected.map(slot),
            counts: snapshot.counts,
            matrix_backings: snapshot.matrix_backings.map(|b| b.is_some()),
            associations: snapshot.associations.map(|p| p.is_some()),
            native_bytes: snapshot.native_bytes,
            allocations: snapshot.allocations,
            invocation_count: snapshot.invocation_count,
            headers: [
                header(owner.header(Slot::FirstPerson))?,
                header(owner.header(Slot::ThirdPerson))?,
            ],
            trace: prefix.cache_trace().map(trace),
            state_sha256: super::super::words(b"ge_player_transform_cache_owner_v1\0", |emit| {
                owner.visit_state_words(emit)
            }),
        }))
    })?;
    let stage = match prefix.stage() {
        Stage::ItemFlags => "item_flags",
        Stage::ItemMode => "item_mode",
        Stage::ItemDirty => "item_dirty",
        Stage::BaseScalars => "base_scalars",
        Stage::FirstPerson => "first_person",
        Stage::ThirdPerson => "third_person",
        Stage::Caches => "caches",
        Stage::CacheFlag => "cache_flag",
        Stage::Invalidate => "invalidate",
        Stage::AwaitingPreload => "awaiting_preload",
        Stage::AwaitingTail => "awaiting_tail",
    };
    Ok(Some(InitializerPrefix {
        stage,
        pending_dependency: prefix.dependency(),
        item_flags_before: prefix.flags().map(|v| v.0),
        item_flags_after: prefix.flags().map(|v| v.1),
        mode_written: prefix.mode_written(),
        dirty_cleared: prefix.dirty_cleared(),
        scalars_written: prefix.scalars_written(),
        receiver_searches: prefix.searches(),
        receivers_found: prefix.receivers().map(|r| r.is_some()),
        cache_enabled: prefix.cache_enabled(),
        invalidated: prefix.invalidated(),
        caches,
        state_sha256: super::super::words(b"ge_player_initializer_prefix_v1\0", |mut emit| {
            prefix.visit_state_words(&mut emit);
            player.base.visit_state_words(&mut emit);
            item.visit_state_words(&mut emit);
        }),
    }))
}
