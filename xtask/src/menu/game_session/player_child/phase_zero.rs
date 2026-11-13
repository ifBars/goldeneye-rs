use super::*;
pub(super) fn validate(v: &Value) -> Result<()> {
    closed(
        v,
        &[
            "stage",
            "delta_bits",
            "duration_raw",
            "frame_bits",
            "entries_visited",
            "selected_offset",
            "normal_requested",
            "secondary_requested",
            "normal_bound",
            "secondary_bound",
            "clip_records",
            "heap_allocated",
            "initial_index_raw",
            "initial_index_clamped",
            "frame_range",
            "setup_returned",
            "flags_applied",
            "pending_file",
            "position",
        ],
    )?;
    let bounded = |key: &str, max: u64| v[key].as_u64().is_some_and(|n| n <= max);
    let null = |key: &str| v[key].is_null();
    ensure!(
        ["delta_bits", "duration_raw", "frame_bits"]
            .iter()
            .all(|k| bounded(k, u64::from(u32::MAX)))
            && ["delta_bits", "frame_bits"]
                .iter()
                .all(|k| f32::from_bits(v[k].as_u64().unwrap() as u32).is_finite())
            && bounded("entries_visited", 65_536)
            && v["entries_visited"].as_u64().unwrap() > 0
            && v["setup_returned"].is_boolean()
            && v["flags_applied"].is_boolean(),
        "phase-zero timing or scan evidence differs"
    );
    if v["stage"] == "returned_without_play" {
        ensure!(
            [
                "selected_offset",
                "normal_requested",
                "secondary_requested",
                "initial_index_raw",
                "initial_index_clamped",
                "frame_range",
                "pending_file",
                "position"
            ]
            .iter()
            .all(|k| null(k))
                && v["normal_bound"] == 0
                && v["secondary_bound"] == 0
                && v["clip_records"] == 0
                && v["heap_allocated"] == false
                && v["setup_returned"] == false
                && v["flags_applied"] == false,
            "no-play phase claims clip effects"
        );
        return Ok(());
    }
    ensure!(
        bounded("selected_offset", u64::from(u32::MAX))
            && bounded("normal_requested", 255)
            && bounded("secondary_requested", 255)
            && bounded("normal_bound", v["normal_requested"].as_u64().unwrap())
            && bounded(
                "secondary_bound",
                v["secondary_requested"].as_u64().unwrap()
            ),
        "phase-zero clip counts differ"
    );
    let total =
        v["normal_requested"].as_u64().unwrap() + v["secondary_requested"].as_u64().unwrap();
    ensure!(
        v["heap_allocated"] == (total > 1) && v["clip_records"] == total.max(1),
        "phase-zero physical allocation differs"
    );
    if v["stage"] == "clip_file_pending" {
        ensure!(
            bounded("pending_file", u64::from(u32::MAX) - 1)
                && ["initial_index_raw", "initial_index_clamped", "frame_range"]
                    .iter()
                    .all(|k| null(k))
                && v["setup_returned"] == false
                && v["flags_applied"] == false
                && v["position"].is_null(),
            "clip-file pending phase advances past lookup"
        );
        return Ok(());
    }
    ensure!(
        null("pending_file")
            && bounded("initial_index_raw", 255)
            && bounded("initial_index_clamped", 255)
            && v["initial_index_clamped"].as_u64()
                == Some(
                    v["initial_index_raw"]
                        .as_u64()
                        .unwrap()
                        .min(v["normal_bound"].as_u64().unwrap())
                ),
        "phase-zero index clamp differs"
    );
    match v["stage"].as_str() {
        Some("clip_range_pending") => ensure!(
            null("frame_range")
                && v["setup_returned"] == false
                && v["flags_applied"] == false
                && v["position"].is_null(),
            "unwritten clip range advanced"
        ),
        Some("frame_count_pending") => ensure!(
            v["frame_range"]
                .as_i64()
                .is_some_and(|n| (-65535..=0).contains(&n))
                && v["setup_returned"] == false
                && v["flags_applied"] == false
                && v["position"].is_null(),
            "frame-count dependency advanced"
        ),
        Some(
            "animation_pending"
            | "animation_owner_pending"
            | "animation_cache_pending"
            | "animation_body_pending"
            | "timing_curve_pending"
            | "root_motion_pending"
            | "initialized",
        ) => {
            ensure!(
                v["frame_range"]
                    .as_i64()
                    .is_some_and(|n| (1..=65535).contains(&n))
                    && v["setup_returned"] == true
                    && v["flags_applied"] == true,
                "animation dependency lacks clip setup"
            );
            if v["stage"] == "animation_pending" {
                ensure!(
                    v["position"].is_null(),
                    "position continuation precedes execution"
                );
            } else {
                super::position::validate(&v["position"], v)?;
            }
        }
        _ => anyhow::bail!("unknown phase-zero dependency"),
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn fixture() -> Value {
    serde_json::json!({"stage":"frame_count_pending","delta_bits":1f32.to_bits(),"duration_raw":60,"frame_bits":0,
        "entries_visited":2,"selected_offset":0,"normal_requested":1,"secondary_requested":0,"normal_bound":1,"secondary_bound":0,
        "clip_records":1,"heap_allocated":false,"initial_index_raw":0,"initial_index_clamped":0,"frame_range":0,
        "setup_returned":false,"flags_applied":false,"pending_file":null,"position":null})
}
