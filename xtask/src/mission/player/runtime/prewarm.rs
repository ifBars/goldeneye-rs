//! Closed source-reset prewarm evidence. Numeric fingerprints include unverified
//! host square roots and do not establish native precision or displayed timing.
use super::{closed, digest, number};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

const LOCALS: &str = "6604ee92f43287dc11c88c87ac99291fa81dfaec379f0a2c6cb7cf4af8e8dca3";

pub(super) fn validate(value: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "seconds_bits",
            "baseline",
            "scaled_step_bits",
            "children",
            "updated_controls",
            "sampled_controls",
            "sampled_scripts",
            "skipped_layers",
            "selected_clip",
            "frame",
            "fade_bits",
            "animation_translations",
            "skin_translations",
            "committed_bones",
            "scratch_flags_raw",
            "locals_sha256",
            "locals_encoding",
            "post_update_bindings_sha256",
            "unverified_sqrt_rotations",
            "root_bias_applied",
            "skin_mapping_applied",
            "skeleton_committed",
            "matrices_refreshed",
            "native_numeric_parity_verified",
            "external_step_executed",
            "first_displayed_pose_verified",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    ensure!(
        value["state"] == "source_reset_prewarm_locals_committed"
            && value["locals_encoding"] == "source_context_locals_be_v1",
        "prewarm state or local-pose encoding differs"
    );
    for (field, expected) in [
        ("seconds_bits", 0x3e4c_cccdu64),
        ("baseline", 60),
        ("scaled_step_bits", 0x4140_0000),
        ("updated_controls", 1),
        ("sampled_controls", 1),
        ("sampled_scripts", 1),
        ("skipped_layers", 1),
        ("selected_clip", 0),
        ("frame", 0),
        ("fade_bits", 0x3f80_0000),
        ("animation_translations", 6),
        ("skin_translations", 42),
        ("committed_bones", 48),
        ("scratch_flags_raw", 0x4000),
        ("unverified_sqrt_rotations", 47),
    ] {
        ensure!(
            number(value, field)? == expected,
            "source reset prewarm input/count differs: {field}"
        );
    }
    ensure!(
        number(value, "animation_translations")?.checked_add(number(value, "skin_translations")?)
            == Some(number(value, "committed_bones")?),
        "prewarm translation source partition differs"
    );
    for field in [
        "root_bias_applied",
        "skin_mapping_applied",
        "skeleton_committed",
    ] {
        ensure!(
            value[field] == true,
            "prewarm prerequisite not applied: {field}"
        );
    }
    for field in [
        "matrices_refreshed",
        "native_numeric_parity_verified",
        "external_step_executed",
        "first_displayed_pose_verified",
        "camera_frame_ready",
    ] {
        ensure!(
            value[field] == false,
            "prewarm promoted unavailable runtime work: {field}"
        );
    }
    digest(value, "locals_sha256")?;
    digest(value, "post_update_bindings_sha256")?;
    ensure!(
        value["locals_sha256"] == LOCALS,
        "prewarm committed locals differ from independent source/host context audit"
    );
    // Post-update owned-node digest is syntax checked until its independent
    // source serialization audit exists. Never treat it as native comparison.
    let children = value["children"]
        .as_array()
        .context("prewarm updated child list absent")?;
    ensure!(
        children.len() == 2,
        "prewarm must update both active script children"
    );
    // Independent private player-animation-clock-v21 prewarm proof: phase0, rate12*.5,
    // (first_frame_count / selected_span) / script_duration, then multiply.
    // Old normalized/previous positions remain0 before this accumulated phase.
    for (child, (reference, node, phase_bits)) in children.iter().zip([
        (0x8400_0022u64, 3, 0x3dc9_714f),
        (0x8400_00bb, 2, 0x3da1_af28),
    ]) {
        closed(
            child,
            &[
                "node",
                "reference",
                "normalized_position_bits",
                "previous_phase_bits",
                "advanced_phase_bits",
                "transition_bits",
                "reciprocal_bits",
            ],
        )?;
        ensure!(
            number(child, "node")? == node,
            "prewarm updated child owner differs from the constructor membership"
        );
        ensure!(
            number(child, "reference")? == reference,
            "prewarm child update order differs from slot-ascending TAIL-to-previous traversal"
        );
        ensure!(
            number(child, "normalized_position_bits")? == 0
                && number(child, "previous_phase_bits")? == 0
                && number(child, "transition_bits")? == 0x3f80_0000
                && number(child, "reciprocal_bits")? == 0,
            "prewarm child cursor/transition boundary differs"
        );
        let bits = u32::try_from(number(child, "advanced_phase_bits")?)?;
        let phase = f32::from_bits(bits);
        ensure!(
            phase.is_finite() && phase > 0.0 && phase < 1.0 && u64::from(bits) == phase_bits,
            "prewarm advanced phase differs from the independent source arithmetic"
        );
    }
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|text| !text.is_empty() && text.len() <= 2048),
        "prewarm scope missing"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "state":"source_reset_prewarm_locals_committed", "seconds_bits":0x3e4c_cccdu32,
            "baseline":60,"scaled_step_bits":0x4140_0000u32,
            "children":[
                {"node":3,"reference":0x8400_0022u32,"normalized_position_bits":0,"previous_phase_bits":0,
                    "advanced_phase_bits":0x3dc9_714fu32,"transition_bits":0x3f80_0000u32,"reciprocal_bits":0},
                {"node":2,"reference":0x8400_00bbu32,"normalized_position_bits":0,"previous_phase_bits":0,
                    "advanced_phase_bits":0x3da1_af28u32,"transition_bits":0x3f80_0000u32,"reciprocal_bits":0}
            ],
            "updated_controls":1,"sampled_controls":1,"sampled_scripts":1,"skipped_layers":1,
            "selected_clip":0,"frame":0,"fade_bits":0x3f80_0000u32,"animation_translations":6,
            "skin_translations":42,"committed_bones":48,"scratch_flags_raw":0x4000,
            "locals_sha256":LOCALS,"locals_encoding":"source_context_locals_be_v1",
            "post_update_bindings_sha256":"a".repeat(64),"unverified_sqrt_rotations":47,
            "root_bias_applied":true,"skin_mapping_applied":true,"skeleton_committed":true,
            "matrices_refreshed":false,"native_numeric_parity_verified":false,"external_step_executed":false,
            "first_displayed_pose_verified":false,"camera_frame_ready":false,"scope":"Authored closed boundary."
        })
    }

    #[test]
    fn closed_prewarm_keeps_source_counts_and_unverified_precision_separate() {
        let valid = fixture();
        validate(&valid).unwrap();
        for field in [
            "seconds_bits",
            "baseline",
            "scaled_step_bits",
            "updated_controls",
            "sampled_controls",
            "sampled_scripts",
            "skipped_layers",
            "selected_clip",
            "frame",
            "fade_bits",
            "animation_translations",
            "skin_translations",
            "committed_bones",
            "scratch_flags_raw",
            "unverified_sqrt_rotations",
        ] {
            let mut changed = valid.clone();
            changed[field] = json!(number(&valid, field).unwrap() + 1);
            assert!(
                validate(&changed).is_err(),
                "changed source scalar/count accepted: {field}"
            );
        }
        let mut changed = valid;
        changed["locals_sha256"] = json!("0".repeat(64));
        assert!(validate(&changed).is_err());
    }

    #[test]
    fn prewarm_rejects_promoted_flags_and_exported_arrays() {
        let valid = fixture();
        for field in [
            "root_bias_applied",
            "skin_mapping_applied",
            "skeleton_committed",
            "matrices_refreshed",
            "native_numeric_parity_verified",
            "external_step_executed",
            "first_displayed_pose_verified",
            "camera_frame_ready",
        ] {
            let mut changed = valid.clone();
            changed[field] = json!(!valid[field].as_bool().unwrap());
            assert!(
                validate(&changed).is_err(),
                "contradictory flag accepted: {field}"
            );
        }
        for key in [
            "locals_raw",
            "translation_bits",
            "quaternion_bits",
            "path",
            "original_data",
        ] {
            let mut changed = valid.clone();
            changed[key] = json!([1, 2, 3]);
            assert!(
                validate(&changed).is_err(),
                "private export accepted: {key}"
            );
        }
    }

    #[test]
    fn child_update_order_owned_identity_and_phase_boundary_are_checked() {
        let valid = fixture();
        for (field, value) in [
            ("node", json!(2)),
            ("reference", json!(0x8400_00bbu32)),
            ("normalized_position_bits", json!(1)),
            ("previous_phase_bits", json!(1)),
            ("transition_bits", json!(0)),
            ("reciprocal_bits", json!(1)),
            ("advanced_phase_bits", json!(0)),
            ("advanced_phase_bits", json!(0x3dc9_7150u32)),
            ("advanced_phase_bits", json!(f32::NAN.to_bits())),
        ] {
            let mut changed = valid.clone();
            changed["children"][0][field] = value;
            assert!(
                validate(&changed).is_err(),
                "changed child boundary accepted: {field}"
            );
        }
        let mut changed = valid;
        changed["children"][0]["original_curve"] = json!([1, 2]);
        assert!(validate(&changed).is_err());
    }
}
