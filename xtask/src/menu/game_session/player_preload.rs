//! Current preload effects do not rewrite the returned initializer snapshot.
use super::{closed, count, player_initializer_prefix};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;
mod completion;
mod evaluation;
mod post_bind;

fn hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn word(v: &Value) -> bool {
    v.as_u64().is_some_and(|n| n <= u64::from(u32::MAX))
}
fn optional_index(v: &Value, limit: u64) -> bool {
    v.is_null() || v.as_u64().is_some_and(|n| n < limit)
}
fn script_publication(v: &Value, t: &Value) -> Result<()> {
    if v.is_null() {
        return Ok(());
    }
    closed(
        v,
        &[
            "file",
            "generation",
            "descriptor_index",
            "section_index",
            "section_key",
            "callback_mask",
            "fresh_base",
            "request_count",
            "active_requests",
            "retained_sections",
            "history_unknown",
            "raw_job",
            "body_state",
            "native_history_verified",
        ],
    )?;
    ensure!(
        !t.is_null()
            && !t["script"].is_null()
            && word(&v["file"])
            && word(&v["generation"])
            && v["file"] == t["component_file"]
            && v["descriptor_index"] == t["script"]["descriptor_index"]
            && v["descriptor_index"].as_u64().is_some_and(|n| n < 32767)
            && v["section_index"].as_u64().is_some_and(|n| n < 32767)
            && word(&v["section_key"])
            && (v["callback_mask"].is_null() || word(&v["callback_mask"]))
            && (v["raw_job"].is_null() || word(&v["raw_job"]))
            && v["fresh_base"].is_boolean()
            && v["history_unknown"].is_boolean()
            && v["fresh_base"] == !v["callback_mask"].is_null()
            && v["native_history_verified"] == false,
        "script publication identity or source metadata differs"
    );
    count(v, "request_count")?;
    count(v, "active_requests")?;
    count(v, "retained_sections")?;
    match v["body_state"].as_str() {
        Some("known_null") => ensure!(
            v["fresh_base"] == true
                && v["callback_mask"]
                    .as_u64()
                    .is_some_and(|mask| mask & 8 != 0)
                && v["active_requests"] == 0
                && v["retained_sections"] == 0
                && v["history_unknown"] == false
                && v["raw_job"] == 0
                && count(v, "section_key")? >> 24 == 8
                && v["section_key"] != 0x0800_0000u32
                && count(v, "section_index")? > 0,
            "known NULL script lacks fresh nonbase publication proof"
        ),
        Some("pending") => {}
        _ => anyhow::bail!("unknown script publication body state"),
    }
    Ok(())
}
pub(super) fn dependency(p: &Value) -> Option<&'static str> {
    if p["stage"] == "returned" {
        return Some("player_initializer_post_preload");
    }
    if !p["completion"]["destruction"].is_null() {
        return completion::dependency(&p["completion"]["destruction"]);
    }
    if !p["outputs"].is_null() {
        return completion::output_dependency(&p["outputs"]);
    }
    if !p["post_bind"].is_null() {
        return Some(
            post_bind::dependency(&p["post_bind"]).unwrap_or("preload_temporary_post_bind"),
        );
    }
    match p["stage"].as_str()? {
        "awaiting_temporary" => Some(
            if let Some(dependency) = temporary_evaluation_dependency(p) {
                dependency
            } else if p["temporary"]["bind_return"] == 1 {
                "preload_temporary_post_bind"
            } else {
                "preload_temporary_bind"
            },
        ),
        "returned" => Some("player_initializer_post_preload"),
        "entry" | "raw_flags" | "entry_keys" | "registry_count" | "row" | "weapon" | "call"
        | "final_flag" => Some("player_animation_preload"),
        _ => None,
    }
}
fn temporary_evaluation_dependency(p: &Value) -> Option<&'static str> {
    match p["temporary"]["pending_dependency"].as_str() {
        Some("temporary_evaluation_sampling") => Some("preload_temporary_evaluation_sampling"),
        Some("temporary_evaluation_skin") => Some("preload_temporary_evaluation_skin"),
        Some("temporary_evaluation_auxiliary_clear") => {
            Some("preload_temporary_evaluation_auxiliary_clear")
        }
        Some("temporary_evaluation_layers") => Some("preload_temporary_evaluation_layers"),
        _ => None,
    }
}
fn call(v: &Value, ordinal: usize, raw_known: &mut BTreeSet<u64>) -> Result<()> {
    closed(
        v,
        &[
            "stage",
            "ordinal",
            "source_ordinal",
            "weapon",
            "generic",
            "alternate",
            "variant",
            "mapping",
            "raw_flags_read",
            "raw_keys_read",
            "raw_index",
            "raw_indices_written",
            "entry_keys_read",
            "entry_index",
            "occupied_written",
            "weapon_written",
            "mapping_written",
            "selected_key_empty",
            "invalidated",
            "extraction_returned",
            "reference_outcome",
            "reference",
            "extraction",
            "return_word",
            "return_reason",
            "state_sha256",
        ],
    )?;
    let stage = [
        "remap",
        "raw_flag",
        "raw_key",
        "raw_indices",
        "entry_scan",
        "raw_occupied",
        "raw_weapon",
        "raw_mapping",
        "entry_key",
        "invalidate",
        "reference",
        "awaiting_temporary",
        "returned",
    ]
    .iter()
    .position(|s| v["stage"] == *s)
    .context("unsupported preload call stage")?;
    ensure!(
        count(v, "ordinal")? == ordinal as u64
            && word(&v["weapon"])
            && v["weapon"] != 0
            && v["alternate"].is_boolean()
            && count(v, "variant")? < 3
            && v["generic"]
                == if v["alternate"] == true {
                    0x0900_07a7u32
                } else {
                    0x0900_0765u32
                }
            && count(v, "raw_flags_read")? <= 32
            && count(v, "raw_keys_read")? <= count(v, "raw_flags_read")?
            && count(v, "entry_keys_read")? <= 35
            && optional_index(&v["raw_index"], 32)
            && optional_index(&v["entry_index"], 34)
            && hash(&v["state_sha256"]),
        "preload call request or counters differ"
    );
    for k in [
        "raw_indices_written",
        "occupied_written",
        "weapon_written",
        "mapping_written",
        "invalidated",
        "extraction_returned",
    ] {
        ensure!(v[k].is_boolean(), "preload call source effect flag absent");
    }
    for (field, reached) in [
        ("occupied_written", 6),
        ("weapon_written", 7),
        ("mapping_written", 8),
    ] {
        if stage < reached {
            ensure!(
                v[field] == false,
                "preload source store ran before its stage"
            );
        } else if stage < 12 {
            ensure!(
                v[field] == true,
                "preload stage advanced before its source store"
            );
        }
    }
    if stage < 5 {
        ensure!(
            v["entry_index"].is_null(),
            "preload entry selected before actual scan"
        );
    }
    if stage < 4 {
        ensure!(
            v["raw_indices_written"] == false,
            "preload index bytes initialized before empty Raw arm"
        );
    }
    if stage < 3 {
        ensure!(
            v["raw_index"].is_null(),
            "preload Raw selected before source flag/key read"
        );
    }
    if (5..12).contains(&stage) {
        ensure!(
            !v["entry_index"].is_null(),
            "preload source stage has no selected entry"
        );
    }
    if (9..12).contains(&stage) {
        ensure!(
            v["selected_key_empty"] == true,
            "preload empty-entry arm lacks actual key read"
        );
    }
    if stage == 11 {
        ensure!(
            v["extraction_returned"] == true && v["invalidated"] == true,
            "temporary boundary precedes extractor/invalidation return"
        );
    }
    if (10..12).contains(&stage) {
        ensure!(
            v["invalidated"] == true,
            "preload extractor precedes actual validity invalidation"
        );
    }
    if v["mapping"].is_null() {
        ensure!(stage == 0, "preload advanced before its actual mapping");
    } else {
        let m = &v["mapping"];
        closed(
            m,
            &["output", "mapped", "steps", "map_rows_read", "state_sha256"],
        )?;
        ensure!(
            stage > 0
                && word(&m["output"])
                && m["mapped"].is_boolean()
                && count(m, "steps")? <= 128
                && count(m, "map_rows_read")? <= 4096
                && hash(&m["state_sha256"]),
            "preload mapping result differs"
        );
    }
    ensure!(
        v["weapon_written"] != true || v["occupied_written"] == true,
        "preload weapon precedes occupancy"
    );
    ensure!(
        v["mapping_written"] != true || v["weapon_written"] == true,
        "preload mapping byte precedes weapon"
    );
    ensure!(
        v["invalidated"] != true
            || (v["mapping_written"] == true && v["selected_key_empty"] == true),
        "preload invalidates before its actual empty entry"
    );
    if v["raw_indices_written"] == true
        || v["occupied_written"] == true
        || v["weapon_written"] == true
        || v["mapping_written"] == true
    {
        let raw = count(v, "raw_index")? * 16;
        ensure!(
            count(v, "raw_flags_read")? == count(v, "raw_index")? + 1
                && count(v, "raw_keys_read")?
                    == count(v, "raw_index")? + u64::from(v["raw_indices_written"] != true),
            "raw scan counters skip source entries"
        );
        if v["raw_indices_written"] == true {
            raw_known.extend(raw + 5..raw + 13);
        }
        if v["occupied_written"] == true {
            raw_known.insert(raw + 4);
        }
        if v["weapon_written"] == true {
            raw_known.extend(raw..raw + 4);
        }
        if v["mapping_written"] == true {
            ensure!(
                !v["entry_index"].is_null(),
                "mapping store lacks actual entry"
            );
            raw_known
                .insert(raw + if v["alternate"] == true { 9 } else { 5 } + count(v, "variant")?);
        }
    }
    if stage < 9 {
        ensure!(
            v["selected_key_empty"].is_null(),
            "entry key consumed before its source read"
        );
    }
    if stage < 10 {
        ensure!(v["invalidated"] == false, "preload invalidation ran early");
    }
    if stage < 11 {
        ensure!(
            v["extraction_returned"] == false,
            "preload extraction returned early"
        );
    }
    if v["extraction_returned"] == true {
        ensure!(
            (stage == 11 || stage == 12 && v["return_reason"] == "temporary_completed")
                && v["invalidated"] == true,
            "preload extraction bypassed prefix"
        );
        match v["reference_outcome"].as_str() {
            Some("no_reference") => ensure!(
                v["reference"].is_null() && v["extraction"].is_null(),
                "known no-reference became a fabricated class-four child"
            ),
            Some("class_four") => {
                ensure!(
                    word(&v["reference"]) && (count(v, "reference")? >> 24) & 0x7f == 4,
                    "preload reference is not class four"
                );
                let e = &v["extraction"];
                closed(
                    e,
                    &[
                        "requested_index",
                        "baseline_index",
                        "transition_ordinal",
                        "command_ordinal",
                        "set_index",
                        "group_ordinal",
                        "child_ordinal",
                        "transitions_read",
                        "commands_read",
                        "groups_read",
                        "children_read",
                    ],
                )?;
                for k in ["requested_index", "baseline_index", "set_index"] {
                    ensure!(
                        count(e, k)? < 32768,
                        "preload directory ordinal exceeds signed table bound"
                    );
                }
                for (ordinal, reads) in [
                    ("transition_ordinal", "transitions_read"),
                    ("command_ordinal", "commands_read"),
                    ("group_ordinal", "groups_read"),
                    ("child_ordinal", "children_read"),
                ] {
                    ensure!(
                        count(e, ordinal)? < count(e, reads)? && count(e, reads)? <= 65536,
                        "preload extractor skipped reached source order"
                    );
                }
            }
            _ => anyhow::bail!("preload extraction outcome absent"),
        }
    } else {
        ensure!(
            v["reference_outcome"].is_null()
                && v["reference"].is_null()
                && v["extraction"].is_null(),
            "unreturned extractor claims a reference"
        );
    }
    if stage == 12 {
        ensure!(
            v["return_reason"] == "temporary_completed"
                || v["reference"].is_null() && v["extraction_returned"] == false,
            "prefix call claims temporary completion"
        );
        match v["return_reason"].as_str() {
            Some("temporary_completed") => ensure!(
                v["return_word"] == 1
                    && v["extraction_returned"] == true
                    && v["selected_key_empty"] == true
                    && v["mapping_written"] == true
                    && v["invalidated"] == true,
                "temporary return bypassed actual call prefix"
            ),
            Some("variant_unavailable") => ensure!(
                v["return_word"] == 0
                    && v["variant"] != 0
                    && v["mapping"]["mapped"] == false
                    && v["raw_flags_read"] == 0
                    && v["entry_keys_read"] == 0
                    && v["raw_index"].is_null(),
                "variant miss read caches"
            ),
            Some("raw_full") => ensure!(
                v["return_word"] == 0
                    && v["raw_flags_read"] == 32
                    && v["raw_keys_read"] == 32
                    && v["raw_index"].is_null()
                    && v["entry_keys_read"] == 0,
                "raw exhaustion is not proved"
            ),
            Some("entries_full") => ensure!(
                v["return_word"] == 0
                    && v["entry_keys_read"] == 34
                    && v["entry_index"].is_null()
                    && v["occupied_written"] == false,
                "entry exhaustion skipped source prefix"
            ),
            Some("existing_entry") => ensure!(
                v["return_word"] == 1
                    && v["mapping_written"] == true
                    && v["selected_key_empty"] == false
                    && v["invalidated"] == false,
                "existing entry fabricated evaluation"
            ),
            _ => anyhow::bail!("unsupported preload call return"),
        }
    } else {
        ensure!(
            v["return_word"].is_null() && v["return_reason"].is_null(),
            "pending preload call claims return"
        );
    }
    if (8..12).contains(&stage) {
        ensure!(
            v["mapping_written"] == true,
            "preload advanced before mapping byte store"
        );
    }
    Ok(())
}

