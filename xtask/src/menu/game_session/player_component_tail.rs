use super::{closed, count, player_selector};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
pub(super) const STAGES: [&str; 14] = [
    "rotation_second",
    "rotation_first",
    "store_rotation_first",
    "store_rotation_second",
    "rotation_invalidate",
    "character",
    "character_10",
    "character_14",
    "session_scale",
    "scale_first",
    "scale_second",
    "scale_invalidate",
    "flags",
    "returned",
];
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub(super) fn validate(t: &Value, c: &Value) -> Result<()> {
    if t.is_null() {
        ensure!(
            !player_selector::at_least(&c["stage"], 5),
            "initializer advanced before outer component-helper return"
        );
        return Ok(());
    }
    closed(
        t,
        &[
            "stage",
            "pending_dependency",
            "helper_return",
            "helper_returned",
            "rotation_reads",
            "rotation_stores",
            "rotation_invalidated",
            "character_outcome",
            "character_scalars_read",
            "character_scale_ready",
            "session_multiple",
            "scale_stores",
            "scale_invalidated",
            "flags_before",
            "flags_after",
            "state_sha256",
        ],
    )?;
    let stage = STAGES
        .iter()
        .position(|s| t["stage"] == *s)
        .context("unsupported outer component-helper stage")?;
    ensure!(
        c["pose_setup"]["stage"] == "returned"
            && c["pose_setup"]["helper_returned"] == true
            && t["helper_returned"] == (stage == 13)
            && t["helper_return"]
                == if stage == 13 {
                    Value::from(1)
                } else {
                    Value::Null
                }
            && t["rotation_reads"] == serde_json::json!([stage >= 2, stage >= 1])
            && t["rotation_stores"] == serde_json::json!([stage >= 3, stage >= 4])
            && t["rotation_invalidated"] == (stage >= 5)
            && hash(&t["state_sha256"])
            && if stage == 13 {
                player_selector::at_least(&c["stage"], 5)
            } else {
                c["stage"] == "player_component_tail_pending"
            },
        "outer component-helper order, nested caller or return scope differs"
    );
    if t["character_outcome"] == "known_absent" {
        ensure!(
            stage == 13
                && t["character_scalars_read"] == 0
                && t["character_scale_ready"] == false
                && t["session_multiple"].is_null()
                && t["scale_stores"] == serde_json::json!([false, false])
                && t["scale_invalidated"] == false
                && t["flags_before"].is_null()
                && t["flags_after"].is_null()
                && t["pending_dependency"].is_null(),
            "known absent character branch claims later scalar/scale/flag effects"
        );
        return Ok(());
    }
    ensure!(
        if stage >= 6 {
            t["character_outcome"] == "bound"
        } else {
            t["character_outcome"].is_null()
        },
        "outer character outcome is unknown, defaulted or reached too early"
    );
    ensure!(
        count(t, "character_scalars_read")?
            == if stage < 7 {
                0
            } else if stage == 7 {
                1
            } else {
                2
            }
            && t["character_scale_ready"] == (stage >= 8)
            && t["scale_stores"] == serde_json::json!([stage >= 10, stage >= 11])
            && t["scale_invalidated"] == (stage >= 12),
        "outer character scalar or paired scale stores skip source ordering"
    );
    if stage < 8 {
        ensure!(
            t["session_multiple"].is_null(),
            "outer session predicate read too early"
        );
    }
    if stage == 8 {
        ensure!(
            t["session_multiple"].is_null() || t["session_multiple"].is_boolean(),
            "outer session predicate has wrong type"
        );
    }
    if stage > 8 {
        ensure!(
            t["session_multiple"] == false,
            "outer helper skipped a multiplayer scale provider"
        );
    }
    if stage == 13 {
        ensure!(
            count(t, "flags_before")? <= 255
                && count(t, "flags_after")? == (count(t, "flags_before")? | 3),
            "outer actual flag OR width differs"
        );
    } else {
        ensure!(
            t["flags_before"].is_null() && t["flags_after"].is_null(),
            "outer flag store precedes both scale pairs and invalidation"
        );
    }
    ensure!(
        t["pending_dependency"].is_null()
            || (stage == 8
                && t["pending_dependency"] == "component_tail_multiplayer_scale"
                && t["session_multiple"] == true),
        "outer component-helper dependency differs"
    );
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
