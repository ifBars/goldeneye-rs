use super::{closed, count, hash};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
fn selected(v: &Value) -> bool {
    v.is_null() || matches!(v.as_str(), Some("first_person" | "third_person"))
}
fn backing(v: &Value, kind: &str) -> Result<usize> {
    closed(
        v,
        &[
            "kind",
            "native_bytes",
            "known_bytes",
            "matrix_payload_known_bytes",
        ],
    )?;
    let bytes = count(v, "native_bytes")?;
    let known = count(v, "known_bytes")?;
    ensure!(
        v["kind"] == kind && bytes <= 131072 && known <= bytes,
        "transform-cache backing kind, size or known-byte count differs"
    );
    if kind == "matrix" {
        ensure!(
            bytes >= 16
                && (bytes - 16) % 64 == 0
                && known >= 8
                && count(v, "matrix_payload_known_bytes")? <= bytes - 16
                && known == 8 + count(v, "matrix_payload_known_bytes")?,
            "matrix prefix was confused with unwritten transform payload"
        );
    } else {
        ensure!(
            v["matrix_payload_known_bytes"].is_null(),
            "nonmatrix backing claims matrix payload knowledge"
        );
    }
    Ok(usize::try_from(bytes)?)
}
fn build(v: &Value, slot: &str) -> Result<usize> {
    closed(
        v,
        &[
            "slot",
            "stage",
            "bone_count",
            "roots_completed",
            "descendant_visits",
            "descendant_bytes",
            "offsets_written",
            "allocations",
            "releases",
            "waiting_skeleton",
            "failed",
            "returned",
            "state_sha256",
        ],
    )?;
    let stage = [
        "cleanup_gate",
        "skeleton",
        "matrix_allocation",
        "validity_allocation",
        "validity_clear",
        "offsets_allocation",
        "temporary_allocation",
        "descendants",
        "compact_allocation",
        "compact_copy",
        "temporary_release",
        "returned",
    ]
    .iter()
    .position(|s| v["stage"] == *s)
    .context("unsupported cache header build stage")?;
    ensure!(
        v["slot"] == slot
            && ["waiting_skeleton", "failed", "returned"]
                .iter()
                .all(|k| v[k].is_boolean())
            && hash(&v["state_sha256"])
            && count(v, "allocations")? <= 5
            && count(v, "releases")? <= 5
            && count(v, "descendant_visits")? <= 16384
            && count(v, "descendant_bytes")? <= 16384
            && v["returned"] == (stage == 11)
            && (v["returned"] != true || v["failed"] == false),
        "cache header trace counters or return differ"
    );
    if stage < 2 {
        ensure!(
            v["bone_count"].is_null(),
            "cache skeleton count was read before its actual provider"
        );
    } else {
        ensure!(
            count(v, "bone_count")? <= 128
                && count(v, "roots_completed")? <= count(v, "bone_count")?
                && count(v, "offsets_written")? <= count(v, "bone_count")?,
            "cache skeleton metadata exceeds actual bounds"
        );
    }
    if v["waiting_skeleton"] == true {
        ensure!(
            stage == 1
                && v["bone_count"].is_null()
                && v["allocations"] == 0
                && v["failed"] == false,
            "unknown skeleton became an empty or failed header"
        );
    }
    if v["returned"] == true {
        let bones = count(v, "bone_count")?;
        if bones == 0 {
            ensure!(
                v["roots_completed"] == 0
                    && v["offsets_written"] == 0
                    && v["descendant_visits"] == 0
                    && v["descendant_bytes"] == 0
                    && v["allocations"] == 0,
                "zero-bone header invented transform storage"
            );
        } else {
            ensure!(
                count(v, "roots_completed")? == bones
                    && count(v, "offsets_written")? == bones
                    && count(v, "descendant_bytes")? == count(v, "descendant_visits")? + bones
                    && v["allocations"] == 5
                    && count(v, "releases")? >= 1,
                "returned cache header skipped topology/copy/temporary release"
            );
        }
    }
    Ok(stage)
}
pub(super) fn validate(v: &Value, p: &Value) -> Result<()> {
    closed(
        v,
        &[
            "selected",
            "counts",
            "matrix_backings",
            "associations",
            "native_bytes",
            "allocations",
            "invocation_count",
            "headers",
            "trace",
            "state_sha256",
        ],
    )?;
    let counts = v["counts"]
        .as_array()
        .filter(|a| a.len() == 2)
        .context("cache counts absent")?;
    let matrices = v["matrix_backings"]
        .as_array()
        .filter(|a| a.len() == 2 && a.iter().all(Value::is_boolean))
        .context("cache matrix presence absent")?;
    let associations = v["associations"]
        .as_array()
        .filter(|a| a.len() == 2 && a.iter().all(Value::is_boolean))
        .context("cache associations absent")?;
    let headers = v["headers"]
        .as_array()
        .filter(|a| a.len() == 2)
        .context("cache header owners absent")?;
    ensure!(
        selected(&v["selected"])
            && hash(&v["state_sha256"])
            && count(v, "native_bytes")? <= 131072
            && count(v, "allocations")? <= 10
            && count(v, "invocation_count")? > 0,
        "cache owner selection or budget differs"
    );
    if let Some(s) = v["selected"].as_str() {
        ensure!(
            matrices[usize::from(s == "third_person")] == true,
            "selected cache has no actual matrix backing"
        );
    }
    let mut bytes = 0;
    let mut allocations = 0;
    for (i, h) in headers.iter().enumerate() {
        closed(
            h,
            &[
                "count",
                "association_bound",
                "backings",
                "invalidation_count_reads",
                "invalidation_pointer_reads",
            ],
        )?;
        let fields = h["backings"]
            .as_array()
            .filter(|a| a.len() == 5)
            .context("cache backing positions absent")?;
        ensure!(
            count(h, "count")? <= 128
                && h["count"] == counts[i]
                && h["association_bound"] == associations[i]
                && matrices[i] == !fields[0].is_null()
                && count(h, "invalidation_count_reads")? == count(h, "invalidation_pointer_reads")?,
            "cache snapshot differs from actual header ownership"
        );
        for (j, kind) in ["matrix", "validity", "offsets", "descendants", "temporary"]
            .into_iter()
            .enumerate()
        {
            if !fields[j].is_null() {
                bytes += backing(&fields[j], kind)?;
                allocations += 1;
            }
        }
        if !fields[1].is_null() {
            ensure!(
                count(&fields[1], "native_bytes")? == count(h, "count")?,
                "cache validity backing differs from its actual count"
            );
        }
        if !fields[2].is_null() {
            ensure!(
                count(&fields[2], "native_bytes")? == 2 * count(h, "count")?,
                "cache offset backing width differs"
            );
        }
    }
    ensure!(
        bytes as u64 == count(v, "native_bytes")? && allocations == count(v, "allocations")?,
        "cache native/live allocation totals differ from real backings"
    );
    let t = &v["trace"];
    ensure!(
        !t.is_null(),
        "initializer cache snapshot has no reached call trace"
    );
    closed(
        t,
        &[
            "ordinal",
            "stage",
            "receivers_found",
            "selected_before",
            "builds",
            "missing_receiver_skip",
            "reused_selected",
            "selection_returned",
            "selected_after",
            "failed",
            "returned",
            "state_sha256",
        ],
    )?;
    let stage = [
        "entry",
        "first_person",
        "third_person",
        "selecting",
        "returned",
    ]
    .iter()
    .position(|s| t["stage"] == *s)
    .context("unsupported transform-cache call stage")?;
    let builds = t["builds"]
        .as_array()
        .filter(|a| a.len() == 2)
        .context("cache ordered build receipts absent")?;
    ensure!(
        count(t, "ordinal")? > 0
            && t["ordinal"] == v["invocation_count"]
            && t["receivers_found"] == p["receivers_found"]
            && selected(&t["selected_before"])
            && selected(&t["selected_after"])
            && [
                "missing_receiver_skip",
                "reused_selected",
                "selection_returned",
                "failed",
                "returned"
            ]
            .iter()
            .all(|k| t[k].is_boolean())
            && hash(&t["state_sha256"])
            && t["returned"] == (stage == 4)
            && (t["returned"] != true || t["failed"] == false),
        "cache source invocation, receiver gates or return differ"
    );
    for (i, b) in builds.iter().enumerate() {
        if !b.is_null() {
            let bstage = build(b, ["first_person", "third_person"][i])?;
            ensure!(stage > i, "cache TP build ran before FP");
            let fields = headers[i]["backings"]
                .as_array()
                .context("header backings missing")?;
            if !fields[0].is_null() {
                ensure!(
                    fields[0]["known_bytes"] == 8 && fields[0]["matrix_payload_known_bytes"] == 0,
                    "new cache allocation invented evaluated matrices"
                );
                if bstage >= 3 {
                    ensure!(
                        count(&fields[0], "native_bytes")? == 16 + 64 * count(b, "bone_count")?,
                        "cache matrix allocation differs from its actual build skeleton count"
                    );
                }
            }
            if bstage >= 5 && !fields[1].is_null() {
                ensure!(
                    fields[1]["known_bytes"] == fields[1]["native_bytes"],
                    "cache validity clear was skipped"
                );
            }
            if !fields[2].is_null() {
                ensure!(
                    count(&fields[2], "known_bytes")? == 2 * count(b, "offsets_written")?,
                    "cache known offsets differ from actual reached writes"
                );
            }
            if b["returned"] == true && count(b, "bone_count")? > 0 {
                ensure!(
                    !fields[0].is_null()
                        && !fields[1].is_null()
                        && !fields[2].is_null()
                        && !fields[3].is_null()
                        && fields[4].is_null()
                        && headers[i]["count"] == b["bone_count"]
                        && fields[3]["known_bytes"] == fields[3]["native_bytes"]
                        && fields[3]["native_bytes"] == b["descendant_bytes"],
                    "returned cache build lost actual compact storage or temporary release"
                );
            }
        }
    }
    if t["returned"] == true {
        ensure!(
            t["selected_after"] == v["selected"],
            "cache source selection return differs from current owner"
        );
        if t["missing_receiver_skip"] == true {
            ensure!(
                p["receivers_found"] != serde_json::json!([true, true])
                    && builds.iter().all(Value::is_null)
                    && t["reused_selected"] == false
                    && t["selection_returned"] == false
                    && t["selected_after"] == t["selected_before"],
                "cache missing receiver branch invented builds or selection"
            );
        } else if t["reused_selected"] == true {
            ensure!(
                !t["selected_before"].is_null()
                    && t["selected_after"] == t["selected_before"]
                    && builds.iter().all(Value::is_null)
                    && t["selection_returned"] == false,
                "cache reuse rebuilt headers or changed selection"
            );
        } else {
            ensure!(
                t["selected_before"].is_null()
                    && t["selection_returned"] == true
                    && builds.iter().all(|b| b["returned"] == true)
                    && t["selected_after"]
                        == if matrices[0] == true {
                            Value::from("first_person")
                        } else {
                            Value::Null
                        },
                "cache call skipped real ordered builds or FP selection"
            );
        }
    } else {
        ensure!(
            t["missing_receiver_skip"] == false
                && t["reused_selected"] == false
                && t["selection_returned"] == false
                && t["selected_after"].is_null(),
            "pending cache call claims a return-only effect"
        );
    }
    Ok(())
}
