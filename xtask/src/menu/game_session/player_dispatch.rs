use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

const COUNTERS: &[&str] = &[
    "scalar_visits",
    "scalar_writes",
    "updates_started",
    "updates_completed",
    "script_invocations",
    "rate_queries",
    "rate_returns",
    "clip_reuses",
    "position_returns",
];
const DEPENDENCIES: &[&str] = &[
    "clip_rate",
    "script_selection",
    "clip_position",
    "animation_owner",
    "clip_cache",
    "animation_body",
    "timing_curve",
    "root_motion",
    "clip_file",
    "clip_range",
    "frame_count",
    "clip_replacement",
];
fn finite_word(value: &Value) -> bool {
    value
        .as_u64()
        .is_some_and(|v| v <= u64::from(u32::MAX) && f32::from_bits(v as u32).is_finite())
}
pub(super) fn validate(s: &Value, action: &Value) -> Result<()> {
    closed(
        s,
        &[
            "stage",
            "captured_position_bits",
            "setter_return_word",
            "history_updated",
            "primary_changed",
            "scalar_visits",
            "scalar_writes",
            "updates_started",
            "updates_completed",
            "pending_slot",
            "script_invocations",
            "rate_queries",
            "rate_returns",
            "clip_reuses",
            "position_returns",
            "pending_dependency",
            "rate_bits",
            "state_sha256",
        ],
    )?;
    let rates = s["rate_bits"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| anyhow::anyhow!("TP dispatch rate slots differ"))?;
    ensure!(
        s["captured_position_bits"] == 0
            && s["history_updated"].is_boolean()
            && (s["primary_changed"].is_null() || s["primary_changed"].is_boolean())
            && count(s, "scalar_visits")? <= 3
            && count(s, "scalar_writes")? <= count(s, "scalar_visits")?
            && count(s, "updates_started")? <= 2
            && count(s, "updates_completed")? <= count(s, "updates_started")?
            && count(s, "script_invocations")? <= count(s, "updates_started")?
            && count(s, "rate_queries")? <= count(s, "script_invocations")?
            && count(s, "rate_returns")? <= count(s, "rate_queries")?
            && count(s, "clip_reuses")? <= count(s, "rate_returns")?
            && count(s, "position_returns")? <= count(s, "rate_returns")?
            && rates.iter().all(|v| v.is_null() || finite_word(v))
            && rates.iter().filter(|v| !v.is_null()).count() as u64 == count(s, "rate_returns")?
            && s["state_sha256"].as_str().is_some_and(|h| h.len() == 64
                && h.bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))),
        "TP dispatch counters, rate or history evidence differs"
    );
    if s["stage"] == "awaiting_setter" {
        ensure!(
            matches!(
                action["stage"].as_str(),
                Some("tp_command_pending" | "tp_setter_tail_pending")
            ) && s["setter_return_word"].is_null()
                && s["history_updated"] == false
                && s["primary_changed"].is_null()
                && s["pending_slot"].is_null()
                && s["pending_dependency"].is_null()
                && COUNTERS.iter().all(|k| s[k] == 0)
                && rates.iter().all(Value::is_null),
            "TP dispatch claims work before setter return"
        );
        return Ok(());
    }
    ensure!(
        s["setter_return_word"]
            .as_u64()
            .is_some_and(|v| v <= u64::from(u32::MAX))
            && s["setter_return_word"] == action["tp_selector"]["return_word"]
            && action["tp_selector"]["stage"] == "returned"
            && count(s, "scalar_visits")? == 3,
        "TP dispatch precedes actual selector/setter return"
    );
    let writes =
        action["tp_selector"]["tracks_player_modes"] == true && s["setter_return_word"] != 0;
    ensure!(
        s["history_updated"] == writes
            && if writes {
                s["primary_changed"].is_boolean()
            } else {
                s["primary_changed"].is_null()
            },
        "TP dispatch history skip/write branch differs"
    );
    match s["stage"].as_str() {
        Some("script_update_pending") => ensure!(
            action["stage"] == "tp_dispatch_pending"
                && count(s, "updates_started")? == count(s, "updates_completed")? + 1
                && s["pending_slot"]
                    == if count(s, "updates_completed")? == 0 {
                        2
                    } else {
                        3
                    }
                && count(s, "script_invocations")? > 0
                && s["pending_dependency"]
                    .as_str()
                    .is_some_and(|v| DEPENDENCIES.contains(&v))
                && if s["pending_slot"] == 2 {
                    rates[1].is_null()
                } else {
                    true
                },
            "TP pending script update claims a dispatcher return or skips a slot"
        ),
        Some("returned") => ensure!(
            matches!(
                action["stage"].as_str(),
                Some(
                    "fp_dispatch_pending"
                        | "fp_command_pending"
                        | "fp_setter_tail_pending"
                        | "fp_dispatch_tail_pending"
                        | "initial_action_suffix_pending"
                )
            ) && s["updates_started"] == 2
                && s["updates_completed"] == 2
                && s["pending_slot"].is_null()
                && s["pending_dependency"].is_null(),
            "TP dispatcher return skips updates or claims a pending dependency"
        ),
        _ => anyhow::bail!("unsupported TP dispatcher boundary"),
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
