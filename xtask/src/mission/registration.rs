//! Validate preparation as a plan, never as live map-instance construction.
use super::{closed, number};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn validate(value: &Value, residency: &Value) -> Result<()> {
    closed(
        value,
        &[
            "prepared",
            "preference_snapshot",
            "native_init_timing_verified",
            "rows",
            "scope",
        ],
    )?;
    ensure!(
        value["prepared"] == true
            && value["preference_snapshot"] == "source_initializer_once"
            && value["native_init_timing_verified"] == false,
        "registration plan invented original initialization history"
    );
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "registration scope missing"
    );
    let rows = value["rows"]
        .as_array()
        .context("registration rows missing")?;
    let resources = residency["resources"]
        .as_array()
        .context("resident resources missing")?;
    ensure!(
        !rows.is_empty() && rows.len() == resources.len(),
        "registration plan lost or invented resident rows"
    );
    let mut indices = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        closed(
            row,
            &[
                "source_ordinal",
                "file_hash",
                "map_reference",
                "flags_raw",
                "subtype_allowlist",
            ],
        )?;
        let ordinal = number(row, "source_ordinal")? as usize;
        let source = resources
            .get(ordinal)
            .context("registration source ordinal is absent")?;
        ensure!(
            indices.insert(ordinal)
                && row["file_hash"] == source["file_hash"]
                && row["map_reference"] == source["requested_map_reference"],
            "registration did not retain source row identity"
        );
        let (flags, allowlist) = if rows.len() == 1 {
            (json!([255, 1, 1]), Value::Null)
        } else if index == 0 {
            ensure!(
                ordinal == 0,
                "mission container left the source first registration slot"
            );
            (json!([255, 0, 1]), Value::Null)
        } else if index == 1 {
            let file = number(row, "file_hash")?;
            ensure!(
                [0x018b_0000, 0x01a1_0000, 0x01a3_0000].contains(&(file & 0xffff_0000))
                    && file & 0xffff != 0,
                "preferred registration row is absent from index one"
            );
            (json!([255, 1, 1]), json!([0x1f00_0088u32]))
        } else {
            (
                json!([255, 0, 1]),
                json!([
                    0x1f00_0006u32,
                    0x1f00_0034u32,
                    0x1f00_0096u32,
                    0x1f00_00acu32,
                    0x1f00_0054u32,
                    0x1f00_009au32
                ]),
            )
        };
        ensure!(
            row["flags_raw"] == flags && row["subtype_allowlist"] == allowlist,
            "registration flags or subtype allowlist differ from the declared source snapshot"
        );
    }
    Ok(())
}