fn temporary(t: &Value, p: &Value, c: &Value) -> Result<()> {
    closed(
        t,
        &[
            "stage",
            "pending_dependency",
            "identity",
            "source_receiver",
            "player",
            "component_file",
            "reference",
            "skin_file",
            "skin_reference",
            "attached",
            "callbacks_bound",
            "skeleton_bound",
            "bones",
            "auxiliary_words",
            "skeleton_backlink_bound",
            "pose_callback_assigned",
            "frame_stamp_known",
            "semantic_flags",
            "component_flags",
            "active_slots",
            "factory_returned",
            "head_inserted",
            "script",
            "forward",
            "maintenance_skipped",
            "reverse_skipped",
            "bind_return",
            "refresh_returned",
            "evaluation",
            "state_sha256",
        ],
    )?;
    let stage = [
        "awaiting_skeleton",
        "binding_skeleton",
        "awaiting_factory",
        "initializing_factory",
        "awaiting_initializer",
        "awaiting_insertion",
        "awaiting_forward_tick",
        "forward_tick",
        "awaiting_evaluation",
        "bind_returned",
        "failed",
        "evaluating_bind",
        "aborted",
    ]
    .iter()
    .position(|s| t["stage"] == *s)
    .context("unknown preload temporary stage")?;
    ensure!(
        count(t, "identity")? > count(t, "source_receiver")?
            && count(t, "source_receiver")? > 0
            && count(t, "player")? > 0
            && t["component_file"] == 0x0102_0016
            && word(&t["reference"])
            && t["reference"]
                == p["current_call"]["reference"]
                    .as_u64()
                    .map_or(json!(0), |n| json!(n))
            && word(&t["skin_file"])
            && word(&t["skin_reference"])
            && t["attached"] == false
            && t["callbacks_bound"] == false
            && t["pose_callback_assigned"] == false
            && t["semantic_flags"] == json!([0, 11])
            && t["component_flags"].as_u64().is_some_and(|n| n <= 255)
            && t["active_slots"] == 1
            && hash(&t["state_sha256"]),
        "temporary aliases a live animator, attachment or constructor state"
    );
    if !c["attachment"].is_null() {
        ensure!(
            count(t, "identity")? <= count(&c["attachment"], "registry_identities")?,
            "temporary identity is not in actual retained registry"
        );
    }
    for k in [
        "skeleton_bound",
        "skeleton_backlink_bound",
        "frame_stamp_known",
        "factory_returned",
        "head_inserted",
        "maintenance_skipped",
        "reverse_skipped",
        "refresh_returned",
    ] {
        ensure!(t[k].is_boolean(), "temporary owner effect flag absent");
    }
    ensure!(
        t["skeleton_backlink_bound"] == t["skeleton_bound"],
        "temporary skeleton backlink differs"
    );
    if t["skeleton_bound"] == true {
        ensure!(
            count(t, "bones")? <= 128 && count(t, "auxiliary_words")? <= 4096,
            "temporary skeleton limits differ"
        );
    } else {
        ensure!(
            t["bones"].is_null()
                && t["auxiliary_words"].is_null()
                && t["frame_stamp_known"] == false,
            "unbound temporary invents skeleton state"
        );
    }
    if (2..10).contains(&stage) {
        ensure!(
            t["skeleton_bound"] == true,
            "temporary factory bypassed skeleton bind"
        );
    }
    let forward_pending = t["forward"]["stage"] == "AwaitingBody";
    let expected = match stage {
        7 if forward_pending => Some("temporary_script_body_residency"),
        0 if count(t, "skin_reference")? >> 24 != 0x0d => Some("temporary_skin_remap"),
        0 | 1 => Some("temporary_skeleton"),
        2 if (count(t, "reference")? >> 24) & 0x7f == 3 => Some("temporary_factory_class"),
        4 if t["script"]["stage"] == "AwaitingScriptBody" => {
            Some("temporary_script_body_residency")
        }
        3 | 4 => Some("temporary_script_initializer"),
        6 | 7 => Some("temporary_forward_tick"),
        8 => Some("temporary_animation_evaluation"),
        11 => match t["evaluation"]["pending_boundary"].as_str() {
            Some("Prepare")
                if t["evaluation"]["prepares"]
                    .as_array()
                    .and_then(|a| a.last())
                    .is_some_and(|p| p["pause"] == "Auxiliary") =>
            {
                Some("temporary_evaluation_auxiliary_clear")
            }
            Some("Prepare") => Some("temporary_evaluation_skin"),
            Some("Node") => Some("temporary_evaluation_sampling"),
            Some(_) => Some("temporary_evaluation_layers"),
            None => None,
        },
        _ => None,
    };
    ensure!(
        match expected {
            Some(d) => t["pending_dependency"] == d,
            None => t["pending_dependency"].is_null(),
        },
        "temporary dependency differs from actual stage"
    );
    if stage < 6 {
        ensure!(
            t["factory_returned"] == false && t["head_inserted"] == false,
            "temporary factory return/insertion ran early"
        );
    }
    if (6..10).contains(&stage) {
        ensure!(
            t["factory_returned"] == true,
            "temporary forward precedes actual factory return"
        );
    }
    if t["head_inserted"] == true {
        ensure!(
            !t["script"].is_null() && t["factory_returned"] == true,
            "temporary head aliases an unreturned script"
        );
    }
    evaluation::validate(&t["evaluation"], t)?;
    if stage == 8 {
        ensure!(
            t["maintenance_skipped"] == true
                && t["reverse_skipped"] == t["head_inserted"]
                && t["bind_return"].is_null()
                && t["refresh_returned"] == false,
            "populated temporary claims bind/evaluation completion"
        );
    } else if stage == 9 {
        ensure!(
            t["bind_return"] == 1
                && if t["head_inserted"] == true {
                    t["refresh_returned"] == false && t["evaluation"]["stage"] == "returned"
                } else {
                    t["refresh_returned"] == true && t["evaluation"].is_null()
                },
            "empty temporary return lacks actual refresh"
        );
    } else {
        ensure!(
            t["bind_return"].is_null() && t["refresh_returned"] == false,
            "temporary bind returned early"
        );
    }
    let s = &t["script"];
    if !s.is_null() {
        closed(
            s,
            &[
                "stage",
                "slot",
                "factory_flags",
                "reference",
                "descriptor_index",
                "body_bound",
                "initializer_returned",
                "phase_zero",
                "clips",
                "state_sha256",
            ],
        )?;
        ensure!(
            s["reference"] == t["reference"]
                && s["slot"] == 0
                && s["factory_flags"] == 3
                && optional_index(&s["descriptor_index"], 32768)
                && (s["body_bound"].is_null() || s["body_bound"].is_boolean())
                && s["initializer_returned"].is_boolean()
                && hash(&s["state_sha256"]),
            "temporary direct script binding or flags differ"
        );
        ensure!(
            matches!(
                s["stage"].as_str(),
                Some(
                    "Constructed"
                        | "AwaitingScript"
                        | "ResolvingScript"
                        | "AwaitingScriptBody"
                        | "AwaitingInitializer"
                        | "Initializing"
                        | "AwaitingClipFile"
                        | "AwaitingClipRange"
                        | "AwaitingFrameCount"
                        | "AwaitingAnimation"
                        | "PreparingPosition"
                        | "AwaitingAnimationOwner"
                        | "AwaitingClipCache"
                        | "AwaitingAnimationBody"
                        | "AwaitingTimingCurve"
                        | "AwaitingRootMotion"
                        | "ReturnedWithoutPlay"
                        | "ReturnedInitialized"
                        | "ReturnedWithoutBody"
                        | "Failed"
                )
            ),
            "unknown temporary script stage"
        );
        let initializer_returned = matches!(
            s["stage"].as_str(),
            Some("ReturnedWithoutBody" | "ReturnedWithoutPlay" | "ReturnedInitialized")
        );
        ensure!(
            s["initializer_returned"] == initializer_returned,
            "temporary initializer return differs from actual retained script stage"
        );
        if !s["phase_zero"].is_null() {
            super::player_child::validate_phase_zero(&s["phase_zero"])?;
            ensure!(
                (s["body_bound"] == true || (forward_pending && s["body_bound"].is_null()))
                    && !s["descriptor_index"].is_null(),
                "temporary initializer bypassed actual script body"
            );
        }
        match s["stage"].as_str() {
            Some("AwaitingScriptBody") => ensure!(
                s["body_bound"].is_null()
                    && s["phase_zero"].is_null()
                    && s["initializer_returned"] == false
                    && !s["descriptor_index"].is_null(),
                "unknown script body became absent or initialized"
            ),
            Some("ReturnedInitialized") => ensure!(
                s["phase_zero"]["stage"] == "initialized",
                "temporary initialized return lacks phase-zero position receipt"
            ),
            Some("ReturnedWithoutPlay") => ensure!(
                s["phase_zero"]["stage"] == "returned_without_play",
                "temporary no-play return lacks source scan"
            ),
            Some("ReturnedWithoutBody") => ensure!(
                s["phase_zero"].is_null()
                    && s["initializer_returned"] == true
                    && (s["body_bound"] == false || (forward_pending && s["body_bound"].is_null())),
                "temporary no-body return consumed an unknown body"
            ),
            _ => {}
        }
        if t["head_inserted"] == true {
            ensure!(
                s["initializer_returned"] == true
                    && matches!(
                        s["stage"].as_str(),
                        Some("ReturnedWithoutPlay" | "ReturnedInitialized" | "ReturnedWithoutBody")
                    ),
                "inserted temporary script initializer did not return"
            );
        }
        let clips = s["clips"].as_array().context("temporary clips absent")?;
        ensure!(
            s["stage"] != "ReturnedWithoutBody" || clips.is_empty(),
            "NULL-body initializer consumed clip records"
        );
        ensure!(
            clips.len() <= 510
                && (s["phase_zero"].is_null()
                    || count(&s["phase_zero"], "clip_records")? == clips.len() as u64),
            "temporary clip owner count differs"
        );
        for clip in clips {
            closed(
                clip,
                &[
                    "file",
                    "reference",
                    "cache_generation",
                    "start_raw",
                    "end_raw",
                    "blend_bits",
                ],
            )?;
            for k in ["file", "reference", "cache_generation", "blend_bits"] {
                ensure!(
                    clip[k].is_null() || word(&clip[k]),
                    "temporary clip raw word malformed"
                );
            }
            for k in ["start_raw", "end_raw"] {
                ensure!(
                    clip[k].is_null()
                        || clip[k]
                            .as_i64()
                            .is_some_and(|n| (-32768..=32767).contains(&n)),
                    "temporary clip frame is not signed16"
                );
            }
        }
    }
    let f = &t["forward"];
    if !f.is_null() {
        closed(
            f,
            &[
                "stage",
                "phase_cleared",
                "body_bound",
                "duration_raw",
                "fps_bits",
                "result_bits",
            ],
        )?;
        ensure!(
            stage >= 7
                && t["head_inserted"] == true
                && f["phase_cleared"].is_boolean()
                && (f["body_bound"].is_null() || f["body_bound"].is_boolean()),
            "temporary forward has no real inserted head"
        );
        ensure!(
            matches!(
                f["stage"].as_str(),
                Some(
                    "Entering"
                        | "Resolving"
                        | "AwaitingBody"
                        | "Duration"
                        | "Fps"
                        | "Returned"
                        | "Failed"
                )
            ),
            "unknown temporary forward stage"
        );
        for k in ["duration_raw", "fps_bits", "result_bits"] {
            ensure!(
                f[k].is_null() || word(&f[k]),
                "temporary forward raw input malformed"
            );
        }
        if forward_pending {
            ensure!(
                stage == 7
                    && f["phase_cleared"] == true
                    && f["body_bound"].is_null()
                    && f["duration_raw"].is_null()
                    && f["fps_bits"].is_null()
                    && f["result_bits"].is_null()
                    && s["body_bound"].is_null()
                    && s["initializer_returned"] == true
                    && t["maintenance_skipped"] == false
                    && t["reverse_skipped"] == false,
                "pending forward body became absent, returned or advanced callbacks"
            );
        }
        if f["stage"] == "Returned" {
            ensure!(
                f["body_bound"].is_boolean(),
                "returned forward treated unknown body as absent"
            );
        }
        if stage == 8 {
            ensure!(
                f["stage"] == "Returned"
                    && f["phase_cleared"] == true
                    && f["body_bound"].is_boolean(),
                "temporary awaits evaluation before actual forward return"
            );
        }
        if f["stage"] == "Returned" && f["body_bound"] == true {
            ensure!(
                word(&f["duration_raw"]) && word(&f["fps_bits"]) && word(&f["result_bits"]),
                "temporary forward invented unknown timing result"
            );
        }
        if f["stage"] == "Returned" && f["body_bound"] == false {
            ensure!(
                f["duration_raw"].is_null()
                    && f["fps_bits"].is_null()
                    && f["result_bits"].is_null(),
                "NULL-body forward consumed skipped timing fields"
            );
        }
    } else if stage == 8 && t["head_inserted"] == true {
        anyhow::bail!("temporary inserted head has no actual forward trace");
    }
    Ok(())
}

