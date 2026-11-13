use super::{closed, count, player_selector, player_suffix};
use anyhow::{Result, ensure};
use serde_json::Value;

fn word(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX))
}
fn optional_word(v: &Value) -> bool {
    v.is_null() || word(v)
}
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn command(c: &Value, acknowledged: bool) -> Result<()> {
    closed(
        c,
        &[
            "stage",
            "opcode",
            "set_reference",
            "set_index",
            "set_groups",
            "groups_visited",
            "weight_writes",
            "mask",
            "pending_slot",
            "child_reference",
            "children_created",
            "children_completed",
            "children_inserted",
            "head_copies",
            "effective_flags_or",
            "command_returned",
            "selector_acknowledged",
            "state_sha256",
        ],
    )?;
    let created = count(c, "children_created")?;
    let completed = count(c, "children_completed")?;
    let inserted = count(c, "children_inserted")?;
    let groups = count(c, "groups_visited")?;
    ensure!(
        word(&c["opcode"])
            && [
                "set_reference",
                "set_index",
                "set_groups",
                "child_reference"
            ]
            .iter()
            .all(|k| optional_word(&c[k]))
            && groups <= 4096
            && (!c["set_groups"].is_null() || groups == 0)
            && count(c, "weight_writes")? <= groups
            && created <= 16384
            && completed <= inserted
            && inserted <= created
            && count(c, "head_copies")? <= created
            && (c["set_groups"].is_null()
                || (count(c, "set_groups")? <= 4096 && groups <= count(c, "set_groups")?))
            && (c["mask"].is_null() || c["mask"].as_u64().is_some_and(|n| n <= 65535))
            && (c["pending_slot"].is_null() || c["pending_slot"].as_u64().is_some_and(|n| n < 4))
            && c["pending_slot"].is_null() == c["child_reference"].is_null()
            && count(c, "effective_flags_or")? <= 65535
            && c["command_returned"].is_boolean()
            && c["selector_acknowledged"] == acknowledged
            && c["command_returned"] == (c["stage"] == "returned")
            && (!acknowledged || c["command_returned"] == true)
            && hash(&c["state_sha256"]),
        "repeat command trace counters or acknowledgement differ"
    );
    ensure!(
        matches!(
            c["stage"].as_str(),
            Some(
                "entry"
                    | "child_factory_pending"
                    | "cleanup_pending"
                    | "mask_control_pending"
                    | "mask_clear_pending"
                    | "mask_refresh_pending"
                    | "mask_backing_pending"
                    | "mask_effects_pending"
                    | "mask_destroy_pending"
                    | "opcode_pending"
                    | "child_wrapper_pending"
                    | "child_continuation_pending"
                    | "returned"
            )
        ),
        "unsupported repeat command boundary"
    );
    if c["command_returned"] == true {
        ensure!(
            c["pending_slot"].is_null() && created == completed && inserted == completed,
            "returned repeat command retains incomplete child effects"
        );
    }
    if c["mask"].is_null() {
        ensure!(
            !c["stage"].as_str().is_some_and(|s| s.starts_with("mask_")),
            "repeat mask boundary lost its operand"
        );
    } else {
        ensure!(
            c["opcode"] == 0x0c00_0005u32
                && c["set_reference"].is_null()
                && c["set_index"].is_null()
                && c["set_groups"].is_null()
                && groups == 0
                && created == 0
                && c["effective_flags_or"] == 0,
            "mask trace claims set-command effects"
        );
    }
    Ok(())
}
fn call(c: &Value, r: &Value, ordinal: usize) -> Result<()> {
    closed(
        c,
        &[
            "target",
            "component_file",
            "generic_request",
            "requested",
            "parameter_bits",
            "tracks_player_modes",
            "fallback",
            "fallback_writes",
            "previous",
            "stage",
            "selector",
            "completed_commands",
            "pending_command",
            "setter_return_word",
            "history_updated",
            "state_sha256",
        ],
    )?;
    let (target, requested, file) = match ordinal {
        0 => ("third_person", &r["tp_mapped"], 0x0102_0017u32),
        1 => (
            "third_person_secondary",
            &r["tp_secondary_mapped"],
            0x0102_0017u32,
        ),
        2 => ("first_person", &r["fp_mapped"], 0x0102_0016u32),
        _ => anyhow::bail!("repeat action has more than three setters"),
    };
    ensure!(
        c["target"] == target
            && c["component_file"] == file
            && c["generic_request"] == r["generic_request"]
            && c["requested"] == *requested
            && c["parameter_bits"] == r["parameter_bits"]
            && c["tracks_player_modes"].is_boolean()
            && c["history_updated"].is_boolean()
            && word(&c["fallback"])
            && count(c, "fallback_writes")? > 0
            && word(&c["previous"])
            && hash(&c["state_sha256"])
            && c["selector"]["tracks_player_modes"] == c["tracks_player_modes"],
        "repeat setter receiver/input differs"
    );
    let pending = c["stage"] == "command_pending";
    player_selector::validate_repeated(
        &c["selector"],
        requested,
        &c["fallback"],
        &c["previous"],
        pending,
    )?;
    let completed = c["completed_commands"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("repeat completed command traces absent"))?;
    ensure!(
        completed.len() <= 4096
            && completed.len() as u64 == count(&c["selector"], "commands_executed")?,
        "repeat command history count differs from actual selector acknowledgements"
    );
    for body in completed {
        command(body, true)?;
    }
    if !c["pending_command"].is_null() {
        ensure!(
            pending,
            "nonpending repeat setter owns an unacknowledged body"
        );
        command(&c["pending_command"], false)?;
        ensure!(
            c["pending_command"]["opcode"] == c["selector"]["pending_opcode"],
            "repeat pending body differs from issued opcode"
        );
    }
    match c["stage"].as_str() {
        Some("selecting") => ensure!(
            c["selector"]["stage"] == "selecting",
            "repeat selecting setter claims selector return"
        ),
        Some("command_pending") => ensure!(
            c["selector"]["stage"] == "command_pending",
            "repeat body has no pending cursor"
        ),
        Some("setter_tail_pending" | "returned") => ensure!(
            c["selector"]["stage"] == "returned",
            "repeat setter tail skipped selector return"
        ),
        _ => anyhow::bail!("unsupported repeat setter boundary"),
    }
    if c["stage"] == "returned" {
        ensure!(
            word(&c["setter_return_word"])
                && c["setter_return_word"] == c["selector"]["return_word"]
                && c["history_updated"]
                    == (c["tracks_player_modes"] == true && c["setter_return_word"] != 0),
            "repeat setter history differs from tracked nonzero return gate"
        );
    } else {
        ensure!(
            c["setter_return_word"].is_null() && c["history_updated"] == false,
            "repeat setter effects precede return"
        );
    }
    Ok(())
}
fn dispatch(d: &Value, r: &Value, component: &Value) -> Result<()> {
    closed(
        d,
        &[
            "phase",
            "captured_position_bits",
            "captured_head_present",
            "captured_matched",
            "tp_coordinate_written",
            "seek_bits",
            "seek_node_present",
            "seek_returned",
            "override_writes",
            "fp_coordinate_bits",
            "fp_coordinate_written",
            "tp_active_slots",
            "fp_active_slots",
            "calls",
            "state_sha256",
        ],
    )?;
    let phase = match d["phase"].as_str() {
        Some("first_third_person") => 0,
        Some("third_person_coordinate") => 1,
        Some("third_person_seek") => 2,
        Some("enabling_override") => 3,
        Some("second_third_person") => 4,
        Some("disabling_override") => 5,
        Some("first_person") => 6,
        Some("first_person_coordinate") => 7,
        Some("returned") => 8,
        _ => anyhow::bail!("unsupported repeat dispatch boundary"),
    };
    ensure!(
        word(&d["captured_position_bits"])
            && [
                "captured_head_present",
                "captured_matched",
                "seek_node_present",
                "seek_returned"
            ]
            .iter()
            .all(|k| d[k].is_boolean())
            && (d["captured_matched"] != true || d["captured_head_present"] == true)
            && (d["captured_matched"] == true || d["captured_position_bits"] == 0)
            && (d["tp_coordinate_written"].is_null() || d["tp_coordinate_written"].is_boolean())
            && (d["fp_coordinate_written"].is_null() || d["fp_coordinate_written"].is_boolean())
            && optional_word(&d["seek_bits"])
            && optional_word(&d["fp_coordinate_bits"])
            && (1..=4).contains(&count(d, "tp_active_slots")?)
            && (1..=4).contains(&count(d, "fp_active_slots")?)
            && d["tp_active_slots"] == component["active_slots"]
            && hash(&d["state_sha256"]),
        "repeat dispatch capture/current owner differs"
    );
    let calls = d["calls"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("repeat setter calls absent"))?;
    let (required, may_start) = match phase {
        0 => (0, true),
        1..=3 => (1, false),
        4 => (1, true),
        5 => (2, false),
        6 => (2, true),
        _ => (3, false),
    };
    ensure!(
        calls.len() >= required && calls.len() <= required + usize::from(may_start),
        "repeat setter count skips source order"
    );
    for (i, c) in calls.iter().enumerate() {
        call(c, r, i)?;
        let previous_writes = if i == 0 {
            count(
                &component["initial_action"]["fp_dispatch"],
                "fallback_writes",
            )?
        } else {
            count(&calls[i - 1], "fallback_writes")?
        };
        let writes = count(c, "fallback_writes")?;
        ensure!(
            if i == 1 {
                writes >= previous_writes
            } else {
                writes > previous_writes
            },
            "repeat fallback write ordinals precede their actual entry stores"
        );
        if i < required {
            ensure!(
                c["stage"] == "returned",
                "repeat dispatch advanced before previous setter returned"
            );
        }
    }
    ensure!(
        count(d, "override_writes")?
            == if phase <= 3 {
                0
            } else if phase <= 5 {
                1
            } else {
                2
            },
        "repeat override stores skip source order"
    );
    if phase < 2 {
        ensure!(
            d["tp_coordinate_written"].is_null()
                && d["seek_bits"].is_null()
                && d["seek_node_present"] == false
                && d["seek_returned"] == false,
            "repeat seek effects precede first setter return"
        );
    } else {
        let captured = u32::try_from(count(d, "captured_position_bits")?)?;
        ensure!(
            d["tp_coordinate_written"].is_boolean()
                && d["seek_bits"] == (0.25f32 * f32::from_bits(captured)).to_bits()
                && d["seek_returned"] == (phase >= 3),
            "repeat seek argument/return differs from captured position"
        );
    }
    ensure!(
        if phase == 8 {
            word(&d["fp_coordinate_bits"]) && d["fp_coordinate_written"].is_boolean()
        } else {
            d["fp_coordinate_bits"].is_null() && d["fp_coordinate_written"].is_null()
        },
        "repeat FP scalar helper claims premature or missing return"
    );
    Ok(())
}
pub(super) fn validate(r: &Value, component: &Value) -> Result<()> {
    let initial = &component["initial_action"];
    if r.is_null() {
        ensure!(
            component["stage"] == initial["stage"]
                && component["stage"] != "player_callback_tail_pending"
                && count(component, "active_slots")?
                    == initial["tp_command"]["active_slots"].as_u64().unwrap_or(1),
            "component stage or active slots differ without a repeated invocation"
        );
        return Ok(());
    }
    closed(
        r,
        &[
            "stage",
            "generic_request",
            "parameter_bits",
            "weapon_key",
            "variant",
            "remap_calls",
            "map_rows_read",
            "fp_mapped",
            "tp_mapped",
            "tp_secondary_mapped",
            "mapping_state_sha256",
            "dispatch",
            "suffix",
            "action_invocation_returned",
            "callback_returned",
            "state_sha256",
        ],
    )?;
    ensure!(
        initial["stage"] == "action_dispatch_repeat_pending"
            && initial["action_invocation_returned"] == true
            && initial["callback_returned"] == false
            && r["generic_request"] == initial["suffix"]["repeat_generic"]
            && r["parameter_bits"] == initial["suffix"]["repeat_parameter_bits"]
            && word(&r["generic_request"])
            && word(&r["parameter_bits"])
            && word(&r["weapon_key"])
            && count(r, "variant")? <= 1
            && r["remap_calls"] == 3
            && count(r, "map_rows_read")? <= 3 * 4096
            && ["fp_mapped", "tp_mapped", "tp_secondary_mapped"]
                .iter()
                .all(|k| word(&r[k]) && r[k] != 0)
            && r["callback_returned"] == false
            && r["action_invocation_returned"].is_boolean()
            && hash(&r["mapping_state_sha256"])
            && hash(&r["state_sha256"]),
        "repeated invocation differs from immutable first request or claims callback completion"
    );
    if !r["dispatch"].is_null() {
        dispatch(&r["dispatch"], r, component)?;
    }
    let returned = r["stage"] == "returned";
    ensure!(
        r["action_invocation_returned"] == returned
            && if returned {
                player_selector::at_least(&component["stage"], 1)
            } else {
                component["stage"] == "action_dispatch_repeat_pending"
            },
        "repeat invocation/root boundary differs"
    );
    match r["stage"].as_str() {
        Some("dispatch_pending") => ensure!(
            r["suffix"].is_null() && r["dispatch"]["phase"] != "returned",
            "repeat suffix began before dispatch return"
        ),
        Some("suffix_pending" | "returned") => ensure!(
            r["dispatch"]["phase"] == "returned",
            "repeat suffix skipped dispatch return"
        ),
        _ => anyhow::bail!("unsupported repeated invocation boundary"),
    }
    player_suffix::validate_repeated(&r["suffix"], r)?;
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
