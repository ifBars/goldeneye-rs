//! Prepare map rows from an explicit one-initialization preference snapshot.
//! This does not construct map instances or execute their subtype factories.
use anyhow::Result;
use ge_core::mission::{
    registration::{Budget, Preferences, RegistrationPlan},
    residency::Request,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(super) struct RowEvidence {
    source_ordinal: usize,
    file_hash: u32,
    map_reference: u32,
    flags_raw: [u8; 3],
    subtype_allowlist: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct RegistrationEvidence {
    prepared: bool,
    preference_snapshot: &'static str,
    native_init_timing_verified: bool,
    rows: Vec<RowEvidence>,
    scope: &'static str,
}

pub(super) struct PreparedRegistration {
    plan: RegistrationPlan,
    evidence: RegistrationEvidence,
}

impl PreparedRegistration {
    /// The inspection context declares a fresh snapshot of the source append
    /// initializer. This is not an inference about live original manager state:
    /// its separate virtual-init timing and previous history remain unverified.
    pub fn from_initialization_snapshot(requests: &[Request]) -> Result<Self> {
        let preferences = Preferences {
            prefixes_98c: vec![0x018b_0000, 0x01a1_0000, 0x01a3_0000],
            prefixes_9ac: vec![0x0181_0000, 0x01a0_0000, 0x01a2_0000],
            prefixes_9cc: vec![0x0183_0000, 0x01a4_0000],
            filter_9ec: vec![0x1f00_0088],
            filter_a2c: vec![
                0x1f00_0006,
                0x1f00_0034,
                0x1f00_0096,
                0x1f00_00ac,
                0x1f00_0054,
                0x1f00_009a,
            ],
        };
        let plan = RegistrationPlan::build(
            requests,
            preferences,
            Budget {
                rows: 128,
                preference_words: 64,
            },
        )?;
        let rows = plan
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                Ok(RowEvidence {
                    source_ordinal: row.source_index(),
                    file_hash: row.request().file,
                    map_reference: row.request().map_reference,
                    flags_raw: row.flags_raw(),
                    subtype_allowlist: plan.row_filter(index)?.map(<[u32]>::to_vec),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let evidence = RegistrationEvidence {
            prepared: true,
            preference_snapshot: "source_initializer_once",
            native_init_timing_verified: false,
            rows,
            scope: "Prepared ordering, raw row flags and subtype allowlists from an explicit fresh-context snapshot of the source preference initializer. Original virtual-init timing/history is unverified. No map instances or trigger/subtype factories are constructed; this plan is not completed world registration.",
        };
        Ok(Self { plan, evidence })
    }
    pub fn plan(&self) -> &RegistrationPlan {
        &self.plan
    }
    pub fn evidence(&self) -> &RegistrationEvidence {
        &self.evidence
    }
}

#[cfg(test)]
#[path = "tests/registration.rs"]
mod tests;