pub(super) fn validate(p: &Value, c: &Value) -> Result<()> {
    if p.is_null() {
        ensure!(
            c["stage"] != "player_preload_temporary_pending",
            "temporary root has no preload owner"
        );
        return Ok(());
    }
    closed(
        p,
        &[
            "stage",
            "pending_dependency",
            "entry_flag",
            "cache_enabled",
            "raw_flags_written",
            "entry_keys_written",
            "registry_count",
            "source_row",
            "rows_visited",
            "enabled_rows",
            "current_weapon",
            "call_in_row",
            "completed_calls",
            "current_call",
            "allocations",
            "caches",
            "flag_cleared",
            "helper_returned",
            "state_sha256",
            "temporary",
            "post_bind",
            "script_publication",
            "outputs",
            "completion",
        ],
    )?;
    let stage = [
        "entry",
        "raw_flags",
        "entry_keys",
        "registry_count",
        "row",
        "weapon",
        "call",
        "awaiting_temporary",
        "final_flag",
        "returned",
    ]
    .iter()
    .position(|s| p["stage"] == *s)
    .context("unsupported preload stage")?;
    let prefix = &c["initializer_prefix"];
    ensure!(
        prefix["stage"] == "awaiting_preload"
            && prefix["cache_enabled"] == 1
            && prefix["invalidated"] == false
            && prefix["receivers_found"] == json!([true, true])
            && hash(&p["state_sha256"])
            && p["cache_enabled"].as_u64().is_some_and(|n| n <= 255)
            && p["flag_cleared"].is_boolean()
            && p["helper_returned"] == (stage == 9)
            && count(p, "raw_flags_written")? <= 32
            && count(p, "entry_keys_written")? <= 34
            && count(p, "call_in_row")? < 6,
        "preload owner bypassed historical initializer or source bounds"
    );
    ensure!(
        dependency(p).is_some_and(|d| p["pending_dependency"] == d && c["pending_dependency"] == d),
        "preload dependency disagrees with actual reached owner"
    );
    ensure!(
        c["stage"]
            == if stage == 7 {
                "player_preload_temporary_pending"
            } else {
                "player_animation_preload_pending"
            },
        "preload root differs"
    );
    if stage == 0 {
        ensure!(
            p["entry_flag"].is_null(),
            "preload entry flag was read early"
        );
    } else {
        ensure!(
            p["entry_flag"].as_u64().is_some_and(|n| n <= 255),
            "preload entry byte absent"
        );
    }
    if stage >= 2 && !(stage == 9 && p["entry_flag"] == 0) {
        ensure!(
            p["raw_flags_written"] == 32,
            "entry table precedes raw flags"
        );
    }
    if (3..9).contains(&stage) || p["flag_cleared"] == true {
        ensure!(
            p["entry_keys_written"] == 34,
            "registry precedes key prefix"
        );
    }
    let rows = count(p, "rows_visited")?;
    let enabled = count(p, "enabled_rows")?;
    let row = count(p, "source_row")?;
    if p["registry_count"].is_null() {
        ensure!(
            stage <= 3 || (stage == 9 && p["entry_flag"] == 0),
            "registry count missing after scan"
        );
        ensure!(
            rows == 0 && enabled == 0 && row == 0,
            "registry visits precede count"
        );
    } else {
        ensure!(
            count(p, "registry_count")? <= 1024
                && rows <= count(p, "registry_count")?
                && enabled <= rows
                && row <= rows,
            "registry source order or enabled visits differ"
        );
    }
    let done = p["completed_calls"]
        .as_array()
        .context("completed preload calls absent")?;
    ensure!(
        done.len() <= 6144 && (p["current_weapon"].is_null() || word(&p["current_weapon"])),
        "preload call budget or weapon differs"
    );
    let mut known = BTreeSet::new();
    for i in 0..count(p, "raw_flags_written")? {
        known.insert(16 * i + 4);
    }
    let mut calls: Vec<&Value> = done.iter().collect();
    if !p["current_call"].is_null() {
        calls.push(&p["current_call"]);
    }
    let mut last_row = None;
    for (i, v) in calls.iter().enumerate() {
        call(v, i, &mut known)?;
        let source = count(v, "source_ordinal")?;
        ensure!(
            source < count(p, "registry_count")? && last_row.is_none_or(|n| source >= n),
            "preload calls reorder source registry"
        );
        ensure!(
            last_row.is_none_or(|n| n == source || i % 6 == 0),
            "preload advanced source row before six returns"
        );
        let in_row = if last_row == Some(source) { i % 6 } else { 0 };
        ensure!(
            v["alternate"] == (in_row % 2 != 0) && count(v, "variant")? == (in_row / 2) as u64,
            "six preload calls reorder kind/variant"
        );
        if i < done.len() {
            ensure!(
                v["stage"] == "returned",
                "completed preload call lacks return"
            );
        }
        last_row = Some(source);
    }
    if let Some(current) = calls.get(done.len()) {
        ensure!(
            current["source_ordinal"] == p["source_row"]
                && current["weapon"] == p["current_weapon"]
                && count(p, "call_in_row")? == done.len() as u64 % 6,
            "current preload call lost its row input"
        );
    }
    if stage == 7 {
        ensure!(
            p["current_call"]["stage"] == "awaiting_temporary"
                && p["flag_cleared"] == false
                && p["cache_enabled"] == p["entry_flag"],
            "temporary wait committed preload prematurely"
        );
    }
    if p["flag_cleared"] == true {
        ensure!(
            stage == 9
                && row == count(p, "registry_count")?
                && done.len() as u64 == enabled * 6
                && p["current_call"].is_null()
                && p["cache_enabled"] == 0,
            "shared flag cleared before exhaustive child returns"
        );
    } else {
        ensure!(
            p["entry_flag"].is_null() || p["cache_enabled"] == p["entry_flag"],
            "preload source flag changed before return"
        );
    }
    let allocations = p["allocations"]
        .as_array()
        .filter(|a| a.len() == 3)
        .context("preload allocation owners absent")?;
    for (i, (kind, bytes, payload, header)) in [
        ("raw", 512, 512, json!([null, null, null, null])),
        ("array34", 7088, 7072, json!([208, 34, null, null])),
        ("array64", 56080, 56064, json!([876, 64, null, null])),
    ]
    .into_iter()
    .enumerate()
    {
        let a = &allocations[i];
        if a.is_null() {
            ensure!(
                stage <= 2 || p["entry_flag"] == 0,
                "preload reached a missing allocation"
            );
            continue;
        }
        closed(
            a,
            &[
                "kind",
                "native_bytes",
                "payload_bytes",
                "known_payload_bytes",
                "header_words",
            ],
        )?;
        ensure!(
            a["kind"] == kind
                && a["native_bytes"] == bytes
                && a["payload_bytes"] == payload
                && a["header_words"] == header,
            "preload payload/header identity differs"
        );
        let n = count(a, "known_payload_bytes")?;
        ensure!(
            match i {
                0 => n >= known.len() as u64 && n <= 416,
                1 => n >= 4 * count(p, "entry_keys_written")? && n <= 7072,
                _ => n == 0,
            },
            "preload invented unwritten cache payload bytes"
        );
    }
    player_initializer_prefix::validate_preload_caches(&p["caches"], prefix)?;
    let mut current = p["caches"].clone();
    let historical = &prefix["caches"];
    let invalidations = calls.iter().filter(|v| v["invalidated"] == true).count() as u64;
    for i in 0..2 {
        for k in ["invalidation_count_reads", "invalidation_pointer_reads"] {
            let selected = historical["selected"]
                == if i == 0 {
                    "first_person"
                } else {
                    "third_person"
                };
            ensure!(
                count(&current["headers"][i], k)?
                    == count(&historical["headers"][i], k)?
                        + if selected { invalidations } else { 0 },
                "preload validity read history differs from actual calls"
            );
            current["headers"][i][k] = historical["headers"][i][k].clone();
        }
    }
    current["state_sha256"] = historical["state_sha256"].clone();
    ensure!(
        current == *historical,
        "preload rewrote immutable cache build or transform payload"
    );
    if !p["temporary"].is_null() {
        ensure!(
            stage == 7,
            "temporary allocated before actual extractor return"
        );
        temporary(&p["temporary"], p, c)?;
    }
    script_publication(&p["script_publication"], &p["temporary"])?;
    post_bind::validate(&p["post_bind"], &p["temporary"])?;
    completion::validate(p, c)?;
    Ok(())
}

