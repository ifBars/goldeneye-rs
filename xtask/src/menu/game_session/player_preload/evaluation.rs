//! Validate actual nonempty evaluation independently of an empty-list refresh.
use super::{closed, count, hash, word};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(super) fn validate(v: &Value, temporary: &Value) -> Result<()> {
    validate_scope(v, temporary, false)
}
pub(super) fn validate_caller(v: &Value, temporary: &Value) -> Result<()> {
    validate_scope(v, temporary, true)
}
fn validate_scope(v: &Value, temporary: &Value, caller: bool) -> Result<()> {
    if v.is_null() {
        ensure!(
            temporary["stage"] != "evaluating_bind" && temporary["stage"] != "aborted",
            "evaluation stage lacks retained trace"
        );
        return Ok(());
    }
    closed(
        v,
        &[
            "stage",
            "pending_boundary",
            "reserved",
            "released",
            "lease_live",
            "aborted",
            "arena_owner",
            "ordinal",
            "allocation_ordinal",
            "reservation_start",
            "cursor_after",
            "live_after",
            "slots",
            "prepares",
            "guards",
            "node_visits",
            "nonempty_layers",
            "successful_samples",
            "advanced_context",
            "commit_register",
            "blended_layers",
            "committed",
            "state_sha256",
        ],
    )?;
    ensure!(
        temporary["skeleton_bound"] == true
            && (temporary["head_inserted"] == true || caller)
            && temporary["factory_returned"] == true
            && temporary["maintenance_skipped"] == true
            && temporary["reverse_skipped"] == temporary["head_inserted"]
            && temporary["refresh_returned"] == (temporary["head_inserted"] == false),
        "nonempty evaluation borrowed an empty or unreturned owner"
    );
    for k in [
        "reserved",
        "released",
        "lease_live",
        "aborted",
        "advanced_context",
        "committed",
    ] {
        ensure!(v[k].is_boolean(), "evaluation effect flag missing");
    }
    ensure!(
        matches!(
            v["stage"].as_str(),
            Some("running" | "pending" | "returned" | "failed" | "aborted")
        ) && hash(&v["state_sha256"]),
        "evaluation stage/hash invalid"
    );
    for k in [
        "node_visits",
        "nonempty_layers",
        "successful_samples",
        "blended_layers",
    ] {
        count(v, k)?;
    }
    ensure!(
        word(&v["commit_register"])
            && v["successful_samples"] == 0
            && v["blended_layers"] == 0
            && v["committed"] == false,
        "unsupported sampling/blend/commit claimed"
    );
    ensure!(
        v["reserved"] == true
            && count(v, "arena_owner")? > 0
            && count(v, "ordinal")? > 0
            && count(v, "allocation_ordinal")? > 0
            && count(v, "reservation_start")? % 16 == 0,
        "nonempty evaluation lacks actual aligned scope"
    );
    let slots = v["slots"].as_array().context("evaluation slots absent")?;
    let prepares = v["prepares"]
        .as_array()
        .context("evaluation preparations absent")?;
    let guards = v["guards"].as_array().context("evaluation guards absent")?;
    ensure!(
        slots.len() <= 1 && prepares.len() <= 1 && guards.len() <= 4096,
        "temporary evaluation limits exceeded"
    );
    let mut visits = 0;
    for s in slots {
        closed(
            s,
            &[
                "slot",
                "weight_bits",
                "branch",
                "tail",
                "prepared",
                "masks_cleared",
                "nodes",
                "successes",
                "flags_after",
            ],
        )?;
        ensure!(
            s["slot"] == 0
                && (s["weight_bits"].is_null() || word(&s["weight_bits"]))
                && s["prepared"].is_boolean()
                && s["masks_cleared"].is_boolean()
                && s["successes"] == 0
                && (s["flags_after"].is_null() || word(&s["flags_after"])),
            "evaluation slot malformed"
        );
        ensure!(
            s["branch"].is_null()
                || matches!(
                    s["branch"].as_str(),
                    Some("nonempty" | "empty_tail" | "low_weight")
                ),
            "evaluation branch unknown"
        );
        ensure!(
            s["masks_cleared"] != true || s["prepared"] == true,
            "masks cleared before skin preparation"
        );
        let nodes = s["nodes"].as_array().context("node walk absent")?;
        ensure!(
            nodes.is_empty()
                || s["prepared"] == true && s["masks_cleared"] == true && s["branch"] == "nonempty",
            "node invoked before preparation/mask stores"
        );
        for n in nodes {
            closed(n, &["node", "result"])?;
            ensure!(
                count(n, "node")? > 0 && n["result"] == 0 && n["node"] == s["tail"],
                "node return differs from real sole tail"
            );
        }
        visits += nodes.len();
    }
    ensure!(
        count(v, "node_visits")? == visits as u64,
        "evaluation node count fabricated"
    );
    for p in prepares {
        closed(
            p,
            &[
                "file",
                "reference",
                "generation",
                "descriptor_bound",
                "body_resident",
                "auxiliary_words",
                "lookups",
                "body_reads",
                "returned",
                "pause",
            ],
        )?;
        ensure!(
            p["file"] == temporary["skin_file"]
                && p["reference"] == temporary["skin_reference"]
                && p["descriptor_bound"].is_boolean()
                && p["body_resident"].is_boolean()
                && count(p, "lookups")? > 0,
            "skin preparation owner differs"
        );
        count(p, "body_reads")?;
        if p["returned"] == "AuxiliaryZero" {
            ensure!(
                p["descriptor_bound"] == true
                    && p["body_resident"] == true
                    && p["auxiliary_words"] == 0
                    && p["pause"].is_null()
                    && word(&p["generation"])
                    && count(p, "body_reads")? > 0,
                "skin preparation lacks actual body/auxiliary gate"
            );
        } else {
            ensure!(
                p["returned"].is_null()
                    || p["returned"] == "DescriptorAbsent"
                    || p["returned"] == "BodyAbsent",
                "unknown preparation return"
            );
        }
    }
    for g in guards {
        closed(
            g,
            &[
                "node",
                "pointer",
                "record",
                "clip_reference",
                "animator_reference_read",
                "animator_reference",
                "result",
                "pending_boundary",
            ],
        )?;
        ensure!(
            count(g, "node")? > 0
                && matches!(
                    g["pointer"].as_str(),
                    Some("constructor_inline" | "record" | "null" | "unknown")
                )
                && (g["result"].is_null() || g["result"] == 0)
                && g["animator_reference_read"].is_boolean()
                && (g["clip_reference"].is_null() || word(&g["clip_reference"]))
                && (g["animator_reference"].is_null() || word(&g["animator_reference"])),
            "guard fields malformed"
        );
        if g["result"] == 0 {
            ensure!(
                g["pending_boundary"].is_null()
                    && slots.iter().any(|s| s["nodes"]
                        .as_array()
                        .is_some_and(|nodes| nodes.iter().any(|n| n["node"] == g["node"])))
                    && (g["clip_reference"] == u32::MAX
                        || g["animator_reference_read"] == true
                            && g["animator_reference"] == u32::MAX),
                "sample return lacks actual sentinel guard"
            );
        }
        if g["clip_reference"] == u32::MAX {
            ensure!(
                g["animator_reference_read"] == false && g["animator_reference"].is_null(),
                "sentinel guard read later animator reference"
            );
        }
    }
    if caller && temporary["head_inserted"] == false {
        ensure!(
            prepares.is_empty()
                && guards.is_empty()
                && v["node_visits"] == 0
                && v["nonempty_layers"] == 0
                && v["advanced_context"] == false,
            "empty caller fabricated nonempty work"
        );
        if v["stage"] == "returned" {
            ensure!(
                v["pending_boundary"].is_null()
                    && slots.len() == 1
                    && slots[0]["prepared"] == false
                    && slots[0]["masks_cleared"] == false
                    && slots[0]["tail"].is_null()
                    && slots[0]["nodes"]
                        .as_array()
                        .is_some_and(|nodes| nodes.is_empty())
                    && slots[0]["branch"] == "empty_tail"
                    && slots[0]["flags_after"] == 0
                    && v["released"] == true
                    && v["lease_live"] == false
                    && v["aborted"] == false
                    && v["cursor_after"] == v["reservation_start"]
                    && v["live_after"] == 0
                    && v["commit_register"] == 0,
                "empty caller omitted own scope release"
            );
            return Ok(());
        }
    }
    if v["stage"] == "returned" {
        ensure!(
            v["pending_boundary"].is_null()
                && v["released"] == true
                && v["lease_live"] == false
                && v["aborted"] == false
                && v["nonempty_layers"] == 1
                && v["node_visits"] == 1
                && v["advanced_context"] == true
                && v["commit_register"] == 0
                && v["cursor_after"] == v["reservation_start"]
                && v["live_after"] == 0,
            "evaluation return lacks matching release or source effects"
        );
        ensure!(
            prepares.len() == 1
                && !prepares[0]["returned"].is_null()
                && slots.len() == 1
                && slots[0]["flags_after"] == 0
                && guards.iter().filter(|g| g["result"] == 0).count() == visits,
            "evaluation return omitted actual prepare/node/layer effects"
        );
    } else {
        ensure!(
            v["released"] == false && (caller || temporary["bind_return"].is_null()),
            "pending/aborted evaluation fabricated return"
        );
        if v["stage"] == "aborted" {
            ensure!(
                v["aborted"] == true && v["lease_live"] == false,
                "abort did not release its lease"
            );
        } else {
            ensure!(
                v["aborted"] == false && v["lease_live"] == true,
                "pending evaluation lost scope"
            );
        }
    }
    Ok(())
}

