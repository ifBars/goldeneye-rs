use super::{closed, count};
use anyhow::{Result, ensure};
use serde_json::Value;
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn word(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX))
}
fn lookup(v: &Value) -> Result<()> {
    closed(
        v,
        &[
            "outcome",
            "directory_index",
            "attribute_present",
            "state_sha256",
        ],
    )?;
    ensure!(
        hash(&v["state_sha256"]),
        "suffix mask lookup digest differs"
    );
    match v["outcome"].as_str() {
        Some("resident") => ensure!(
            word(&v["directory_index"]) && v["attribute_present"].is_boolean(),
            "resident suffix mask metadata is missing"
        ),
        Some("missing_descriptor" | "missing_body" | "pending") => ensure!(
            v["directory_index"].is_null() && v["attribute_present"].is_null(),
            "unreached suffix mask metadata was invented"
        ),
        _ => anyhow::bail!("unsupported suffix mask lookup outcome"),
    }
    Ok(())
}
fn helper(h: &Value, target: &str) -> Result<()> {
    closed(
        h,
        &[
            "target",
            "receiver_searched",
            "receiver_bound",
            "lookups",
            "applied_slots",
            "returned",
            "state_sha256",
        ],
    )?;
    let rows = h["lookups"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| anyhow::anyhow!("suffix mask lookups must retain two ordered positions"))?;
    let slots = h["applied_slots"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("suffix applied slots absent"))?;
    let expected = if target == "first_person" {
        &[0u64, 1, 2][..]
    } else {
        &[0u64, 1, 2, 3][..]
    };
    ensure!(
        h["target"] == target
            && ["receiver_searched", "receiver_bound", "returned"]
                .iter()
                .all(|k| h[k].is_boolean())
            && hash(&h["state_sha256"])
            && slots.len() <= expected.len()
            && slots
                .iter()
                .zip(expected)
                .all(|(v, e)| v.as_u64() == Some(*e)),
        "suffix helper receiver, slot order or digest differs"
    );
    for row in rows {
        if !row.is_null() {
            lookup(row)?;
        }
    }
    if h["receiver_searched"] == false {
        ensure!(
            h["receiver_bound"] == false
                && h["returned"] == false
                && rows.iter().all(Value::is_null)
                && slots.is_empty(),
            "unsearched suffix helper claims receiver/mask effects"
        );
        return Ok(());
    }
    if h["receiver_bound"] == false {
        ensure!(
            h["returned"] == true && rows.iter().all(Value::is_null) && slots.is_empty(),
            "missing suffix receiver claims mask reads or an unreturned helper"
        );
        return Ok(());
    }
    let first = rows[0]["outcome"].as_str();
    let second = rows[1]["outcome"].as_str();
    match first {
        None => ensure!(
            rows[0].is_null() && rows[1].is_null() && h["returned"] == false && slots.is_empty(),
            "second suffix mask reached before first"
        ),
        Some("missing_descriptor" | "missing_body") => ensure!(
            rows[1].is_null() && h["returned"] == true && slots.is_empty(),
            "absent first suffix mask did not take its early return"
        ),
        Some("pending") => ensure!(
            rows[1].is_null() && h["returned"] == false && slots.is_empty(),
            "pending first suffix mask became absence or success"
        ),
        Some("resident") => match second {
            None => ensure!(
                rows[1].is_null() && h["returned"] == false && slots.is_empty(),
                "unread second suffix mask claims effects"
            ),
            Some("missing_descriptor" | "missing_body") => ensure!(
                h["returned"] == true && slots.is_empty(),
                "absent second suffix mask claims writes"
            ),
            Some("pending") => ensure!(
                h["returned"] == false && slots.is_empty(),
                "pending second suffix mask became success"
            ),
            Some("resident") => ensure!(
                h["returned"] != true || slots.len() == expected.len(),
                "suffix mask helper returned before all physical slot writes"
            ),
            _ => anyhow::bail!("suffix second mask outcome differs"),
        },
        _ => anyhow::bail!("suffix first mask outcome differs"),
    }
    Ok(())
}
pub(super) fn validate(s: &Value, action: &Value) -> Result<()> {
    if s.is_null() {
        ensure!(
            action["action_invocation_returned"] == false
                && action["stage"] != "action_dispatch_repeat_pending",
            "returned first invocation has no suffix proof"
        );
        return Ok(());
    }
    closed(
        s,
        &[
            "stage",
            "classification",
            "mask_helpers",
            "parameter_visits",
            "parameter_writes",
            "pending_dependency",
            "repeat_generic",
            "repeat_parameter_bits",
            "state_sha256",
        ],
    )?;
    let helpers = s["mask_helpers"]
        .as_array()
        .filter(|v| v.len() == 2)
        .ok_or_else(|| anyhow::anyhow!("suffix must retain both ordered helper receipts"))?;
    helper(&helpers[0], "first_person")?;
    helper(&helpers[1], "third_person")?;
    ensure!(
        count(s, "classification")? <= 3
            && count(s, "parameter_visits")? <= 13
            && count(s, "parameter_writes")? <= count(s, "parameter_visits")?
            && hash(&s["state_sha256"])
            && action["fp_dispatch"]["stage"] == "returned",
        "suffix classification, parameters or FP caller return differs"
    );
    let returned = s["stage"] == "returned";
    ensure!(
        action["action_invocation_returned"] == returned
            && action["stage"]
                == if returned {
                    "action_dispatch_repeat_pending"
                } else {
                    "initial_action_suffix_pending"
                },
        "suffix and enclosing first-invocation return differ"
    );
    if !returned {
        ensure!(
            s["repeat_generic"].is_null() && s["repeat_parameter_bits"].is_null(),
            "repeat request prepared before first invocation returned"
        );
    }
    match s["stage"].as_str() {
        Some("first_person_masks") => ensure!(
            helpers[0]["returned"] == false
                && helpers[1]["receiver_searched"] == false
                && s["parameter_visits"] == 0
                && s["parameter_writes"] == 0
                && s["pending_dependency"] == "bone_masks",
            "FP suffix mask prefix skips source ordering"
        ),
        Some("third_person_masks") => ensure!(
            helpers[0]["returned"] == true
                && helpers[1]["returned"] == false
                && s["parameter_visits"] == 0
                && s["parameter_writes"] == 0
                && s["pending_dependency"] == "bone_masks",
            "TP suffix mask prefix skips FP helper return"
        ),
        Some("first_person_parameters") => ensure!(
            helpers.iter().all(|h| h["returned"] == true)
                && count(s, "parameter_visits")? <= 3
                && matches!(
                    s["pending_dependency"].as_str(),
                    Some("action_parameters" | "fp_secondary_speed_parameters")
                ),
            "FP suffix parameters precede masks or invent a dependency"
        ),
        Some("third_person_parameters") => ensure!(
            helpers.iter().all(|h| h["returned"] == true)
                && matches!(
                    s["pending_dependency"].as_str(),
                    Some("action_parameters" | "tp_primary_mode_parameters")
                ),
            "TP suffix parameter dependency differs"
        ),
        Some("returned") => {
            ensure!(
                helpers.iter().all(|h| h["returned"] == true) && s["pending_dependency"].is_null(),
                "suffix return skipped a mask helper or pending provider"
            );
            if s["repeat_generic"].is_null() {
                ensure!(
                    s["repeat_parameter_bits"].is_null(),
                    "repeat argument exists without its request"
                );
            } else {
                ensure!(
                    matches!(
                        s["repeat_generic"].as_u64(),
                        Some(0x0900_0765 | 0x0900_07a7)
                    ) && s["repeat_parameter_bits"] == 0x7f7f_ffffu32,
                    "repeat request changed source generic/FLT_MAX argument"
                );
            }
        }
        _ => anyhow::bail!("unsupported action suffix boundary"),
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
