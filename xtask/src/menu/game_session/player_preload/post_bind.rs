//! Caller suffix keeps unknown process data and its second scope distinct.
use super::{closed, count, evaluation, hash, word};
use anyhow::{Result, ensure};
use serde_json::Value;

pub(super) fn dependency(p: &Value) -> Option<&'static str> {
    match p["stage"].as_str()? {
        "awaiting_head" => Some("preload_post_bind_head"),
        "awaiting_parameter" => Some("preload_post_bind_parameter_pointer"),
        "awaiting_bone_outputs" => Some("preload_temporary_bone_outputs"),
        "evaluating" => match p["evaluation"]["pending_boundary"].as_str() {
            Some("Prepare")
                if p["evaluation"]["prepares"]
                    .as_array()
                    .and_then(|a| a.last())
                    .is_some_and(|p| p["pause"] == "Auxiliary") =>
            {
                Some("preload_post_bind_evaluation_auxiliary_clear")
            }
            Some("Prepare") => Some("preload_post_bind_evaluation_skin"),
            Some("Node") => Some("preload_post_bind_evaluation_sampling"),
            Some(_) => Some("preload_post_bind_evaluation_layers"),
            None => None,
        },
        _ => None,
    }
}
pub(super) fn validate(p: &Value, t: &Value) -> Result<()> {
    if p.is_null() {
        return Ok(());
    }
    closed(
        p,
        &[
            "stage",
            "pending_dependency",
            "temporary_identity",
            "head_reads",
            "first_scope",
            "head",
            "parameter",
            "process",
            "capture",
            "phase",
            "evaluation",
            "state_sha256",
        ],
    )?;
    ensure!(
        t["bind_return"] == 1
            && p["temporary_identity"] == t["identity"]
            && hash(&p["state_sha256"]),
        "caller did not retain its actual returned temporary"
    );
    ensure!(
        matches!(
            p["stage"].as_str(),
            Some(
                "awaiting_head"
                    | "awaiting_parameter"
                    | "reading_scalar"
                    | "assigning_phase"
                    | "evaluating"
                    | "awaiting_bone_outputs"
                    | "failed"
                    | "aborted"
            )
        ),
        "unknown caller stage"
    );
    ensure!(
        dependency(p).map_or_else(
            || p["pending_dependency"].is_null(),
            |d| p["pending_dependency"] == d
        ),
        "caller dependency differs"
    );
    let process = &p["process"];
    closed(
        process,
        &[
            "owner",
            "initial_bits",
            "current_bits",
            "revision",
            "reads",
            "capture_issued",
        ],
    )?;
    ensure!(
        count(process, "owner")? > 0
            && process["initial_bits"] == 0x3f800000u32
            && process["current_bits"].is_null(),
        "KnownInitial substituted for unknown current process scalar"
    );
    count(process, "revision")?;
    count(process, "reads")?;
    if !p["head"].is_null() {
        closed(&p["head"], &["kind", "node"])?;
        count(p, "head_reads")?;
        ensure!(
            matches!(
                p["head"]["kind"].as_str(),
                Some("class_four" | "nonmatching" | "null" | "unknown")
            ),
            "head type unknown"
        );
        match p["head"]["kind"].as_str() {
            Some("unknown") => ensure!(
                p["head"]["node"].is_null()
                    && p["stage"] == "awaiting_head"
                    && p["parameter"].is_null()
                    && p["capture"].is_null()
                    && p["phase"].is_null()
                    && p["evaluation"].is_null(),
                "unknown head was treated as a completed mismatch"
            ),
            Some("null") => ensure!(
                p["head"]["node"].is_null()
                    && t["head_inserted"] == false
                    && p["parameter"].is_null()
                    && p["capture"].is_null()
                    && p["phase"].is_null(),
                "NULL head borrowed a nonempty list or caller stores"
            ),
            Some("class_four" | "nonmatching") => ensure!(
                count(&p["head"], "node")? > 0,
                "typed head lacks an actual node"
            ),
            _ => {}
        }
    }
    if !p["parameter"].is_null() {
        let v = &p["parameter"];
        closed(v, &["node", "pointer", "offset", "previous", "bits"])?;
        ensure!(
            p["head"]["kind"] == "class_four"
                && v["node"] == p["head"]["node"]
                && v["pointer"] == "constructor_inline_phase"
                && v["offset"] == 0x64
                && v["bits"] == 0x3f000000u32
                && (v["previous"].is_null() || word(&v["previous"])),
            "caller parameter store lacks actual pointee"
        );
    }
    if !p["capture"].is_null() {
        let r = &p["capture"];
        closed(r, &["owner", "read_sequence", "revision", "current_bits"])?;
        ensure!(
            !p["parameter"].is_null()
                && r["owner"] == process["owner"]
                && count(r, "read_sequence")? > 0
                && count(r, "read_sequence")? <= count(process, "reads")?
                && count(r, "revision")? <= count(process, "revision")?
                && r["current_bits"].is_null()
                && process["capture_issued"] == true,
            "scalar capture lacks persistent owner/read provenance"
        );
    } else {
        ensure!(
            process["capture_issued"].is_null(),
            "unread scalar has capture provenance"
        );
    }
    if !p["phase"].is_null() {
        let phase = &p["phase"];
        closed(
            phase,
            &[
                "upper_bits",
                "selected_bits",
                "assignment",
                "header_known_bits",
                "header_expression_bound",
            ],
        )?;
        ensure!(
            !p["capture"].is_null()
                && phase["upper_bits"] == 0
                && phase["selected_bits"].is_null()
                && phase["assignment"] == "deferred"
                && phase["header_known_bits"].is_null()
                && phase["header_expression_bound"] == true,
            "unknown ordered clamp became a stale concrete phase"
        );
    }
    if !p["evaluation"].is_null() {
        ensure!(
            !p["head"].is_null() && (p["head"]["kind"] != "class_four" || !p["phase"].is_null()),
            "second evaluator ran before caller stores"
        );
        evaluation::validate_caller(&p["evaluation"], t)?;
        let first = &p["first_scope"];
        closed(first, &["arena_owner", "ordinal", "allocation_ordinal"])?;
        ensure!(
            count(first, "arena_owner")? > 0
                && count(first, "ordinal")? > 0
                && count(first, "allocation_ordinal")? > 0
                && first["arena_owner"] == p["evaluation"]["arena_owner"]
                && count(&p["evaluation"], "ordinal")? > count(first, "ordinal")?
                && count(&p["evaluation"], "allocation_ordinal")?
                    > count(first, "allocation_ordinal")?,
            "second evaluation borrowed first scope or foreign arena"
        );
        if !t["evaluation"].is_null() {
            ensure!(
                ["arena_owner", "ordinal", "allocation_ordinal"]
                    .iter()
                    .all(|k| first[*k] == t["evaluation"][*k])
                    && t["evaluation"]["stage"] == "returned"
                    && t["evaluation"]["released"] == true,
                "caller changed historical first bind scope"
            );
        } else {
            ensure!(
                t["refresh_returned"] == true && t["head_inserted"] == false,
                "missing first bind scope"
            );
        }
    }
    if p["stage"] == "awaiting_bone_outputs" {
        ensure!(
            p["evaluation"]["stage"] == "returned" && p["evaluation"]["released"] == true,
            "bone dependency skipped second evaluation return"
        );
    }
    Ok(())
}

/// The measured first caller finishes its second no-sample evaluation only.
pub(super) fn validate_native(p: &Value, t: &Value, c: &Value) -> Result<()> {
    validate(p, t)?;
    ensure!(
        p["stage"] == "awaiting_bone_outputs"
            && p["head_reads"] == 1
            && p["head"] == serde_json::json!({"kind":"class_four","node":1})
            && p["parameter"]
                == serde_json::json!({"node":1,"pointer":"constructor_inline_phase","offset":0x64,"previous":0,"bits":0x3f000000}),
        "Tutorial caller did not patch its actual returned class-four head"
    );
    ensure!(
        p["process"]["owner"] == 1
            && p["process"]["revision"] == 0
            && p["capture"]["owner"] == 1
            && p["capture"]["revision"] == 0
            && p["phase"]
                == serde_json::json!({"upper_bits":0,"selected_bits":null,"assignment":"deferred","header_known_bits":null,"header_expression_bound":true}),
        "Tutorial caller replaced UnknownCurrent phase or process provenance"
    );
    evaluation::validate_native(&p["evaluation"], t, c)?;
    Ok(())
}
