//! Lazy source action remapping. Availability belongs to the actual target
//! component; a missing provider is an error, not an unavailable mode.
use super::{QueryBudget, Remap, RemapStep, WeaponTables};
use anyhow::{Result, ensure};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingKind {
    FirstPerson,
    ThirdPerson,
    ThirdPersonSecondary,
}

impl WeaponTables {
    pub fn map_action(
        &self,
        weapon: u32,
        generic: u32,
        variant: u32,
        kind: MappingKind,
        budget: QueryBudget,
        mut available: impl FnMut(u32) -> Result<bool>,
    ) -> Result<Remap> {
        let mut result = Remap {
            source_weapon: weapon,
            generic,
            variant,
            output: generic,
            mapped: false,
            steps: Vec::new(),
            map_rows_read: 0,
        };
        let mut current = weapon;
        let mut seen = BTreeSet::new();
        loop {
            ensure!(
                seen.len() < budget.remap_steps,
                "static weapon remap-step budget exhausted"
            );
            ensure!(seen.insert(current), "static weapon parent cycle");
            let Some(record) = self.record(current) else {
                result.steps.push(RemapStep {
                    weapon: current,
                    record_source_index: None,
                    matched_source_ordinal: None,
                    candidate_raw: 0,
                    available: None,
                });
                return Ok(result);
            };
            let parent = if kind == MappingKind::FirstPerson {
                record.fp_parent_key()?
            } else {
                record.tp_parent_key()?
            };
            let tp_map = if kind == MappingKind::FirstPerson {
                None
            } else {
                record.tp_map()?
            };
            let normal = if tp_map.is_some() {
                tp_map
            } else {
                record.normal_map()?
            };
            let secondary = record.secondary_map()?;
            // Secondary has priority. If a parent exists, source skips this
            // record's normal map after a secondary miss and recurses instead.
            let maps = [
                (kind == MappingKind::ThirdPersonSecondary, secondary),
                (
                    kind != MappingKind::ThirdPersonSecondary || parent == 0,
                    normal,
                ),
            ];
            for (reached, map) in maps {
                if !reached {
                    continue;
                }
                let lookup = self.lookup(
                    map,
                    generic,
                    variant,
                    budget.map_rows,
                    &mut result.map_rows_read,
                )?;
                let accepted = if lookup.mapped_raw != 0 {
                    Some(available(lookup.mapped_raw)?)
                } else {
                    None
                };
                result.steps.push(RemapStep {
                    weapon: current,
                    record_source_index: Some(record.source_index()),
                    matched_source_ordinal: lookup.matched_source_ordinal,
                    candidate_raw: lookup.mapped_raw,
                    available: accepted,
                });
                if accepted == Some(true) {
                    result.output = lookup.mapped_raw;
                    result.mapped = true;
                    return Ok(result);
                }
            }
            if parent == 0 {
                return Ok(result);
            }
            current = parent;
        }
    }
}