/// Measured Tutorial progress: 32 committed temporaries precede a resident sampling dependency.
pub(super) fn validate_native(p: &Value, c: &Value, expected_player: u64) -> Result<()> {
    validate(p, c)?;
    ensure!(
        p["stage"] == "awaiting_temporary"
            && p["pending_dependency"] == "preload_temporary_evaluation_sampling"
            && p["entry_flag"] == 1
            && p["cache_enabled"] == 1
            && p["raw_flags_written"] == 32
            && p["entry_keys_written"] == 34
            && p["registry_count"] == 38
            && p["source_row"] == 36
            && p["rows_visited"] == 37
            && p["enabled_rows"] == 28
            && p["current_weapon"] == 0x0200_0551
            && p["call_in_row"] == 0
            && p["completed_calls"]
                .as_array()
                .is_some_and(|a| a.len() == 162)
            && p["flag_cleared"] == false
            && p["helper_returned"] == false
            && p["allocations"][0]["known_payload_bytes"] == 368
            && p["allocations"][1]["known_payload_bytes"] == 6664
            && p["allocations"][2]["known_payload_bytes"] == 0,
        "Tutorial preload resumed-loop progress or source flag differs"
    );
    let progress = &p["completion"];
    ensure!(
        progress["completed_temporary_count"] == 32
            && progress["discarded_temporary_count"] == 16
            && progress["found_output_count"] == 192
            && progress["missing_output_count"] == 0
            && progress["destroyed_temporary_count"] == 32
            && progress["entry_commit_count"] == 32
            && progress["history"]
                .as_array()
                .is_some_and(|a| a.len() == 16)
            && progress["pending_snapshot"].is_null()
            && progress["destruction"].is_null()
            && p["outputs"].is_null()
            && p["post_bind"].is_null()
            && p["script_publication"].is_null(),
        "Tutorial completed temporary/output/destructor/commit accounting differs"
    );
    let first = &progress["first_completed_snapshot"];
    ensure!(
        first["ordinal"] == 0
            && count(first, "temporary_identity")? >= 3
            && count(first, "receiver")?.checked_add(1)
                == Some(count(first, "temporary_identity")?)
            && first["outputs"]["found"] == 6
            && first["outputs"]["missing"] == 0
            && first["outputs"]["stores"] == 30,
        "Tutorial original temporary proof is missing"
    );
    validate_native_first(first, &p["completed_calls"][0], c, expected_player)?;
    validate_native_current(p, c, expected_player)
}
fn validate_native_current(p: &Value, c: &Value, expected_player: u64) -> Result<()> {
    let call = &p["current_call"];
    let t = &p["temporary"];
    let e = &t["evaluation"];
    let first = &p["completion"]["first_completed_snapshot"];
    ensure!(
        call["stage"] == "awaiting_temporary"
            && call["ordinal"] == 162
            && call["source_ordinal"] == 36
            && call["weapon"] == 0x0200_0551
            && call["generic"] == 0x0900_0765
            && call["alternate"] == false
            && call["variant"] == 0
            && call["entry_index"] == 32
            && call["mapping"]["output"] == 0x0900_086d
            && call["reference"] == 0x8400_00b7u32
            && call["selected_key_empty"] == true
            && call["invalidated"] == true
            && call["return_word"].is_null(),
        "Tutorial current call is not the measured resident animation dependency"
    );
    ensure!(
        call["extraction"]
            == json!({"requested_index":41,"baseline_index":0,"transition_ordinal":42,"command_ordinal":0,"set_index":21,"group_ordinal":0,"child_ordinal":0,"transitions_read":43,"commands_read":1,"groups_read":1,"children_read":1}),
        "Tutorial current resident reference extraction differs"
    );
    ensure!(
        t["stage"] == "evaluating_bind"
            && t["pending_dependency"] == "temporary_evaluation_sampling"
            && count(first, "temporary_identity")?.checked_add(32) == Some(count(t, "identity")?)
            && t["source_receiver"] == first["receiver"]
            && t["player"] == expected_player
            && t["skin_file"] == 0x010a_0050
            && t["skin_reference"] == 0x0d00_016f
            && t["reference"] == call["reference"]
            && t["bones"] == 48
            && t["auxiliary_words"] == 0
            && t["factory_returned"] == true
            && t["head_inserted"] == true
            && t["bind_return"].is_null()
            && c["attachment"]["registry_identities"] == t["identity"]
            && c["attachment"]["item_components"] == 2,
        "Tutorial current temporary is not the live distinct sampling owner"
    );
    let script = &t["script"];
    ensure!(
        script["stage"] == "ReturnedInitialized"
            && script["descriptor_index"] == 183
            && script["reference"] == call["reference"]
            && script["body_bound"] == true
            && script["initializer_returned"] == true
            && script["phase_zero"]["stage"] == "initialized"
            && script["phase_zero"]["heap_allocated"] == true
            && script["phase_zero"]["normal_bound"] == 4
            && script["phase_zero"]["clip_records"] == 4
            && script["clips"].as_array().is_some_and(|a| a.len() == 4)
            && t["forward"]
                == json!({"stage":"Returned","phase_cleared":true,"body_bound":true,"duration_raw":33,"fps_bits":0x41f00000,"result_bits":0}),
        "Tutorial current resident initializer/forward prefix differs"
    );
    ensure!(
        e["stage"] == "pending"
            && e["pending_boundary"] == "Node"
            && e["reserved"] == true
            && e["released"] == false
            && e["lease_live"] == true
            && e["aborted"] == false
            && e["arena_owner"] == 1
            && e["reservation_start"] == 0
            && e["cursor_after"].is_null()
            && e["live_after"].is_null()
            && e["node_visits"] == 0
            && e["nonempty_layers"] == 0
            && e["successful_samples"] == 0
            && e["advanced_context"] == false
            && e["commit_register"] == 0
            && e["blended_layers"] == 0
            && e["committed"] == false,
        "Tutorial resident evaluation fabricated sampling/release/commit"
    );
    ensure!(
        e["slots"]
            == json!([{"slot":0,"weight_bits":0x3f800000,"branch":"nonempty","tail":1,"prepared":true,"masks_cleared":true,"nodes":[],"successes":0,"flags_after":null}])
            && e["prepares"]
                == json!([{"file":0x010a0050,"reference":0x0d00016f,"generation":0,"descriptor_bound":true,"body_resident":true,"auxiliary_words":0,"lookups":1,"body_reads":1,"returned":"AuxiliaryZero","pause":null}])
            && e["guards"]
                == json!([{"node":1,"pointer":"record","record":0,"clip_reference":0x83000019u32,"animator_reference_read":true,"animator_reference":0x840000b7u32,"result":null,"pending_boundary":"Sampling"}]),
        "Tutorial current node did not stop at actual resident clip sampling"
    );
    for snapshot in std::iter::once(&p["completion"]["first_completed_snapshot"]).chain(
        p["completion"]["history"]
            .as_array()
            .context("history missing")?
            .iter()
            .map(|h| &h["snapshot"]),
    ) {
        let earlier = &snapshot["post_bind"]["evaluation"];
        ensure!(
            snapshot["player"] == expected_player
                && snapshot["receiver"] == first["receiver"]
                && earlier["arena_owner"] == e["arena_owner"]
                && earlier["stage"] == "returned"
                && earlier["released"] == true
                && earlier["lease_live"] == false
                && count(e, "ordinal")? > count(earlier, "ordinal")?
                && count(e, "allocation_ordinal")? > count(earlier, "allocation_ordinal")?,
            "current evaluation reused an earlier released scope"
        );
    }
    Ok(())
}
fn validate_native_first(p: &Value, call: &Value, c: &Value, expected_player: u64) -> Result<()> {
    ensure!(
        call["stage"] == "returned"
            && call["ordinal"] == 0
            && call["source_ordinal"] == 2
            && call["weapon"] == 0x0200_016e
            && call["generic"] == 0x0900_0765
            && call["alternate"] == false
            && call["variant"] == 0
            && call["mapping"]["output"] == 0x0900_09a1
            && call["mapping"]["mapped"] == true
            && call["mapping"]["steps"] == 1
            && call["mapping"]["map_rows_read"] == 1
            && call["raw_flags_read"] == 1
            && call["raw_keys_read"] == 0
            && call["raw_index"] == 0
            && call["raw_indices_written"] == true
            && call["entry_keys_read"] == 2
            && call["entry_index"] == 0
            && call["occupied_written"] == true
            && call["weapon_written"] == true
            && call["mapping_written"] == true
            && call["selected_key_empty"] == true
            && call["invalidated"] == true
            && call["extraction_returned"] == true
            && call["reference_outcome"] == "class_four"
            && call["reference"] == 0x8400_00efu32
            && call["return_word"] == 1
            && call["return_reason"] == "temporary_completed",
        "Tutorial actual first mapping/extractor/cache-call prefix differs"
    );
    let e = &call["extraction"];
    ensure!(
        e["requested_index"] == 86
            && e["baseline_index"] == 0
            && e["transition_ordinal"] == 8
            && e["command_ordinal"] == 0
            && e["set_index"] == 220
            && e["group_ordinal"] == 0
            && e["child_ordinal"] == 0
            && e["transitions_read"] == 9
            && e["commands_read"] == 1
            && e["groups_read"] == 1
            && e["children_read"] == 1,
        "Tutorial class-four extraction does not match actual reached source order"
    );
    let t = &p["temporary"];
    ensure!(
        t["stage"] == "bind_returned"
            && t["pending_dependency"].is_null()
            && t["identity"] == p["temporary_identity"]
            && t["source_receiver"] == p["receiver"]
            && t["player"] == expected_player
            && t["component_file"] == 0x0102_0016
            && t["reference"] == call["reference"]
            && t["skin_file"] == 0x010a_0050
            && t["skin_reference"] == 0x0d00_016f
            && t["skeleton_bound"] == true
            && t["bones"] == 48
            && t["auxiliary_words"] == 0
            && t["skeleton_backlink_bound"] == true
            && t["attached"] == false
            && t["callbacks_bound"] == false
            && t["pose_callback_assigned"] == false
            && t["frame_stamp_known"] == false
            && t["semantic_flags"] == json!([0, 11])
            && t["component_flags"] == 0
            && t["active_slots"] == 1
            && t["factory_returned"] == true
            && t["head_inserted"] == true
            && t["forward"]
                == json!({"stage":"Returned","phase_cleared":true,
                "body_bound":false,"duration_raw":null,"fps_bits":null,"result_bits":null})
            && t["maintenance_skipped"] == true
            && t["reverse_skipped"] == true
            && t["bind_return"] == 1
            && t["refresh_returned"] == false
            && count(&c["attachment"], "registry_identities")? >= 3
            && c["attachment"]["item_components"] == 2,
        "Tutorial temporary owner or completed inner bind differs"
    );
    let s = &t["script"];
    ensure!(
        s["stage"] == "ReturnedWithoutBody"
            && s["slot"] == 0
            && s["factory_flags"] == 3
            && s["reference"] == call["reference"]
            && s["descriptor_index"] == 239
            && s["body_bound"] == false
            && s["initializer_returned"] == true
            && s["phase_zero"].is_null()
            && s["clips"] == json!([]),
        "Tutorial fresh NULL script skipped or invented initializer work"
    );
    ensure!(
        p["script_publication"]
            == json!({"file":0x0102_0016,"generation":0,
            "descriptor_index":239,"section_index":9,"section_key":0x0800_0009,
            "callback_mask":0x819c,"fresh_base":true,"request_count":0,
            "active_requests":0,"retained_sections":0,"history_unknown":false,
            "raw_job":0,"body_state":"known_null","native_history_verified":false}),
        "Tutorial current same-owner publication proof differs"
    );
    evaluation::validate_native(&t["evaluation"], t, c)?;
    post_bind::validate_native(&p["post_bind"], t, c)?;
    Ok(())
}

#[cfg(test)]
pub(super) mod tests;