/// Measured first Tutorial temporary: one real node takes its constructor guard.
pub(super) fn validate_native(v: &Value, temporary: &Value, component: &Value) -> Result<()> {
    validate(v, temporary)?;
    ensure!(
        v["stage"] == "returned"
            && v["arena_owner"] == 1
            && v["reservation_start"] == 0
            && v["cursor_after"] == 0
            && v["live_after"] == 0,
        "Tutorial nonempty evaluation did not release its actual shared scope"
    );
    ensure!(
        v["slots"]
            == serde_json::json!([{"slot":0,"weight_bits":0x3f800000,"branch":"nonempty","tail":1,"prepared":true,"masks_cleared":true,"nodes":[{"node":1,"result":0}],"successes":0,"flags_after":0}]),
        "Tutorial evaluation omitted actual tail/preparation/mask/node effects"
    );
    ensure!(
        v["prepares"]
            == serde_json::json!([{"file":0x010a0050,"reference":0x0d00016f,"generation":0,"descriptor_bound":true,"body_resident":true,"auxiliary_words":0,"lookups":1,"body_reads":1,"returned":"AuxiliaryZero","pause":null}]),
        "Tutorial preparation differs from retained primary skin and zero auxiliary branch"
    );
    ensure!(
        v["guards"]
            == serde_json::json!([{"node":1,"pointer":"constructor_inline","record":null,"clip_reference":u32::MAX,"animator_reference_read":false,"animator_reference":null,"result":0,"pending_boundary":null}]),
        "Tutorial node did not return through its actual first MAX sentinel guard"
    );
    for previous in [
        &component["bind_refresh"],
        &component["first_person"]["bind_refresh"],
        &component["initial_action"]["tp_capture"]["refresh"],
        &component["initial_action"]["fp_dispatch"]["mask_capture"]["refresh"],
    ] {
        ensure!(
            previous["scope_released"] == true
                && previous["live_reservations_after"] == 0
                && count(v, "ordinal")? > count(previous, "refresh_ordinal")?
                && count(v, "allocation_ordinal")? > count(previous, "allocation_ordinal")?,
            "Tutorial evaluation did not follow earlier shared arena refreshes"
        );
    }
    Ok(())
}
