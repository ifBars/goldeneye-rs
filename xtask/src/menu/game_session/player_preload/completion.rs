use super::*;

pub(super) fn output_dependency(v: &Value) -> Option<&'static str> {
    match v["pending_dependency"].as_str() {
        Some("preload_bone_output_skin_lookup") => Some("preload_bone_output_skin_lookup"),
        Some("preload_bone_output_skin_body") => Some("preload_bone_output_skin_body"),
        Some("preload_bone_output_tag") => Some("preload_bone_output_tag"),
        Some("preload_temporary_destructor") => Some("preload_temporary_destructor"),
        None => Some("preload_temporary_bone_outputs"),
        _ => None,
    }
}
pub(super) fn dependency(v: &Value) -> Option<&'static str> {
    if v["stage"] == "destroyed" {
        return Some("preload_temporary_entry_commit");
    }
    match v["pending_dependency"].as_str() {
        None => Some("preload_temporary_destructor"),
        Some("preload_teardown_attachment_list") => Some("preload_teardown_attachment_list"),
        Some("preload_teardown_extra_owner") => Some("preload_teardown_extra_owner"),
        Some("preload_teardown_active_count") => Some("preload_teardown_active_count"),
        Some("preload_teardown_slot_list") => Some("preload_teardown_slot_list"),
        Some("preload_teardown_control_node") => Some("preload_teardown_control_node"),
        Some("preload_teardown_child_state") => Some("preload_teardown_child_state"),
        Some("preload_teardown_child_reference_list") => {
            Some("preload_teardown_child_reference_list")
        }
        Some("preload_teardown_linked_base_child") => Some("preload_teardown_linked_base_child"),
        Some("preload_teardown_skeleton_pointer") => Some("preload_teardown_skeleton_pointer"),
        Some("preload_teardown_base_owner") => Some("preload_teardown_base_owner"),
        Some("preload_teardown_nonterminating_base") => {
            Some("preload_teardown_nonterminating_base")
        }
        _ => None,
    }
}
fn outputs(v: &Value) -> Result<()> {
    closed(
        v,
        &[
            "stage",
            "pending_dependency",
            "iteration",
            "hash_cursor",
            "hash_reads",
            "lookups",
            "body_reads",
            "tag_walks",
            "stores",
            "found",
            "missing",
            "outcomes",
            "state_sha256",
        ],
    )?;
    ensure!(
        hash(&v["state_sha256"]) && output_dependency(v).is_some(),
        "bone output hash/dependency malformed"
    );
    ensure!(
        matches!(
            v["stage"].as_str(),
            Some(
                "hash"
                    | "lookup"
                    | "body"
                    | "tag"
                    | "store_index"
                    | "store_translation_low"
                    | "store_translation_high"
                    | "store_rotation_low"
                    | "store_rotation_high"
                    | "done"
                    | "failed"
                    | "aborted"
            )
        ),
        "unknown bone output stage"
    );
    let a = v["outcomes"]
        .as_array()
        .filter(|a| a.len() == 6)
        .context("six output outcomes absent")?;
    let found = a.iter().filter(|o| **o == "found").count() as u64;
    let missing = a.iter().filter(|o| !o.is_null() && **o != "found").count() as u64;
    ensure!(
        a.iter().all(|o| o.is_null()
            || matches!(
                o.as_str(),
                Some("found" | "zero_hash" | "descriptor_absent" | "body_absent" | "sentinel")
            ))
            && count(v, "found")? == found
            && count(v, "missing")? == missing
            && count(v, "iteration")? <= 6
            && count(v, "hash_cursor")? <= 6
            && count(v, "stores")? <= 30,
        "bone output outcomes/counts differ"
    );
    ensure!(
        count(v, "hash_reads")? <= 6
            && word(&v["lookups"])
            && word(&v["body_reads"])
            && word(&v["tag_walks"])
            && (count(v, "lookups")? == 0 || count(v, "hash_reads")? > 0)
            && (count(v, "body_reads")? == 0 || count(v, "lookups")? > 0)
            && (count(v, "tag_walks")? == 0 || count(v, "body_reads")? > 0),
        "bone output lazy reads differ"
    );
    if v["stage"] == "done" {
        ensure!(
            count(v, "iteration")? == 6
                && found + missing == 6
                && count(v, "stores")? == 5 * found + missing
                && v["pending_dependency"] == "preload_temporary_destructor",
            "incomplete six-output return"
        );
    }
    Ok(())
}
fn destruction(v: &Value, s: &Value) -> Result<()> {
    closed(
        v,
        &[
            "stage",
            "pending_dependency",
            "temporary_identity",
            "ordinal",
            "children",
            "skeleton",
            "snapshot_words",
            "snapshot_digest",
            "state_sha256",
        ],
    )?;
    ensure!(
        dependency(v).is_some()
            && v["temporary_identity"] == s["temporary_identity"]
            && v["ordinal"] == s["ordinal"]
            && hash(&v["state_sha256"])
            && count(v, "snapshot_words")? > 0
            && v["snapshot_digest"]
                .as_str()
                .is_some_and(|s| s.len() == 16 && s.bytes().all(|c| c.is_ascii_hexdigit()))
            && count(v, "children")? <= 1,
        "destructor identity/snapshot differs"
    );
    ensure!(
        matches!(
            v["stage"].as_str(),
            Some(
                "attachment_list"
                    | "extra_owner"
                    | "slot_children"
                    | "skeleton"
                    | "component_words"
                    | "attachment_list_again"
                    | "base_destructor"
                    | "awaiting_free"
                    | "destroyed"
                    | "failed"
                    | "aborted"
            )
        ),
        "unknown destructor stage"
    );
    if !v["skeleton"].is_null() {
        closed(&v["skeleton"], &["bones", "auxiliary_words", "block_bytes"])?;
        ensure!(
            v["skeleton"]["bones"] == s["temporary"]["bones"]
                && v["skeleton"]["auxiliary_words"] == s["temporary"]["auxiliary_words"]
                && count(&v["skeleton"], "block_bytes")? > 0,
            "released skeleton receipt differs"
        );
    }
    if v["stage"] == "destroyed" {
        ensure!(
            v["pending_dependency"].is_null() && !v["skeleton"].is_null(),
            "destroyed temporary lacks completed skeleton release"
        );
    }
    Ok(())
}
fn snapshot(s: &Value, p: &Value, c: &Value) -> Result<()> {
    closed(
        s,
        &[
            "ordinal",
            "source_ordinal",
            "player",
            "receiver",
            "weapon",
            "generic",
            "alternate",
            "variant",
            "entry",
            "mapped",
            "reference",
            "temporary_identity",
            "temporary",
            "post_bind",
            "outputs",
            "script_publication",
        ],
    )?;
    let ordinal = count(s, "ordinal")? as usize;
    let call = p["completed_calls"]
        .as_array()
        .and_then(|a| a.get(ordinal))
        .unwrap_or(&p["current_call"]);
    ensure!(
        !call.is_null() && call["ordinal"] == s["ordinal"],
        "snapshot lacks matching call"
    );
    for k in [
        "source_ordinal",
        "weapon",
        "generic",
        "alternate",
        "variant",
    ] {
        ensure!(call[k] == s[k], "snapshot call request differs");
    }
    ensure!(
        s["entry"] == call["entry_index"]
            && s["mapped"] == call["mapping"]["output"]
            && s["reference"] == call["reference"].as_u64().map_or(json!(0), |n| json!(n))
            && s["temporary_identity"] == s["temporary"]["identity"]
            && s["receiver"] == s["temporary"]["source_receiver"]
            && s["player"] == s["temporary"]["player"],
        "snapshot binding differs"
    );
    let mut historical = p.clone();
    historical["current_call"] = call.clone();
    temporary(&s["temporary"], &historical, c)?;
    post_bind::validate(&s["post_bind"], &s["temporary"])?;
    script_publication(&s["script_publication"], &s["temporary"])?;
    outputs(&s["outputs"])?;
    ensure!(
        s["outputs"]["stage"] == "done" && s["post_bind"]["stage"] == "awaiting_bone_outputs",
        "pre-destruction snapshot precedes outputs"
    );
    Ok(())
}
pub(super) fn validate(p: &Value, c: &Value) -> Result<()> {
    let v = &p["completion"];
    closed(
        v,
        &[
            "history_limit",
            "completed_temporary_count",
            "discarded_temporary_count",
            "found_output_count",
            "missing_output_count",
            "destroyed_temporary_count",
            "entry_commit_count",
            "history",
            "pending_snapshot",
            "first_completed_snapshot",
            "destruction",
        ],
    )?;
    let history = v["history"]
        .as_array()
        .context("temporary history absent")?;
    let completed = count(v, "completed_temporary_count")?;
    let first = &v["first_completed_snapshot"];
    if completed == 0 {
        ensure!(
            first.is_null(),
            "first completed snapshot precedes temporary return"
        );
    } else {
        snapshot(first, p, c)?;
        let earliest = p["completed_calls"]
            .as_array()
            .context("preload calls absent")?
            .iter()
            .find(|call| call["return_reason"] == "temporary_completed")
            .context("first completed temporary call absent")?;
        ensure!(
            first["ordinal"] == earliest["ordinal"],
            "first completed snapshot is not the actual earliest temporary call"
        );
        for retained in history {
            if retained["snapshot"]["ordinal"] == first["ordinal"] {
                ensure!(
                    retained["snapshot"] == *first,
                    "first completed snapshot differs from retained history"
                );
            }
        }
    }
    let commits = p["completed_calls"]
        .as_array()
        .context("preload calls absent")?
        .iter()
        .filter(|c| c["return_reason"] == "temporary_completed")
        .count() as u64;
    ensure!(
        v["history_limit"] == 16
            && history.len() <= 16
            && history.len() as u64 + count(v, "discarded_temporary_count")? == completed
            && count(v, "entry_commit_count")? == commits
            && completed == commits
            && count(v, "destroyed_temporary_count")?
                == completed + u64::from(v["destruction"]["stage"] == "destroyed"),
        "temporary history/destruction/commit accounting differs"
    );
    ensure!(
        count(v, "found_output_count")? + count(v, "missing_output_count")?
            == 6 * (completed + u64::from(!v["pending_snapshot"].is_null())),
        "output aggregate differs from completed helpers"
    );
    let mut previous = None;
    for h in history {
        closed(h, &["snapshot", "destruction"])?;
        let s = &h["snapshot"];
        snapshot(s, p, c)?;
        destruction(&h["destruction"], s)?;
        let ordinal = count(s, "ordinal")?;
        ensure!(
            previous.is_none_or(|old| ordinal > old)
                && h["destruction"]["stage"] == "destroyed"
                && p["completed_calls"][ordinal as usize]["return_reason"] == "temporary_completed",
            "history is reordered or lacks key commit"
        );
        previous = Some(ordinal);
    }
    if !p["outputs"].is_null() {
        outputs(&p["outputs"])?;
        ensure!(
            p["stage"] == "awaiting_temporary" && !p["post_bind"].is_null(),
            "outputs lack current post-bind call"
        );
    }
    if !v["pending_snapshot"].is_null() {
        snapshot(&v["pending_snapshot"], p, c)?;
        ensure!(
            v["pending_snapshot"]["ordinal"] == p["current_call"]["ordinal"],
            "pending snapshot belongs to old call"
        );
        for k in ["temporary", "post_bind", "outputs", "script_publication"] {
            ensure!(
                v["pending_snapshot"][k] == p[k],
                "pending snapshot and current historical receipt differ"
            );
        }
    }
    if !v["destruction"].is_null() {
        ensure!(
            !v["pending_snapshot"].is_null(),
            "destructor lost pre-destruction snapshot"
        );
        destruction(&v["destruction"], &v["pending_snapshot"])?;
    }
    if !p["allocations"][1].is_null() {
        ensure!(
            count(&p["allocations"][1], "known_payload_bytes")?
                <= 136
                    + 34 * count(v, "found_output_count")?
                    + 2 * count(v, "missing_output_count")?
                    + if v["pending_snapshot"].is_null() && !p["outputs"].is_null() {
                        8 * count(&p["outputs"], "stores")?
                    } else {
                        0
                    },
            "Array34 known bytes lack reached output stores"
        );
    }
    Ok(())
}
