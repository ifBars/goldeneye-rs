use super::{closed, count};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
pub(super) const STAGES: [&str; 24] = [
    "entry",
    "blend_gate",
    "enable_arguments",
    "third_person_enable",
    "first_person_enable",
    "character_binding",
    "character_10",
    "character_14",
    "session_scale",
    "scale",
    "scale_invalidate",
    "dirty",
    "weapon_mode",
    "force_components",
    "user_target",
    "user_lookup",
    "user_false_arm",
    "backlink_components",
    "attachment",
    "global_byte",
    "global_attachment",
    "fade_inputs",
    "final_session",
    "returned",
];
fn word(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX))
}
fn byte(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= 255)
}
fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn optional_bool(v: &Value) -> bool {
    v.is_null() || v.is_boolean()
}
fn enable(v: &Value, i: usize, p: &Value) -> Result<()> {
    closed(
        v,
        &[
            "component_file",
            "reference",
            "enabled",
            "reference_matched",
            "flags_before",
            "flags_after",
            "invalidated_8e",
        ],
    )?;
    ensure!(
        v["component_file"] == [0x0102_0017u32, 0x0102_0016][i]
            && word(&v["reference"])
            && !matches!((count(v, "reference")? >> 24) & 0x7f, 2 | 0x0e)
            && v["enabled"] == p[if i == 0 { "tp_enabled" } else { "fp_enabled" }]
            && v["enabled"].is_boolean()
            && v["reference_matched"].is_boolean()
            && v["invalidated_8e"].is_boolean(),
        "pose enable receiver or reference differs"
    );
    if v["reference_matched"] == true {
        let before = count(v, "flags_before")?;
        let after = count(v, "flags_after")?;
        ensure!(
            before <= 65535
                && after
                    == if v["enabled"] == true {
                        before | 1
                    } else {
                        before & 0xfffe
                    }
                && v["invalidated_8e"] == v["enabled"],
            "pose semantic enable flag width or stores differ"
        );
    } else {
        ensure!(
            v["flags_before"].is_null()
                && v["flags_after"].is_null()
                && v["invalidated_8e"] == false,
            "pose mismatched reference claims enable stores"
        );
    }
    Ok(())
}
fn lookup(v: &Value) -> Result<()> {
    closed(
        v,
        &[
            "visited",
            "matched",
            "association_state",
            "association_origin",
            "state_sha256",
        ],
    )?;
    ensure!(
        v["visited"] == 1
            && v["matched"].is_boolean()
            && matches!(
                v["association_state"].as_str(),
                Some("known_null" | "bound")
            )
            && matches!(
                v["association_origin"].as_str(),
                Some("local_user_constructor" | "supplied_snapshot")
            )
            && (v["association_state"] != "known_null" || v["matched"] == false)
            && (v["association_origin"] != "local_user_constructor"
                || v["association_state"] == "known_null")
            && hash(&v["state_sha256"]),
        "pose local-user result lacks its actual association lifetime"
    );
    Ok(())
}
pub(super) fn validate(p: &Value, c: &Value) -> Result<()> {
    if p.is_null() {
        ensure!(
            !super::player_selector::at_least(&c["stage"], 4),
            "outer helper advanced before pose helper return"
        );
        return Ok(());
    }
    closed(
        p,
        &[
            "stage",
            "pending_dependency",
            "input_raw",
            "input_signed",
            "helper_returned",
            "entry_702",
            "blend_ba8",
            "bab_written",
            "tp_enabled",
            "fp_enabled",
            "receiver_searches",
            "enable_writes",
            "character_bound",
            "character_scalars_read",
            "character_vector_read",
            "session_scale_multiple",
            "scale_source",
            "scale_stored",
            "scale_invalidated",
            "dirty_before",
            "dirty_after",
            "ca4_written",
            "mode_afc_absent",
            "force_nodes_checked",
            "user_target_bound",
            "user_lookup",
            "false_arm_b00_absent",
            "backlink_nodes_checked",
            "attachment_absent",
            "weapon_global_read",
            "weapon_global_byte",
            "global_argument",
            "global_equivalence_used",
            "global_attachment_absent",
            "fade_inputs_read",
            "fade_argument_ready",
            "fade_attachment_absent",
            "final_session_multiple",
            "state_sha256",
        ],
    )?;
    let stage = STAGES
        .iter()
        .position(|s| p["stage"] == *s)
        .context("unsupported pose helper stage")?;
    let searches = p["receiver_searches"]
        .as_array()
        .filter(|v| v.len() == 2 && v.iter().all(Value::is_boolean))
        .context("pose receiver searches absent")?;
    let enables = p["enable_writes"]
        .as_array()
        .filter(|v| v.len() == 2)
        .context("pose enable receipts absent")?;
    ensure!(
        c["constructor_tail"]["stage"] == "awaiting_pose"
            && c["constructor_tail"]["helper_return"] == 1
            && p["input_raw"] == c["constructor_tail"]["pose_input_raw"]
            && byte(&p["input_raw"])
            && p["input_signed"].as_i64() == Some(i64::from(count(p, "input_raw")? as u8 as i8))
            && p["helper_returned"] == (stage == 23)
            && hash(&p["state_sha256"])
            && if stage == 23 {
                super::player_selector::at_least(&c["stage"], 4)
            } else {
                c["stage"] == "player_pose_setup_pending"
            },
        "pose helper caller, signed input or return scope differs"
    );
    for key in [
        "bab_written",
        "character_bound",
        "character_vector_read",
        "scale_stored",
        "scale_invalidated",
        "ca4_written",
        "user_target_bound",
        "weapon_global_read",
        "global_equivalence_used",
        "fade_inputs_read",
        "fade_argument_ready",
    ] {
        ensure!(p[key].is_boolean(), "pose effect flag is not boolean");
    }
    for key in [
        "tp_enabled",
        "fp_enabled",
        "session_scale_multiple",
        "mode_afc_absent",
        "false_arm_b00_absent",
        "attachment_absent",
        "global_attachment_absent",
        "fade_attachment_absent",
        "final_session_multiple",
    ] {
        ensure!(
            optional_bool(&p[key]),
            "pose branch read is not an optional boolean"
        );
    }
    ensure!(
        count(p, "character_scalars_read")? <= 2
            && count(p, "force_nodes_checked")? <= 2
            && count(p, "backlink_nodes_checked")? <= 2,
        "pose source visit counts exceed actual owners"
    );
    let early = p["entry_702"].as_u64().is_some_and(|v| v > 0 && v <= 255);
    if early {
        ensure!(
            stage == 23
                && p["pending_dependency"].is_null()
                && [
                    "blend_ba8",
                    "tp_enabled",
                    "fp_enabled",
                    "session_scale_multiple",
                    "scale_source",
                    "dirty_before",
                    "dirty_after",
                    "mode_afc_absent",
                    "user_lookup",
                    "false_arm_b00_absent",
                    "attachment_absent",
                    "weapon_global_byte",
                    "global_argument",
                    "global_attachment_absent",
                    "fade_attachment_absent",
                    "final_session_multiple"
                ]
                .iter()
                .all(|k| p[k].is_null())
                && [
                    "bab_written",
                    "character_bound",
                    "character_vector_read",
                    "scale_stored",
                    "scale_invalidated",
                    "ca4_written",
                    "user_target_bound",
                    "weapon_global_read",
                    "global_equivalence_used",
                    "fade_inputs_read",
                    "fade_argument_ready"
                ]
                .iter()
                .all(|k| p[k] == false)
                && [
                    "character_scalars_read",
                    "force_nodes_checked",
                    "backlink_nodes_checked"
                ]
                .iter()
                .all(|k| p[k] == 0)
                && searches.iter().all(|v| *v == false)
                && enables.iter().all(Value::is_null),
            "pose nonzero entry branch claims later effects"
        );
        return Ok(());
    }
    ensure!(
        if stage == 0 {
            p["entry_702"].is_null()
        } else {
            p["entry_702"] == 0
        },
        "pose entry gate lost its actual byte"
    );
    ensure!(
        p["blend_ba8"].is_null() || byte(&p["blend_ba8"]),
        "pose blend byte lost its width"
    );
    if stage == 0 {
        ensure!(
            p["blend_ba8"].is_null() && p["bab_written"] == false,
            "pose blend effects precede entry read"
        );
    }
    if stage >= 2 {
        ensure!(
            p["blend_ba8"] == 0 && p["bab_written"] == true,
            "pose skipped its blend-dependent bab store"
        );
    }
    if stage < 3 {
        ensure!(
            p["tp_enabled"].is_null() && p["fp_enabled"].is_null(),
            "pose enable arguments were invented before their read"
        );
    } else {
        ensure!(
            p["tp_enabled"] == (p["input_signed"] == 0)
                && p["fp_enabled"].is_boolean()
                && (p["tp_enabled"] != true || p["fp_enabled"] == false),
            "pose enable arguments differ from the source input branch"
        );
    }
    for i in 0..2 {
        if stage < 3 + i {
            ensure!(
                searches[i] == false && enables[i].is_null(),
                "pose receiver reached out of order"
            );
        }
        if stage > 3 + i {
            ensure!(
                searches[i] == true,
                "pose skipped its ordered receiver search"
            );
        }
        if !enables[i].is_null() {
            ensure!(
                searches[i] == true,
                "pose enable has no actual receiver search"
            );
            enable(&enables[i], i, p)?;
        }
    }
    ensure!(
        p["character_bound"] == (stage >= 6)
            && count(p, "character_scalars_read")?
                == if stage < 7 {
                    0
                } else if stage == 7 {
                    1
                } else {
                    2
                }
            && p["character_vector_read"] == (stage >= 8),
        "pose character fields or vector skip source ordering"
    );
    if stage < 8 {
        ensure!(
            p["session_scale_multiple"].is_null(),
            "pose session scale predicate read too early"
        );
    }
    if stage > 8 {
        ensure!(
            p["session_scale_multiple"] == false,
            "pose skipped a multiplayer scale provider"
        );
    }
    ensure!(
        (p["scale_source"].is_null()
            || stage >= 9
                && p["scale_source"]
                    == if p["fp_enabled"] == true {
                        "startup_ones"
                    } else {
                        "character"
                    })
            && p["scale_stored"] == (stage >= 10)
            && p["scale_invalidated"] == (stage >= 11)
            && (p["scale_stored"] != true || !p["scale_source"].is_null()),
        "pose scale stores or invalidation prefix differs"
    );
    if stage <= 11 {
        ensure!(
            p["dirty_before"].is_null() && p["dirty_after"].is_null(),
            "pose dirty-byte store precedes scale invalidation"
        );
    } else {
        ensure!(
            byte(&p["dirty_before"])
                && byte(&p["dirty_after"])
                && count(p, "dirty_after")? == count(p, "dirty_before")? & 0xdf,
            "pose dirty-byte mask or width differs"
        );
    }
    ensure!(
        (stage >= 12 || p["ca4_written"] == false) && (stage <= 12 || p["ca4_written"] == true),
        "pose weapon mode store precedes its source branch"
    );
    for (key, reached) in [
        ("mode_afc_absent", 12),
        ("false_arm_b00_absent", 16),
        ("attachment_absent", 18),
        ("global_attachment_absent", 20),
        ("fade_attachment_absent", 21),
    ] {
        if stage < reached {
            ensure!(p[key].is_null(), "pose NULL helper read too early");
        }
        if stage > reached {
            ensure!(
                p[key] == true,
                "pose advanced past an unsupported live receiver"
            );
        }
    }
    ensure!(
        (stage >= 13 || p["force_nodes_checked"] == 0)
            && (stage >= 17 || p["backlink_nodes_checked"] == 0)
            && (stage >= 14 || p["user_target_bound"] == false)
            && (stage <= 14 || p["user_target_bound"] == true),
        "pose component visits or user target skip source ordering"
    );
    if !p["user_lookup"].is_null() {
        ensure!(stage >= 15, "pose user lookup ran before target selection");
        lookup(&p["user_lookup"])?;
    }
    if stage > 15 {
        ensure!(
            !p["user_lookup"].is_null() && p["user_lookup"]["matched"] == false,
            "pose advanced past its matched-user provider"
        );
    }
    ensure!(
        (stage >= 19 || p["weapon_global_read"] == false)
            && (stage <= 19 || p["weapon_global_read"] == true)
            && (p["weapon_global_byte"].is_null()
                || p["weapon_global_read"] == true && byte(&p["weapon_global_byte"])),
        "pose global read was skipped or defaulted"
    );
    if let Some(raw) = p["weapon_global_byte"].as_u64() {
        ensure!(
            p["global_argument"].as_i64()
                == Some(if raw == 0 {
                    p["input_signed"]
                        .as_i64()
                        .context("pose signed input absent")?
                } else {
                    0
                })
                && p["global_equivalence_used"] == false,
            "known pose global changed the actual argument"
        );
    } else {
        ensure!(
            p["global_argument"].is_null() && p["global_equivalence_used"] == (stage > 20),
            "unknown pose global became a fabricated value or an unproved no-op"
        );
    }
    ensure!(
        (stage >= 21 || p["fade_inputs_read"] == false)
            && (stage >= 21 || p["fade_argument_ready"] == false)
            && (p["fade_argument_ready"] != true || p["fade_inputs_read"] == true)
            && (stage <= 21 || p["fade_inputs_read"] == true && p["fade_argument_ready"] == true),
        "pose fade input/argument prefix differs"
    );
    if stage < 22 {
        ensure!(
            p["final_session_multiple"].is_null(),
            "final session predicate read too early"
        );
    }
    if stage == 23 {
        ensure!(
            p["final_session_multiple"] == false,
            "pose helper returned before its final multiplayer branch"
        );
    }
    let expected = match stage {
        1 => Some("pose_blend_threshold"),
        3 | 4 => Some("pose_skeleton_enable"),
        8 => Some("pose_multiplayer_scale"),
        12 => Some("pose_weapon_mode"),
        14 => Some("pose_user_target"),
        15 => Some("pose_matched_user"),
        17 => Some("pose_component_backlink"),
        20 => Some("pose_global_argument"),
        21 => Some("pose_fade_attachment"),
        22 => Some("pose_final_multiplayer"),
        _ => None,
    };
    if !p["pending_dependency"].is_null() {
        ensure!(
            expected.is_some_and(|v| p["pending_dependency"] == v)
                || matches!(stage, 12 | 13 | 16 | 18)
                    && p["pending_dependency"] == "pose_attachment_owner",
            "pose dependency skips its actual source stage"
        );
    }
    match p["pending_dependency"].as_str() {
        Some("pose_blend_threshold") => ensure!(
            p["bab_written"] == true && byte(&p["blend_ba8"]) && p["blend_ba8"] != 0,
            "pose blend dependency lost its source prefix"
        ),
        Some("pose_multiplayer_scale") => ensure!(
            p["session_scale_multiple"] == true,
            "pose scale dependency has no multiplayer predicate"
        ),
        Some("pose_matched_user") => ensure!(
            p["user_lookup"]["matched"] == true,
            "pose user dependency has no actual matched association"
        ),
        Some("pose_global_argument") => ensure!(
            p["global_attachment_absent"] == false,
            "pose global dependency has no live receiver"
        ),
        Some("pose_fade_attachment") => ensure!(
            p["fade_attachment_absent"] == false
                && p["fade_inputs_read"] == true
                && p["fade_argument_ready"] == true,
            "pose fade dependency lost its reached input prefix"
        ),
        Some("pose_final_multiplayer") => ensure!(
            p["final_session_multiple"] == true,
            "pose final multiplayer dependency has no predicate"
        ),
        _ => {}
    }
    Ok(())
}
#[cfg(test)]
pub(super) mod tests;
