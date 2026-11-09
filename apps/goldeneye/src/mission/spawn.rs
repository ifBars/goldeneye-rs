//! Prepared StartPoint selection from retained data and explicit registration
//! filters. No live trigger catalog, player object or camera is created here.
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::Edb,
    mission::{TriggerLocator, startpoints},
};
use ge_core::mission::{
    registration::RegistrationPlan,
    spawn::{Budget, CatalogEntry, SourceSpawnPlan},
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const STARTPOINT_SUBTYPE: u32 = 0x1f00_0081;
const MAX_ENTRIES: usize = 4096;
const ENCODING: &str = "position_u32be[4] || rotation_u32be[4] || rotation_mode_u8";

#[derive(Debug, Clone, Serialize)]
pub(super) struct SelectedLocator {
    file_hash: u32,
    source_ordinal: usize,
    prepared_row_index: usize,
    locator: TriggerLocator,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SpawnEvidence {
    state: &'static str,
    requested_id_before: u32,
    requested_id_after: u32,
    registration_rows: usize,
    eligible_files: usize,
    filtered_files: usize,
    prepared_entries: usize,
    visits: usize,
    selection_found: bool,
    used_fallback: bool,
    selected_catalog_index: Option<usize>,
    selected: Option<SelectedLocator>,
    pose_sha256: Option<String>,
    pose_sha256_encoding: &'static str,
    rotation_mode: Option<u8>,
    catalog_membership_verified: bool,
    catalog_created: bool,
    creation_inputs_verified: bool,
    camera_frame_ready: bool,
    scope: &'static str,
}

pub(super) struct PreparedSpawn {
    plan: SourceSpawnPlan,
    evidence: SpawnEvidence,
}

impl PreparedSpawn {
    pub fn prepare(
        registration: &RegistrationPlan,
        resources: &[Edb],
        requested_id: u32,
    ) -> Result<Self> {
        ensure!(
            registration.rows().len() == resources.len(),
            "spawn preparation resource/registration count differs"
        );
        let mut catalog = Vec::new();
        let mut locators = Vec::new();
        let mut eligible_files = 0;
        let mut filtered_files = 0;
        for (prepared_row_index, row) in registration.rows().iter().enumerate() {
            let source_ordinal = row.source_index();
            let edb = resources
                .get(source_ordinal)
                .context("spawn preparation source resource is absent")?;
            ensure!(
                edb.header.hash == row.request().file,
                "spawn preparation resource identity differs from row"
            );
            let filter = registration.row_filter(prepared_row_index)?;
            if row.flags_raw()[2] == 0
                || filter.is_some_and(|values| !values.contains(&STARTPOINT_SUBTYPE))
            {
                filtered_files += 1;
                continue;
            }
            eligible_files += 1;
            ensure!(
                edb.header.version == 263,
                "StartPoint preparation requires supported EDB version"
            );
            let points = edb
                .mission_start_points(
                    row.request().map_reference,
                    startpoints::Budget {
                        maps: 128,
                        rows: 65_536,
                        types: 65_536,
                        points: MAX_ENTRIES - catalog.len(),
                    },
                )?
                .context("spawn preparation map reference is absent")?;
            catalog
                .try_reserve(points.len())
                .context("retaining prepared StartPoint catalog")?;
            locators
                .try_reserve(points.len())
                .context("retaining prepared StartPoint provenance")?;
            for point in points {
                // The source catalog constructor assigns byte0=1 for this
                // exact class. This is an explicit prepared snapshot, not a
                // claim that its factory or live-list insertion has occurred.
                catalog.push(CatalogEntry {
                    wrapper_flag_raw: 1,
                    id: point.id,
                    trigger_flags_raw: point.trigger_flags_raw,
                    position_bits: point.position_bits,
                    rotation_bits: point.rotation_bits,
                });
                locators.push(SelectedLocator {
                    file_hash: edb.header.hash,
                    source_ordinal,
                    prepared_row_index,
                    locator: point.locator,
                });
            }
        }
        let plan = SourceSpawnPlan::prepare(
            &catalog,
            requested_id,
            Budget {
                entries: MAX_ENTRIES,
                visits: MAX_ENTRIES * 2,
            },
        )?;
        let selection = plan.selection();
        let selected = selection
            .selected_index()
            .map(|index| {
                locators
                    .get(index)
                    .cloned()
                    .context("selected StartPoint provenance absent")
            })
            .transpose()?;
        let pose = plan.pose();
        let pose_sha256 = pose.map(|pose| {
            let mut digest = Sha256::new();
            for word in pose.position_bits.into_iter().chain(pose.rotation_bits) {
                digest.update(word.to_be_bytes());
            }
            digest.update([pose.rotation_mode]);
            format!("{:x}", digest.finalize())
        });
        let evidence = SpawnEvidence {
            state: if selected.is_some() {
                "prepared_startpoint"
            } else {
                "missing_startpoint"
            },
            requested_id_before: selection.requested_id_before,
            requested_id_after: selection.requested_id_after,
            registration_rows: registration.rows().len(),
            eligible_files,
            filtered_files,
            prepared_entries: catalog.len(),
            visits: selection.visits,
            selection_found: selected.is_some(),
            used_fallback: selection.used_fallback(),
            selected_catalog_index: selection.selected_index(),
            selected,
            pose_sha256,
            pose_sha256_encoding: ENCODING,
            rotation_mode: pose.map(|pose| pose.rotation_mode),
            catalog_membership_verified: false,
            catalog_created: false,
            creation_inputs_verified: false,
            camera_frame_ready: false,
            scope: "StartPoint records selected in explicit prepared registration order with its subtype allowlists. Wrapper byte1 models the source catalog-construction snapshot; live catalog membership and creation gates are unverified. The owned source spawn plan preserves raw position, zeroes rotation X/Z, preserves yaw and uses rotation mode3. Public evidence contains a pose hash and locators, never coordinates or pose arrays. Requested ID1 is the declared inspection default and is proven for Tutorial New Game only; other caller histories remain explicit inputs. No trigger/player/camera is created.",
        };
        Ok(Self { plan, evidence })
    }

    pub fn plan(&self) -> &SourceSpawnPlan {
        &self.plan
    }
    pub fn prepared_entry_count(&self) -> usize {
        self.evidence.prepared_entries
    }
    pub fn evidence(&self) -> &SpawnEvidence {
        &self.evidence
    }
}

#[cfg(test)]
#[path = "tests/spawn.rs"]
mod tests;
