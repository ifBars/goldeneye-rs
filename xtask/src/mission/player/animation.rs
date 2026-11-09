//! Explicit host mode plans remain separate from real animator execution.
use super::super::{closed, number};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(crate) fn validate(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "input_source",
            "requested_mode",
            "previous_mode",
            "fallback_mode",
            "target_index",
            "return_word",
            "attempts",
            "matched_transitions",
            "planned_commands",
            "mask_operations",
            "start_sets",
            "layer_groups",
            "child_requests",
            "previous_matches",
            "fallback_matches",
            "plan_sha256",
            "plan_encoding",
            "initial_request_verified",
            "runtime_children_created",
            "animation_sampled",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    for field in [
        "requested_mode",
        "previous_mode",
        "fallback_mode",
        "return_word",
    ] {
        ensure!(
            number(value, field)? <= u64::from(u32::MAX),
            "mode input/return outside word width"
        );
    }
    ensure!(
        value["input_source"] == "explicit_host_state"
            && value["plan_encoding"] == "mode_plan_be_v1"
            && value["initial_request_verified"] == false
            && value["runtime_children_created"] == false
            && value["animation_sampled"] == false
            && value["camera_frame_ready"] == false,
        "mode plan invented initial state or animator/camera readiness"
    );
    let attempts = number(value, "attempts")?;
    let matched = number(value, "matched_transitions")?;
    ensure!(
        attempts <= 2
            && matched <= attempts
            && matched == number(value, "previous_matches")? + number(value, "fallback_matches")?
            && number(value, "previous_matches")? <= 1
            && number(value, "fallback_matches")? <= 1
            && number(value, "planned_commands")? <= 4096
            && number(value, "planned_commands")?
                == number(value, "mask_operations")? + number(value, "start_sets")?
            && number(value, "layer_groups")? <= 4096
            && number(value, "child_requests")? <= 16_384,
        "mode plan counters are inconsistent or exceed source planning bounds"
    );
    if value["target_index"].is_null() {
        ensure!(
            value["state"] == "target_missing"
                && attempts == 0
                && matched == 0
                && value["return_word"] == 0
                && value["planned_commands"] == 0
                && value["layer_groups"] == 0
                && value["child_requests"] == 0,
            "missing target fabricated transition effects"
        );
    } else {
        ensure!(
            number(value, "target_index")? < 32768
                && attempts > 0
                && if value["return_word"] == 0 {
                    value["state"] == "no_nonzero_return"
                } else {
                    value["state"] == "planned" && matched > 0
                },
            "mode selection status no longer matches its return word"
        );
    }
    ensure!(
        value["plan_sha256"]
            .as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            && value["scope"]
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "mode plan digest/scope missing"
    );
    Ok(())
}
