//! Exact observed supplied Tutorial checkpoint, separate from generic schemas.
use super::{player_component_tail, player_initializer_prefix, player_pose_setup};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
pub(super) fn validate(player: &Value) -> Result<()> {
    let c = &player["component_initialization"];
    let pose = &c["pose_setup"];
    let outer = &c["component_tail"];
    let prefix = &c["initializer_prefix"];
    player_pose_setup::validate(pose, c)?;
    player_component_tail::validate(outer, c)?;
    player_initializer_prefix::validate(prefix, c)?;
    ensure!(
        player["pending_dependency"] == "player_animation_preload"
            && c["stage"] == "player_animation_preload_pending"
            && c["pending_dependency"] == "player_animation_preload"
            && pose["stage"] == "returned"
            && pose["input_raw"] == 1
            && pose["input_signed"] == 1
            && pose["entry_702"] == 0
            && pose["blend_ba8"] == 0
            && pose["bab_written"] == true
            && pose["tp_enabled"] == false
            && pose["fp_enabled"] == true
            && pose["receiver_searches"] == json!([true, true])
            && pose["character_bound"] == true
            && pose["character_scalars_read"] == 2
            && pose["character_vector_read"] == true
            && pose["session_scale_multiple"] == false
            && pose["scale_source"] == "startup_ones"
            && pose["scale_stored"] == true
            && pose["scale_invalidated"] == true
            && pose["dirty_before"] == 0x13
            && pose["dirty_after"] == 0x13
            && pose["ca4_written"] == true
            && pose["mode_afc_absent"] == true
            && pose["force_nodes_checked"] == 2
            && pose["user_target_bound"] == true
            && pose["user_lookup"]["visited"] == 1
            && pose["user_lookup"]["matched"] == false
            && pose["user_lookup"]["association_state"] == "known_null"
            && pose["user_lookup"]["association_origin"] == "local_user_constructor"
            && pose["false_arm_b00_absent"] == true
            && pose["backlink_nodes_checked"] == 2
            && pose["attachment_absent"] == true
            && pose["weapon_global_read"] == true
            && pose["weapon_global_byte"].is_null()
            && pose["global_argument"].is_null()
            && pose["global_equivalence_used"] == true
            && pose["global_attachment_absent"] == true
            && pose["fade_inputs_read"] == true
            && pose["fade_argument_ready"] == true
            && pose["fade_attachment_absent"] == true
            && pose["final_session_multiple"] == false
            && pose["helper_returned"] == true,
        "Tutorial nested pose helper did not return through its actual user/NULL branches and unknown-global equivalent no-op"
    );
    for (i, reference, before, after, enabled) in [
        (0, 0x8400_0063u32, 3, 2, false),
        (1, 0x8400_0022, 3, 3, true),
    ] {
        let e = &pose["enable_writes"][i];
        ensure!(
            e["reference"] == reference
                && e["reference_matched"] == true
                && e["enabled"] == enabled
                && e["flags_before"] == before
                && e["flags_after"] == after
                && e["invalidated_8e"] == enabled,
            "Tutorial live semantic enable flags/reference differ from retained factory output"
        );
    }
    ensure!(
        outer["stage"] == "returned"
            && outer["helper_return"] == 1
            && outer["helper_returned"] == true
            && outer["rotation_reads"] == json!([true, true])
            && outer["rotation_stores"] == json!([true, true])
            && outer["rotation_invalidated"] == true
            && outer["character_outcome"] == "bound"
            && outer["character_scalars_read"] == 2
            && outer["character_scale_ready"] == true
            && outer["session_multiple"] == false
            && outer["scale_stores"] == json!([true, true])
            && outer["scale_invalidated"] == true
            && outer["flags_before"] == 0x13
            && outer["flags_after"] == 0x13,
        "Tutorial outer component helper did not return after its actual rotation and character-scale stores"
    );
    ensure!(
        prefix["stage"] == "awaiting_preload"
            && prefix["pending_dependency"] == "player_animation_preload"
            && prefix["item_flags_before"] == 0
            && prefix["item_flags_after"] == 12
            && prefix["mode_written"] == true
            && prefix["dirty_cleared"] == true
            && prefix["scalars_written"] == true
            && prefix["receiver_searches"] == 2
            && prefix["receivers_found"] == json!([true, true])
            && prefix["cache_enabled"] == 1
            && prefix["invalidated"] == false,
        "Tutorial initializer prefix skipped stores or claimed animation preload"
    );
    let cache = &prefix["caches"];
    let trace = &cache["trace"];
    ensure!(
        cache["selected"] == "first_person"
            && cache["counts"] == json!([48, 37])
            && cache["matrix_backings"] == json!([true, true])
            && cache["associations"] == json!([false, false])
            && cache["native_bytes"] == 6377
            && cache["allocations"] == 8
            && trace["returned"] == true
            && trace["missing_receiver_skip"] == false
            && trace["selected_after"] == "first_person",
        "Tutorial persistent transform-cache ownership differs"
    );
    for (i, n, visits, desc) in [(0, 48u32, 333, 381u32), (1, 37, 232, 269)] {
        let h = &cache["headers"][i];
        let b = &h["backings"];
        ensure!(
            h["count"] == n
                && h["association_bound"] == false
                && h["invalidation_count_reads"] == 0
                && h["invalidation_pointer_reads"] == 0
                && b[0]["native_bytes"] == 16 + 64 * n
                && b[0]["known_bytes"] == 8
                && b[0]["matrix_payload_known_bytes"] == 0
                && b[1]["native_bytes"] == n
                && b[1]["known_bytes"] == n
                && b[2]["native_bytes"] == 2 * n
                && b[2]["known_bytes"] == 2 * n
                && b[3]["native_bytes"] == desc
                && b[3]["known_bytes"] == desc
                && b[4].is_null(),
            "Tutorial cache backings were guessed, initialized as matrices or not compacted/released"
        );
        if trace["reused_selected"] != true {
            let build = &trace["builds"][i];
            ensure!(
                build["bone_count"] == n
                    && build["roots_completed"] == n
                    && build["offsets_written"] == n
                    && build["descendant_visits"] == visits
                    && build["descendant_bytes"] == desc
                    && build["allocations"] == 5
                    && build["releases"] == 1
                    && build["returned"] == true,
                "Tutorial actual cache build topology differs"
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
