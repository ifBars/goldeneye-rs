use super::{closed, count, player_command, player_mask, player_selector};
use anyhow::{Result, ensure};
use serde_json::Value;
pub(super) fn validate(s: &Value, action: &Value) -> Result<()> {
    if s.is_null() {
        ensure!(
            matches!(
                action["stage"].as_str(),
                Some(
                    "tp_command_pending"
                        | "tp_setter_tail_pending"
                        | "tp_dispatch_pending"
                        | "fp_dispatch_pending"
                )
            ),
            "FP dispatch receipt missing after entry"
        );
        return Ok(());
    }
    closed(
        s,
        &[
            "stage",
            "component_file",
            "qualification_skipped",
            "capture_performed",
            "initial_heads_empty",
            "fallback",
            "fallback_writes",
            "selector",
            "command",
            "mask_capture",
            "mask_commands_returned",
            "setter_return_word",
            "history_updated",
            "primary_changed",
            "coordinate_written",
            "state_sha256",
        ],
    )?;
    ensure!(
        s["component_file"] == 0x0102_0016u32
            && s["qualification_skipped"] == true
            && s["capture_performed"] == false
            && s["initial_heads_empty"] == true
            && s["fallback"] == 0x0900_065eu32
            && count(s, "fallback_writes")? > count(&action["tp_capture"], "fallback_writes")?
            && s["selector"]["tracks_player_modes"] == true
            && s["history_updated"].is_boolean()
            && (s["primary_changed"].is_null() || s["primary_changed"].is_boolean())
            && (s["coordinate_written"].is_null() || s["coordinate_written"].is_boolean())
            && s["state_sha256"].as_str().is_some_and(|h| h.len() == 64
                && h.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))),
        "FP receiver, qualification or fallback snapshot differs"
    );
    let pending = s["stage"] == "command_pending";
    player_selector::validate_for(
        &s["selector"],
        &action["fp_mapped"],
        &s["fallback"],
        pending,
    )?;
    player_command::validate_for(&s["command"], &s["selector"], pending)?;
    let masks = count(s, "mask_commands_returned")?;
    let executed = count(&s["selector"], "commands_executed")?;
    ensure!(
        masks <= executed,
        "FP returned mask count exceeds acknowledged commands"
    );
    if s["command"]["selector_acknowledged"] == true {
        if s["command"]["opcode"] == 0x0c00_0005u32 {
            ensure!(masks > 0, "returned mask command is missing from history");
            if s["command"]["mask"] != 0 {
                ensure!(
                    s["mask_capture"] == s["command"]["mask_effects"],
                    "latest returned nonzero mask capture disappeared"
                );
            }
        } else {
            ensure!(
                masks < executed,
                "mask history overwrote the latest returned set command"
            );
        }
    }
    if !s["mask_capture"].is_null() {
        let capture = &s["mask_capture"];
        let raw = count(capture, "mask_raw")?;
        ensure!(
            masks > 0 && raw > 0 && raw <= 65535 && capture["capture_returned"] == true,
            "historical mask capture has no returned nonzero command"
        );
        player_mask::validate(
            capture,
            "command_returned",
            raw,
            count(capture, "active_slots_before")?,
        )?;
    }
    for refresh in [
        &s["command"]["mask_effects"]["refresh"],
        &s["mask_capture"]["refresh"],
    ] {
        if !refresh.is_null() {
            ensure!(
                count(refresh, "refresh_ordinal")?
                    > count(&action["tp_capture"]["refresh"], "refresh_ordinal")?
                    && count(refresh, "allocation_ordinal")?
                        > count(&action["tp_capture"]["refresh"], "allocation_ordinal")?,
                "FP mask refresh does not follow the retained TP capture"
            );
        }
    }
    let parent = match s["stage"].as_str() {
        Some("command_pending") => "fp_command_pending",
        Some("setter_tail_pending") => "fp_setter_tail_pending",
        Some("dispatch_tail_pending") => "fp_dispatch_tail_pending",
        Some("returned") => "initial_action_suffix_pending",
        _ => anyhow::bail!("unsupported FP dispatcher boundary"),
    };
    ensure!(
        action["stage"] == parent
            || (s["stage"] == "returned" && action["stage"] == "action_dispatch_repeat_pending"),
        "FP dispatcher parent dependency differs"
    );
    let setter = matches!(
        s["stage"].as_str(),
        Some("dispatch_tail_pending" | "returned")
    );
    if setter {
        ensure!(
            s["setter_return_word"]
                .as_u64()
                .is_some_and(|v| v <= u64::from(u32::MAX))
                && s["setter_return_word"] == s["selector"]["return_word"],
            "FP setter skipped selector return"
        );
        let writes = s["setter_return_word"] != 0;
        ensure!(
            s["history_updated"] == writes
                && if writes {
                    s["primary_changed"].is_boolean()
                } else {
                    s["primary_changed"].is_null()
                },
            "FP tracked setter history branch differs"
        );
    } else {
        ensure!(
            s["setter_return_word"].is_null()
                && s["history_updated"] == false
                && s["primary_changed"].is_null(),
            "FP setter effects precede its return"
        );
    }
    ensure!(
        if s["stage"] == "returned" {
            s["coordinate_written"].is_boolean()
        } else {
            s["coordinate_written"].is_null()
        },
        "FP dispatch helper was skipped or claimed too early"
    );
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
