//! StartPoint selection and initial single-local-player pose planning.
//!
//! Catalog entries are caller-supplied runtime/prepared snapshots, not proof of
//! live trigger membership. Planning does not create a player, satisfy session
//! dependencies, or produce a camera frame. Pose lanes stay raw floating-point
//! bits because selection/copying does not establish finite-value restrictions.
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntry {
    pub wrapper_flag_raw: u8,
    pub id: u32,
    pub trigger_flags_raw: u32,
    pub position_bits: [u32; 4],
    pub rotation_bits: [u32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub entries: usize,
    /// Total addressed rows across the requested-ID pass and optional ID-zero
    /// rescan. Exhaustion is an error, never a fabricated missing selection.
    pub visits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionResult {
    Requested { catalog_index: usize },
    Fallback { catalog_index: usize },
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub result: SelectionResult,
    pub requested_id_before: u32,
    pub requested_id_after: u32,
    pub visits: usize,
}

impl Selection {
    pub fn selected_index(self) -> Option<usize> {
        match self.result {
            SelectionResult::Requested { catalog_index }
            | SelectionResult::Fallback { catalog_index } => Some(catalog_index),
            SelectionResult::Missing => None,
        }
    }

    pub fn used_fallback(self) -> bool {
        matches!(self.result, SelectionResult::Fallback { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pose {
    pub position_bits: [u32; 4],
    pub rotation_bits: [u32; 4],
    pub rotation_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpawnPlan {
    selection: Selection,
    pose: Option<Pose>,
}

impl SourceSpawnPlan {
    /// Prepare the source StartPoint selector's supported single-local-player
    /// branch from an explicit catalog snapshot. This does not assert the
    /// caller's creation gates; use evaluate when those live inputs exist.
    pub fn prepare(catalog: &[CatalogEntry], requested_id: u32, budget: Budget) -> Result<Self> {
        ensure!(
            catalog.len() <= budget.entries,
            "spawn catalog entry budget exhausted"
        );
        let mut visits = 0usize;
        let requested = find(catalog, requested_id, &mut visits, budget.visits)?;
        let (result, requested_id_after) = if let Some(catalog_index) = requested {
            (SelectionResult::Requested { catalog_index }, requested_id)
        } else if let Some(catalog_index) = find(catalog, 0, &mut visits, budget.visits)? {
            (SelectionResult::Fallback { catalog_index }, 0)
        } else {
            (SelectionResult::Missing, requested_id)
        };
        let selection = Selection {
            result,
            requested_id_before: requested_id,
            requested_id_after,
            visits,
        };
        let pose = selection.selected_index().map(|index| {
            let entry = &catalog[index];
            let mut rotation_bits = entry.rotation_bits;
            rotation_bits[0] = 0;
            rotation_bits[2] = 0;
            Pose {
                position_bits: entry.position_bits,
                rotation_bits,
                rotation_mode: 3,
            }
        });
        Ok(Self { selection, pose })
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn pose(&self) -> Option<Pose> {
        self.pose
    }
}

fn find(
    catalog: &[CatalogEntry],
    id: u32,
    visits: &mut usize,
    limit: usize,
) -> Result<Option<usize>> {
    for (index, entry) in catalog.iter().enumerate() {
        ensure!(*visits < limit, "spawn catalog visit budget exhausted");
        *visits += 1;
        // Source sign-extends the byte and tests nonzero. High-bit bytes are
        // eligible too; eligibility is not restricted to the literal byte one.
        if entry.wrapper_flag_raw as i8 != 0 && entry.id == id && entry.trigger_flags_raw & 1 == 0 {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSnapshot {
    pub byte_4a_raw: u8,
    pub flags_28_raw: u32,
}

/// Explicit producer inputs. No default marks unavailable live dependencies
/// ready. The flag predicate is the host-supplied result of the source manager
/// +0x14 query with mask 0x100; it is not inferred from loaded resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationInputs {
    pub session_byte_41_raw: u8,
    pub session_byte_40_raw: u8,
    pub external_state_0b_active: bool,
    pub selected_level_hash: u32,
    pub manager_10_present: bool,
    pub manager_14_present: bool,
    pub manager_14_ready_100: bool,
    pub user_snapshot: Option<UserSnapshot>,
    pub session_mode: u32,
    pub local_user_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateReason {
    Session41,
    Session40,
    ExternalState0b,
    FrontendLevel,
    Manager10Absent,
    Manager14Absent,
    Manager14NotReady,
    UserSnapshotAbsent,
    UserAlreadyCreated,
    UserNotEligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedPath {
    SessionMode { mode: u32 },
    LocalUserCount { count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluation {
    Gated(GateReason),
    Unsupported(UnsupportedPath),
    /// Includes an explicit Missing selection when the source selector finds
    /// neither requested ID nor ID zero. No player is created by this value.
    Planned(SourceSpawnPlan),
}

pub fn evaluate(
    inputs: CreationInputs,
    catalog: &[CatalogEntry],
    requested_id: u32,
    budget: Budget,
) -> Result<Evaluation> {
    let gated = if inputs.session_byte_41_raw != 0 {
        Some(GateReason::Session41)
    } else if inputs.session_byte_40_raw != 0 {
        Some(GateReason::Session40)
    } else if inputs.external_state_0b_active {
        Some(GateReason::ExternalState0b)
    } else if inputs.selected_level_hash == 0x0108_0004 {
        Some(GateReason::FrontendLevel)
    } else if !inputs.manager_10_present {
        Some(GateReason::Manager10Absent)
    } else if !inputs.manager_14_present {
        Some(GateReason::Manager14Absent)
    } else if !inputs.manager_14_ready_100 {
        Some(GateReason::Manager14NotReady)
    } else {
        None
    };
    if let Some(reason) = gated {
        return Ok(Evaluation::Gated(reason));
    }
    let Some(user) = inputs.user_snapshot else {
        return Ok(Evaluation::Gated(GateReason::UserSnapshotAbsent));
    };
    if user.byte_4a_raw != 0 {
        return Ok(Evaluation::Gated(GateReason::UserAlreadyCreated));
    }
    if user.flags_28_raw & 0x100 == 0 {
        return Ok(Evaluation::Gated(GateReason::UserNotEligible));
    }
    if matches!(inputs.session_mode, 1 | 2) {
        return Ok(Evaluation::Unsupported(UnsupportedPath::SessionMode {
            mode: inputs.session_mode,
        }));
    }
    if inputs.local_user_count != 1 {
        return Ok(Evaluation::Unsupported(UnsupportedPath::LocalUserCount {
            count: inputs.local_user_count,
        }));
    }
    Ok(Evaluation::Planned(SourceSpawnPlan::prepare(
        catalog,
        requested_id,
        budget,
    )?))
}

#[cfg(test)]
#[path = "tests/spawn.rs"]
mod tests;
