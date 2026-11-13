use super::*;
pub(super) fn validate(v: &Value, phase: &Value) -> Result<()> {
    closed(
        v,
        &[
            "selected_index",
            "file_bound",
            "generation_observed",
            "metadata_bound",
            "cache_generation",
            "descriptor_index",
            "body_bound",
            "timing_flags",
            "animation_fps",
            "script_fps_bits",
            "frame_bits",
            "previous_position_bits",
            "normalized_position_bits",
            "root_motion_pending",
        ],
    )?;
    let word = |k: &str| v[k].as_u64().is_some_and(|n| n <= u64::from(u32::MAX));
    let null = |k: &str| v[k].is_null();
    let absent = |keys: &[&str]| keys.iter().all(|k| null(k));
    ensure!(
        v["selected_index"] == phase["initial_index_raw"]
            && v["file_bound"].is_boolean()
            && v["metadata_bound"].is_boolean()
            && v["root_motion_pending"].is_boolean()
            && ["generation_observed", "cache_generation"]
                .iter()
                .all(|k| null(k) || word(k))
            && if v["metadata_bound"] == true {
                v["descriptor_index"].as_u64().is_some_and(|n| n < 32767)
            } else {
                null("descriptor_index")
            },
        "position owner or metadata evidence differs"
    );
    if v["generation_observed"].is_u64() && v["metadata_bound"] == true {
        ensure!(
            v["cache_generation"] == v["generation_observed"],
            "position cache generation differs"
        );
    }
    let later = [
        "timing_flags",
        "animation_fps",
        "script_fps_bits",
        "frame_bits",
        "previous_position_bits",
        "normalized_position_bits",
    ];
    match phase["stage"].as_str() {
        Some("animation_owner_pending") => {
            ensure!(
                v["file_bound"] == true
                    && null("generation_observed")
                    && v["metadata_bound"] == false
                    && null("body_bound")
                    && absent(&later)
                    && v["root_motion_pending"] == false,
                "animation owner dependency advanced"
            );
            return Ok(());
        }
        Some("animation_cache_pending") => {
            ensure!(
                if v["file_bound"] == true {
                    word("generation_observed") && v["cache_generation"] == v["generation_observed"]
                } else {
                    null("generation_observed") && null("cache_generation")
                },
                "unknown animation cache owner/generation differs"
            );
            ensure!(
                v["metadata_bound"] == false
                    && null("body_bound")
                    && absent(&later)
                    && v["root_motion_pending"] == false,
                "unknown animation cache advanced"
            );
            return Ok(());
        }
        Some("animation_body_pending") => {
            ensure!(
                v["file_bound"] == true
                    && v["metadata_bound"] == true
                    && null("body_bound")
                    && absent(&later)
                    && v["root_motion_pending"] == false,
                "animation body dependency advanced"
            );
            return Ok(());
        }
        _ => {}
    }
    ensure!(
        v["file_bound"] == true
            && v["body_bound"].is_boolean()
            && (v["body_bound"] != true || v["metadata_bound"] == true)
            && word("timing_flags"),
        "position body or timing evidence differs"
    );
    let flags = v["timing_flags"].as_u64().unwrap();
    if phase["stage"] == "timing_curve_pending" {
        ensure!(
            flags & 3 != 0 && absent(&later[1..]) && v["root_motion_pending"] == false,
            "timing curve dependency advanced"
        );
        return Ok(());
    }
    ensure!(
        flags & 3 == 0
            && if v["body_bound"] == true {
                v["animation_fps"].as_u64().is_some_and(|n| n <= 255) && word("script_fps_bits")
            } else {
                null("animation_fps") && null("script_fps_bits")
            },
        "position FPS reads differ"
    );
    ensure!(
        [
            "frame_bits",
            "previous_position_bits",
            "normalized_position_bits"
        ]
        .iter()
        .all(|k| word(k))
            && f32::from_bits(v["normalized_position_bits"].as_u64().unwrap() as u32).is_finite(),
        "normalized position absent or nonfinite"
    );
    ensure!(
        if phase["stage"] == "root_motion_pending" {
            v["root_motion_pending"] == true
                && (v["normalized_position_bits"].as_u64().unwrap() & 0x7fff_ffff) != 0
        } else {
            phase["stage"] == "initialized" && v["root_motion_pending"] == false
        },
        "position return/root-motion dependency differs"
    );
    Ok(())
}
#[cfg(test)]
pub(super) fn fixture() -> Value {
    serde_json::json!({"selected_index":0,"file_bound":true,"generation_observed":0,"metadata_bound":true,"cache_generation":0,"descriptor_index":3,
        "body_bound":true,"timing_flags":0,"animation_fps":30,"script_fps_bits":30f32.to_bits(),"frame_bits":0,"previous_position_bits":0,"normalized_position_bits":0,"root_motion_pending":false})
}
