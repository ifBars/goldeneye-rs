use super::{closed, count};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
pub(super) const STAGES: [&str; 19] = [
    "restore_state",
    "select_script",
    "current_record",
    "variant",
    "cleanup_key",
    "first_component",
    "primary_key",
    "body_cleanup",
    "read_cleanup_link",
    "child_cleanup",
    "marker",
    "actor_gate",
    "first_override",
    "second_override",
    "first_effect",
    "second_effect",
    "helper_return",
    "pose_input",
    "awaiting_pose",
];
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
            !super::player_selector::at_least(&component["stage"], 3),
            "pose setup precedes constructor helper return"
        );
        return Ok(());
    }
    closed(
        t,
        &[
            "stage",
            "pending_dependency",
            "saved_state_704",
            "state_restored",
            "script_reference",
            "metadata_reads",
            "current_slot",
            "current_record_present",
            "current_inventory_key",
            "variant",
            "cleanup_checks",
            "b84_written",
            "bd1_written",
            "actor_absent",
            "override_removals",
            "helper_return",
            "pose_input_raw",
            "pose_input_signed",
            "state_sha256",
        ],
    )?;
    let stage = STAGES
        .iter()
        .position(|s| t["stage"] == *s)
        .context("unsupported constructor continuation stage")?;
    let reads = t["metadata_reads"]
        .as_array()
        .filter(|r| r.len() == 3)
        .context("constructor metadata reads must keep three ordered positions")?;
    let checks = t["cleanup_checks"]
        .as_array()
        .filter(|r| r.len() == 5)
        .context("constructor cleanup checks must keep five ordered positions")?;
    let removals = t["override_removals"]
        .as_array()
        .filter(|r| r.len() == 2)
        .context("constructor override removals must keep two ordered positions")?;
    ensure!(
        component["callback_tail"]["stage"] == "returned"
            && component["callback_tail"]["callback_returned"] == true
            && word(&t["saved_state_704"])
            && t["state_restored"] == (stage > 0 && t["saved_state_704"] == 5)
            && t["b84_written"].is_boolean()
            && t["bd1_written"].is_boolean()
            && hash(&t["state_sha256"])
            && if stage == 18 {
                super::player_selector::at_least(&component["stage"], 3)
            } else {
                component["stage"] == "player_constructor_tail_pending"
            },
        "constructor entry, restore or current root stage differs"
    );
    let expected_dependency = match stage {
        1 => Some("constructor_script_selection"),
        3 => Some("constructor_alternate_variant"),
        4 => Some("constructor_component_binding"),
        5 => Some("constructor_component_cleanup"),
        6 => Some("constructor_special_primary_cleanup"),
        7 => Some("constructor_body_cleanup"),
        9 => Some("constructor_child_cleanup"),
        11 => Some("constructor_actor_cleanup"),
        12 | 13 => Some("constructor_visual_overrides"),
        14 => Some("constructor_first_effect_cleanup"),
        15 => Some("constructor_second_effect_cleanup"),
        18 => Some("constructor_pose"),
        _ => None,
    };
    if t["pending_dependency"].is_null() {
        ensure!(stage != 18, "pose boundary lost its actual dependency");
    } else {
        ensure!(
            expected_dependency.is_some_and(|d| t["pending_dependency"] == d),
            "constructor dependency skips source ordering"
        );
    }
    ensure!(
        if stage >= 2 {
            t["script_reference"] == 0x0400_1dacu32
        } else {
            t["script_reference"].is_null()
        },
        "constructor script selection was skipped or invented"
    );
    for (i, (field, reached)) in [("type", 1), ("inventory_key", 4), ("primary_key", 6)]
        .into_iter()
        .enumerate()
    {
        if reads[i].is_null() {
            continue;
        }
        closed(&reads[i], &["slot", "field", "value"])?;
        ensure!(
            stage >= reached
                && reads[i]["slot"].as_u64().is_some_and(|n| n < 128)
                && reads[i]["field"] == field
                && word(&reads[i]["value"]),
            "constructor metadata field was read before its source branch"
        );
    }
    if stage >= 2 {
        ensure!(
            !matches!(reads[0]["value"].as_u64(), Some(0x4b20_0001 | 0x4b20_0009)),
            "unsupported metadata type claimed script selection"
        );
    }
    if stage >= 5 {
        ensure!(
            matches!(reads[1]["value"].as_u64(), Some(0x0200_0830 | 0x0200_1e4d)),
            "constructor cleanup skipped its actual inventory-key gate"
        );
    }
    if stage >= 7 {
        ensure!(
            reads[2]["value"] != 0x4b00_001bu32,
            "constructor skipped special primary cleanup"
        );
    }
    if stage >= 3 {
        let slot = t["current_slot"]
            .as_i64()
            .filter(|s| (-128..=127).contains(s))
            .context("constructor current signed slot missing")?;
        ensure!(
            t["current_record_present"].is_boolean()
                && word(&t["current_inventory_key"])
                && (slot >= 0 || t["current_record_present"] == false)
                && (t["current_record_present"] != false || t["current_inventory_key"] == 0),
            "constructor current record NULL branch differs"
        );
    } else {
        ensure!(
            [
                "current_slot",
                "current_record_present",
                "current_inventory_key"
            ]
            .iter()
            .all(|k| t[k].is_null()),
            "constructor record read precedes script selection"
        );
    }
    ensure!(
        if stage >= 4 {
            t["variant"].as_u64().is_some_and(|v| v <= 1)
        } else {
            t["variant"].is_null()
        },
        "constructor variant precedes its reached branch"
    );
    for (i, reached) in [5, 7, 8, 14, 15].into_iter().enumerate() {
        ensure!(
            checks[i].is_null() || checks[i].is_boolean(),
            "constructor cleanup check is not an optional actual NULL result"
        );
        if stage < reached {
            ensure!(
                checks[i].is_null(),
                "constructor cleanup link read too early"
            );
        }
        if stage > reached {
            ensure!(
                checks[i].is_boolean(),
                "constructor cleanup skipped a link read"
            );
        }
        let supported_after = if i == 2 { 9 } else { reached };
        if stage > supported_after {
            ensure!(
                checks[i] == true,
                "constructor advanced past an unsupported cleanup owner"
            );
        }
    }
    ensure!(
        (stage >= 9 || t["b84_written"] == false)
            && (stage <= 9 || t["b84_written"] == true)
            && (stage >= 10 || t["bd1_written"] == false)
            && (stage <= 10 || t["bd1_written"] == true),
        "constructor word/byte clears skip source ordering"
    );
    ensure!(
        t["actor_absent"].is_null() || t["actor_absent"].is_boolean(),
        "constructor actor NULL result has wrong type"
    );
    if stage < 11 {
        ensure!(
            t["actor_absent"].is_null(),
            "constructor actor gate read before marker store"
        );
    }
    if stage > 11 {
        ensure!(
            t["actor_absent"] == true,
            "constructor advanced past an unsupported actor owner"
        );
    }
    for (i, r) in removals.iter().enumerate() {
        let reached = 12 + i;
        if stage < reached {
            ensure!(r.is_null(), "constructor override removal ran too early");
        }
        if stage > reached {
            ensure!(!r.is_null(), "constructor skipped ordered override removal");
        }
        if !r.is_null() {
            closed(r, &["key", "count_raw", "removed"])?;
            ensure!(
                r["key"] == [0x0600_056du32, 0x0600_067b][i]
                    && word(&r["count_raw"])
                    && (u32::try_from(count(r, "count_raw")?)? as i32) <= 0
                    && r["removed"] == false,
                "constructor count-only no-op removal invented a mutation"
            );
        }
    }
    ensure!(
        if stage >= 17 {
            t["helper_return"] == 1
        } else {
            t["helper_return"].is_null()
        },
        "constructor helper return skipped cleanup or claims an early return"
    );
    if stage == 18 {
        let raw = t["pose_input_raw"]
            .as_u64()
            .filter(|v| *v <= 255)
            .context("constructor pose argument byte absent")? as u8;
        ensure!(
            t["pose_input_signed"].as_i64() == Some(i64::from(raw as i8)),
            "constructor pose argument lost signed byte semantics"
        );
    } else {
        ensure!(
            t["pose_input_raw"].is_null() && t["pose_input_signed"].is_null(),
            "pose argument read precedes the real helper return"
        );
    }
    match t["pending_dependency"].as_str() {
        Some("constructor_script_selection") => ensure!(
            matches!(reads[0]["value"].as_u64(), Some(0x4b20_0001 | 0x4b20_0009)),
            "constructor type dependency has no reached source gate"
        ),
        Some("constructor_component_binding") => ensure!(
            !matches!(reads[1]["value"].as_u64(), Some(0x0200_0830 | 0x0200_1e4d)),
            "supported cleanup key was substituted with a binding dependency"
        ),
        Some("constructor_component_cleanup") => ensure!(
            checks[0] == false,
            "constructor component dependency has no live owner"
        ),
        Some("constructor_special_primary_cleanup") => ensure!(
            reads[2]["value"] == 0x4b00_001bu32,
            "constructor special cleanup lost its actual primary key"
        ),
        Some("constructor_body_cleanup") => ensure!(
            checks[1] == false,
            "constructor body dependency has no live owner"
        ),
        Some("constructor_child_cleanup") => ensure!(
            checks[2] == false && t["b84_written"] == true,
            "constructor child dependency lost its already-written b84 prefix"
        ),
        Some("constructor_actor_cleanup") => ensure!(
            t["actor_absent"] == false,
            "constructor actor dependency has no live owner"
        ),
        Some("constructor_visual_overrides") => ensure!(
            removals[stage - 12].is_null(),
            "constructor nonempty override dependency claims a returned no-op"
        ),
        Some("constructor_first_effect_cleanup") => ensure!(
            checks[3] == false,
            "constructor first effect dependency has no live owner"
        ),
        Some("constructor_second_effect_cleanup") => ensure!(
            checks[4] == false,
            "constructor second effect dependency has no live owner"
        ),
        _ => {}
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
