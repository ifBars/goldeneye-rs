//! Independent source/host fingerprints for requested camera-bone transforms.
use super::super::{closed, number};
use anyhow::{Result, ensure};
use serde_json::Value;

const LOCALS: &str = "6604ee92f43287dc11c88c87ac99291fa81dfaec379f0a2c6cb7cf4af8e8dca3";
const UNCACHED: &str = "7aaeae80fbea9b32db9ed988be670a5acd13f8b0025140a19a3074dc6f2738bc";
const CACHED: &str = "4a1e21c98029fb9d601eb11487237c53bd206e7294acd674bfd7cfa270d98760";

pub(super) fn validate(value: &Value, player: &Value) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "boundary",
            "cache_selection",
            "tag",
            "bone",
            "chain_length",
            "local_transforms",
            "matrix_products",
            "cache_slots",
            "cache_hits",
            "cache_misses",
            "valid_cached_bones",
            "unverified_input_rotations",
            "host_matrix_arithmetic_unverified",
            "encoding",
            "transform_sha256",
            "committed_locals_sha256",
            "tag_bound",
            "committed_pose_used",
            "owner_world_applied",
            "matrices_refreshed",
            "native_numeric_parity_verified",
            "first_displayed_pose_verified",
            "camera_frame_ready",
            "scope",
        ],
    )?;
    ensure!(
        value["state"] == "local_camera_bone_fk_prepared"
            && value["boundary"] == "source_reset_prewarm_committed_locals",
        "camera-bone boundary changed"
    );
    let prewarm = &player["initial_layers"]["prewarm_pose"];
    ensure!(
        prewarm.is_object()
            && prewarm["skeleton_committed"] == true
            && prewarm["locals_sha256"] == LOCALS
            && player["animation_sampled"] == true,
        "camera-bone transform lacks the actual committed prewarm"
    );
    ensure!(
        value["committed_locals_sha256"] == LOCALS,
        "camera-bone input pose fingerprint changed"
    );
    for (key, expected) in [
        ("tag", 0x0e00_0028u64),
        ("bone", 47),
        ("chain_length", 3),
        ("local_transforms", 3),
        ("unverified_input_rotations", 2),
    ] {
        ensure!(
            number(value, key)? == expected,
            "camera-bone source input/count changed: {key}"
        );
    }
    ensure!(
        value["bone"] == player["camera_bone"]
            && value["tag"] == player["camera_tag"]
            && value["chain_length"] == player["camera_parent_chain_length"],
        "camera-bone transform differs from the retained tag binding"
    );
    let (encoding, digest, counts) = match value["cache_selection"].as_str() {
        Some("explicit_disabled_inspection") => {
            ("source_camera_bone_fk_be_v1", UNCACHED, [3, 0, 0, 0, 0])
        }
        Some("fresh_fp_actor_cache") => (
            "source_cached_camera_bone_fk_be_v1",
            CACHED,
            [2, 48, 1, 3, 3],
        ),
        _ => anyhow::bail!("unknown camera-bone cache boundary"),
    };
    for (key, expected) in [
        "matrix_products",
        "cache_slots",
        "cache_hits",
        "cache_misses",
        "valid_cached_bones",
    ]
    .into_iter()
    .zip(counts)
    {
        ensure!(
            number(value, key)? == expected,
            "camera-bone cache operation/count changed: {key}"
        );
    }
    ensure!(
        value["encoding"] == encoding && value["transform_sha256"] == digest,
        "camera-bone transform differs from independent source/host reconstruction"
    );
    for key in [
        "tag_bound",
        "committed_pose_used",
        "host_matrix_arithmetic_unverified",
    ] {
        ensure!(
            value[key] == true,
            "camera-bone prerequisite/provenance missing: {key}"
        );
    }
    for key in [
        "owner_world_applied",
        "matrices_refreshed",
        "native_numeric_parity_verified",
        "first_displayed_pose_verified",
        "camera_frame_ready",
    ] {
        ensure!(
            value[key] == false,
            "camera-bone transform promoted unavailable work: {key}"
        );
    }
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 2048),
        "camera-bone scope missing"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture(cached: bool) -> (Value, Value) {
        let value = json!({"state":"local_camera_bone_fk_prepared","boundary":"source_reset_prewarm_committed_locals",
            "cache_selection":if cached {"fresh_fp_actor_cache"} else {"explicit_disabled_inspection"},
            "tag":0x0e00_0028u32,"bone":47,"chain_length":3,"local_transforms":3,
            "matrix_products":if cached {2} else {3},"cache_slots":if cached {48} else {0},
            "cache_hits":usize::from(cached),"cache_misses":if cached {3} else {0},"valid_cached_bones":if cached {3} else {0},
            "unverified_input_rotations":2,"host_matrix_arithmetic_unverified":true,
            "encoding":if cached {"source_cached_camera_bone_fk_be_v1"} else {"source_camera_bone_fk_be_v1"},
            "transform_sha256":if cached {CACHED} else {UNCACHED},"committed_locals_sha256":LOCALS,
            "tag_bound":true,"committed_pose_used":true,"owner_world_applied":false,"matrices_refreshed":false,
            "native_numeric_parity_verified":false,"first_displayed_pose_verified":false,"camera_frame_ready":false,
            "scope":"Authored source/host boundary."});
        let player = json!({"camera_bone":47,"camera_tag":0x0e00_0028u32,"camera_parent_chain_length":3,"animation_sampled":true,
            "initial_layers":{"prewarm_pose":{"skeleton_committed":true,"locals_sha256":LOCALS}}});
        (value, player)
    }
    #[test]
    fn cache_paths_keep_distinct_operation_counts_and_fingerprints() {
        for cached in [false, true] {
            let (value, player) = fixture(cached);
            validate(&value, &player).unwrap();
            for key in [
                "matrix_products",
                "cache_slots",
                "cache_hits",
                "cache_misses",
                "valid_cached_bones",
                "unverified_input_rotations",
            ] {
                let mut bad = value.clone();
                bad[key] = json!(number(&value, key).unwrap() + 1);
                assert!(validate(&bad, &player).is_err(), "{key}");
            }
            let mut bad = value;
            bad["transform_sha256"] = json!(if cached { UNCACHED } else { CACHED });
            assert!(validate(&bad, &player).is_err());
        }
    }
    #[test]
    fn transform_requires_committed_input_and_cannot_promote_camera_or_export_matrices() {
        let (value, player) = fixture(true);
        for key in [
            "owner_world_applied",
            "matrices_refreshed",
            "native_numeric_parity_verified",
            "first_displayed_pose_verified",
            "camera_frame_ready",
        ] {
            let mut bad = value.clone();
            bad[key] = json!(true);
            assert!(validate(&bad, &player).is_err());
        }
        let mut bad = value.clone();
        bad["full_bits"] = json!(vec![0u32; 16]);
        assert!(validate(&bad, &player).is_err());
        let mut missing = player;
        missing["initial_layers"] = Value::Null;
        assert!(validate(&value, &missing).is_err());
    }
}
