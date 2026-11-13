use super::{closed, count, player_refresh};
use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn validate(e: &Value, stage: &str, mask: u64, active: u64) -> Result<()> {
    ensure!(mask <= 65535, "mask operand exceeds native width");
    if matches!(stage, "mask_control_pending" | "mask_clear_pending") {
        ensure!(
            e.is_null() && (mask == 0) == (stage == "mask_clear_pending"),
            "mask helper effects claimed before entry"
        );
        return Ok(());
    }
    closed(
        e,
        &[
            "mask_raw",
            "active_slots_before",
            "slots_visited",
            "selected_mask",
            "clear_mask",
            "controls_allocated",
            "controls_inserted",
            "capture_returned",
            "refresh",
            "state_sha256",
        ],
    )?;
    let before = e["active_slots_before"].as_u64();
    let selected = e["selected_mask"].as_u64();
    let clear = e["clear_mask"].as_u64();
    let visited = count(e, "slots_visited")?;
    let allocated = count(e, "controls_allocated")?;
    let inserted = count(e, "controls_inserted")?;
    ensure!(
        e["mask_raw"] == mask
            && (e["active_slots_before"].is_null()
                || before.is_some_and(|v| v <= 4 && v == active))
            && (e["selected_mask"].is_null()
                || selected.is_some_and(|v| v <= 255 && v & mask == v))
            && (e["clear_mask"].is_null() || clear.is_some_and(|v| v <= 255))
            && visited <= before.unwrap_or(0)
            && allocated <= 4
            && inserted <= visited
            && inserted <= allocated
            && e["capture_returned"].is_boolean()
            && e["state_sha256"].as_str().is_some_and(|h| h.len() == 64
                && h.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))
            && (before.is_none() || selected.is_some())
            && (visited == 0) == clear.is_none(),
        "mask loop counts or source-read prefix differs"
    );
    if !e["refresh"].is_null() {
        player_refresh::validate(&e["refresh"])?;
    }
    if mask == 0 {
        ensure!(
            allocated == 0
                && inserted == 0
                && e["capture_returned"] == false
                && e["refresh"].is_null(),
            "clear-only mask invented a control capture"
        );
    } else {
        ensure!(
            allocated >= 1,
            "nonzero mask lost its original control allocation"
        );
        if e["capture_returned"] == true {
            ensure!(
                !e["refresh"].is_null(),
                "mask capture has no returned refresh"
            );
        }
        if selected.is_some() {
            ensure!(
                e["capture_returned"] == true,
                "mask selection read before capture return"
            );
        }
    }
    match stage {
        "mask_refresh_pending" => ensure!(
            mask != 0
                && allocated == 1
                && e["refresh"].is_null()
                && e["capture_returned"] == false
                && before.is_none()
                && selected.is_none()
                && visited == 0
                && inserted == 0,
            "pending mask refresh claims later effects"
        ),
        "mask_backing_pending" => ensure!(
            mask != 0
                && allocated == 1
                && !e["refresh"].is_null()
                && e["capture_returned"] == false
                && before.is_none()
                && selected.is_none()
                && visited == 0
                && inserted == 0,
            "mask backing boundary skips refresh or claims capture"
        ),
        "mask_effects_pending" => ensure!(
            (mask == 0 || e["capture_returned"] == true)
                && before.is_none()
                && selected.is_none()
                && visited == 0
                && inserted == 0,
            "pending mask loop claims reached source reads"
        ),
        "mask_destroy_pending" => ensure!(
            before.is_some()
                && selected.is_some()
                && visited > 0
                && clear.is_some_and(|bits| bits & (1 << (visited - 1)) != 0),
            "mask destruction dependency skipped a source read"
        ),
        "command_returned" => ensure!(
            before.is_some()
                && selected.is_some()
                && visited == before.unwrap_or(0)
                && inserted
                    == u64::from((selected.unwrap_or(0) & ((1 << visited) - 1)).count_ones())
                && if mask == 0 {
                    true
                } else {
                    e["capture_returned"] == true && allocated == inserted.max(1)
                },
            "mask return skips capture, allocation or active-slot loop"
        ),
        _ => anyhow::bail!("unsupported mask effect boundary"),
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
