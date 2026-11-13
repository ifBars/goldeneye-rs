use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;

fn word(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX))
}
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub(super) fn validate(t: &Value, component: &Value) -> Result<()> {
    if t.is_null() {
        ensure!(
            !matches!(
                component["stage"].as_str(),
                Some("player_constructor_tail_pending" | "player_pose_setup_pending")
            ),
            "constructor continuation precedes callback return"
        );
        return Ok(());
    }
    closed(
        t,
        &[
            "stage",
            "logging_noop_returned",
            "e59_raw",
            "initial_state_704",
            "state_704",
            "physics_outcome",
            "physics_flags_before",
            "physics_flags_after",
            "word_708",
            "flags_220_before",
            "flags_220_after",
            "callback_returned",
            "state_sha256",
        ],
    )?;
    let stage = match t["stage"].as_str() {
        Some("logging") => 0,
        Some("derived_timer") => 1,
        Some("state_704") => 2,
        Some("physics") => 3,
        Some("final_flags") => 4,
        Some("returned") => 5,
        _ => anyhow::bail!("unsupported callback tail boundary"),
    };
    ensure!(
        component["initial_action"]["action_invocation_returned"] == true
            && component["initial_action"]["callback_returned"] == false
            && component["repeat_action"]["action_invocation_returned"] == true
            && component["repeat_action"]["callback_returned"] == false
            && component["repeat_action"]["suffix"]["stage"] == "returned"
            && t["logging_noop_returned"] == (stage > 0)
            && t["callback_returned"] == (stage == 5)
            && hash(&t["state_sha256"])
            && if stage == 5 {
                matches!(
                    component["stage"].as_str(),
                    Some("player_constructor_tail_pending" | "player_pose_setup_pending")
                )
            } else {
                component["stage"] == "player_callback_tail_pending"
            },
        "callback tail caller, logging return or current boundary differs"
    );
    if stage >= 2 {
        ensure!(
            t["e59_raw"].as_u64().is_some_and(|n| n <= 255),
            "callback timer return lost its branch byte"
        );
    } else {
        ensure!(
            t["e59_raw"].is_null(),
            "callback timer effects precede its source call"
        );
    }
    if stage >= 3 {
        ensure!(
            (t["initial_state_704"].is_null() || word(&t["initial_state_704"]))
                && t["state_704"] == if t["e59_raw"] == 0 { 2 } else { 3 },
            "callback state704 differs from its actual branch byte"
        );
    } else {
        ensure!(
            t["initial_state_704"].is_null() && t["state_704"].is_null(),
            "callback state704 effects precede timer return"
        );
    }
    if stage >= 4 {
        match t["physics_outcome"].as_str() {
            Some("absent") => ensure!(
                t["physics_flags_before"].is_null() && t["physics_flags_after"].is_null(),
                "absent callback physics claims flag stores"
            ),
            Some("flags_cleared") => ensure!(
                count(t, "physics_flags_before")? <= 65535
                    && count(t, "physics_flags_after")?
                        == count(t, "physics_flags_before")? & 0xfcfc,
                "callback physics clear width/mask differs"
            ),
            _ => anyhow::bail!("callback physics helper has not returned"),
        }
    } else {
        ensure!(
            t["physics_outcome"].is_null()
                && t["physics_flags_before"].is_null()
                && t["physics_flags_after"].is_null(),
            "callback physics effects precede the state704 store"
        );
    }
    if stage == 5 {
        ensure!(
            t["word_708"] == 1
                && word(&t["flags_220_before"])
                && word(&t["flags_220_after"])
                && count(t, "flags_220_after")? == count(t, "flags_220_before")? | 1,
            "callback final flags or708 store differ"
        );
    } else {
        ensure!(
            t["word_708"].is_null()
                && t["flags_220_before"].is_null()
                && t["flags_220_after"].is_null(),
            "callback final stores precede source return"
        );
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
