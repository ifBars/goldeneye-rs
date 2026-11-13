use super::super::super::Owned;
use anyhow::{Context, Result, ensure};
use ge_core::mission::player::constructor::continuation::{MetadataField, Pending, Stage};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataRead {
    pub slot: u8,
    pub field: &'static str,
    pub value: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverrideRemoval {
    pub key: u32,
    pub count_raw: u32,
    pub removed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstructorTail {
    pub stage: &'static str,
    pub pending_dependency: Option<&'static str>,
    pub saved_state_704: u32,
    pub state_restored: bool,
    pub script_reference: Option<u32>,
    pub metadata_reads: [Option<MetadataRead>; 3],
    pub current_slot: Option<i8>,
    pub current_record_present: Option<bool>,
    pub current_inventory_key: Option<u32>,
    pub variant: Option<u8>,
    pub cleanup_checks: [Option<bool>; 5],
    pub b84_written: bool,
    pub bd1_written: bool,
    pub actor_absent: Option<bool>,
    pub override_removals: [Option<OverrideRemoval>; 2],
    pub helper_return: Option<u32>,
    pub pose_input_raw: Option<u8>,
    pub pose_input_signed: Option<i8>,
    pub state_sha256: String,
}
pub(super) fn build(player: &Owned) -> Result<Option<ConstructorTail>> {
    let Some(tail) = &player.constructor_tail else {
        return Ok(None);
    };
    let entry = player
        .callback_entry
        .context("constructor callback entry absent")?;
    ensure!(
        tail.player() == player.base.player()
            && entry.player() == tail.player()
            && entry.saved_state_704() == tail.saved_state_704()
            && player
                .completed_callback_evidence
                .as_ref()
                .is_some_and(|c| c.callback_returned),
        "constructor continuation changed its entry or lacks a returned callback snapshot"
    );
    if let Some(snapshot) = &player.completed_constructor_evidence {
        ensure!(
            tail.stage() == Stage::AwaitingPose
                && *snapshot == receipt(tail, snapshot.state_sha256.clone()),
            "constructor snapshot differs from its actual completed helper receipt"
        );
        return Ok(Some(snapshot.clone()));
    }
    if tail.stage() != Stage::RestoreState {
        let callback_state = player
            .completed_callback_evidence
            .as_ref()
            .and_then(|c| c.state_704)
            .context("returned callback state snapshot absent")?;
        ensure!(
            tail.state_restored() == (tail.saved_state_704() == 5)
                && player.base.word(0x704)
                    == Some(if tail.saved_state_704() == 5 {
                        5
                    } else {
                        callback_state
                    }),
            "constructor saved-state branch differs from the actual live handler"
        );
    }
    let inventory = player.prelude.inventory();
    if let Some(slot) = tail.current_slot() {
        let record = if slot < 0 {
            None
        } else {
            inventory.records().get(slot as usize)
        };
        ensure!(
            player.reset.byte(0xc96).map(|b| b as i8) == Some(slot)
                && tail.current_record_present() == Some(record.is_some())
                && tail.current_inventory_key() == Some(record.map_or(0, |r| r.key)),
            "constructor current record differs from retained inventory"
        );
        if let Some(variant) = tail.variant() {
            ensure!(
                player.base.byte(0x5ed) == Some(0)
                    && variant == u8::from(record.is_some_and(|r| r.flags & 0x100 != 0)),
                "constructor variant differs from reached record flags"
            );
        }
    }
    for read in tail.metadata_reads().iter().flatten() {
        ensure!(
            player.reset.byte(0xc98) == Some(read.slot),
            "constructor metadata slot changed"
        );
        let row = player
            .loadout
            .metadata_at_selected_slot(read.slot, inventory)?;
        let actual = match read.field {
            MetadataField::Type => row.type_key,
            MetadataField::InventoryKey => row.inventory_key,
            MetadataField::PrimaryKey => row.primary_key,
        };
        ensure!(
            read.value == actual,
            "constructor metadata differs from actual retained row"
        );
    }
    for (check, at) in tail
        .cleanup_checks()
        .iter()
        .zip([0xafc, 0xb00, 0xaf8, 0xbb0, 0xbb4])
    {
        if let Some(absent) = check {
            ensure!(
                player.reset.word(at).map(|word| word == 0) == Some(*absent),
                "constructor cleanup link differs"
            );
        }
    }
    if tail.b84_written() {
        ensure!(
            player.reset.word(0xb84) == Some(0),
            "constructor b84 word clear differs"
        );
    }
    if tail.bd1_written() {
        ensure!(
            player.reset.byte(0xbd1) == Some(0),
            "constructor bd1 byte clear differs"
        );
    }
    if let Some(absent) = tail.actor_absent() {
        ensure!(
            player.base.word(0x570).map(|w| w == 0) == Some(absent),
            "constructor actor gate differs"
        );
    }
    for removal in tail.override_removals().iter().flatten() {
        ensure!(
            player.base.word(0x480) == Some(removal.count_raw)
                && (removal.count_raw as i32) <= 0
                && !removal.removed,
            "constructor count-only override removal changed the container"
        );
    }
    if let Some(raw) = tail.pose_input_raw() {
        ensure!(
            player.reset.byte(0xc7d) == Some(raw),
            "constructor signed pose input differs"
        );
    }
    Ok(Some(receipt(
        tail,
        super::super::words(b"ge_player_constructor_continuation_v1\0", |mut emit| {
            tail.visit_state_words(&mut emit);
            player.base.visit_state_words(&mut emit);
            player.reset.visit_state_words(&mut emit);
            inventory.visit_state_words(&mut emit);
        }),
    )))
}
fn receipt(tail: &Pending, state_sha256: String) -> ConstructorTail {
    let stage = match tail.stage() {
        Stage::RestoreState => "restore_state",
        Stage::SelectScript => "select_script",
        Stage::CurrentRecord => "current_record",
        Stage::Variant => "variant",
        Stage::CleanupKey => "cleanup_key",
        Stage::FirstComponent => "first_component",
        Stage::PrimaryKey => "primary_key",
        Stage::BodyCleanup => "body_cleanup",
        Stage::ReadCleanupLink => "read_cleanup_link",
        Stage::ChildCleanup => "child_cleanup",
        Stage::Marker => "marker",
        Stage::ActorGate => "actor_gate",
        Stage::FirstOverride => "first_override",
        Stage::SecondOverride => "second_override",
        Stage::FirstEffect => "first_effect",
        Stage::SecondEffect => "second_effect",
        Stage::HelperReturn => "helper_return",
        Stage::PoseInput => "pose_input",
        Stage::AwaitingPose => "awaiting_pose",
    };
    ConstructorTail {
        stage,
        pending_dependency: tail.dependency().map(|d| d.as_str()),
        saved_state_704: tail.saved_state_704(),
        state_restored: tail.state_restored(),
        script_reference: tail.script_reference(),
        metadata_reads: tail.metadata_reads().map(|r| {
            r.map(|r| MetadataRead {
                slot: r.slot,
                field: match r.field {
                    MetadataField::Type => "type",
                    MetadataField::InventoryKey => "inventory_key",
                    MetadataField::PrimaryKey => "primary_key",
                },
                value: r.value,
            })
        }),
        current_slot: tail.current_slot(),
        current_record_present: tail.current_record_present(),
        current_inventory_key: tail.current_inventory_key(),
        variant: tail.variant(),
        cleanup_checks: *tail.cleanup_checks(),
        b84_written: tail.b84_written(),
        bd1_written: tail.bd1_written(),
        actor_absent: tail.actor_absent(),
        override_removals: tail.override_removals().map(|r| {
            r.map(|r| OverrideRemoval {
                key: r.key,
                count_raw: r.count_raw,
                removed: r.removed,
            })
        }),
        helper_return: tail.helper_return(),
        pose_input_raw: tail.pose_input_raw(),
        pose_input_signed: tail.pose_input_signed(),
        state_sha256,
    }
}
