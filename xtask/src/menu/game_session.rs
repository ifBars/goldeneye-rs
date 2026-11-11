//! Native shared-handler New Game probes. Retained CPU mission resources are a
//! development checkpoint; none of these counts promote a live player/camera.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
mod player_refresh;
mod player_skeleton;

const CONTROLLER_FIELDS: &[&str] = &[
    "requested",
    "loaded",
    "cancelled",
    "failed",
    "retired_workers",
    "retired_worker_failures",
    "active",
    "provider",
    "native_provider_timing_verified",
    "scope",
];
const SNAPSHOT_FIELDS: &[&str] = &[
    "stage",
    "difficulty",
    "level_hash",
    "start_point_id",
    "session_active",
    "local_users",
    "resources_retained",
    "map_instances",
    "source_start_points",
    "world_registered",
    "player_created",
    "camera_ready",
    "mode",
    "backend",
    "manager_ready_flags_raw",
    "user_ready_flags_raw",
    "source_create_counter",
    "created_session_selected",
    "failure",
    "world_initialization",
];
const WORLD_COUNTS: &[&str] = &[
    "maps_activated",
    "original_slots",
    "disabled_slots",
    "filtered_slots",
    "unclassified_slots",
    "source_null_slots",
    "unsupported_factory_slots",
    "unsupported_shape_slots",
    "owned_triggers",
    "common_scratch",
    "membership_entries",
    "active_triggers",
    "start_points",
    "generic_triggers",
    "common_be_triggers",
    "resource08_triggers",
    "active91_triggers",
    "range0c_triggers",
    "radius06_triggers",
    "actor12_triggers",
    "range82_triggers",
    "a3_triggers",
    "range27_triggers",
    "common09_triggers",
    "ad_triggers",
    "radius03_triggers",
    "common8a_triggers",
    "radius1b_triggers",
    "radius74_triggers",
    "shape71_triggers",
    "materialized_shapes",
    "radius88_triggers",
    "auxac_triggers",
    "map_initialization_blocked",
    "staged_triggers",
    "stopped_animators",
    "resolved_references",
    "blocked_references",
    "animators_initialized",
    "animators_failed",
    "shared_variables",
    "unwritten_scratch_words",
    "state_flags",
    "execution_slots",
    "active_executions",
    "create_events_denied",
    "registration_events_unmapped",
    "zone_queries",
    "catalog_entries",
    "shape_selectors_initialized",
];

const ACTOR_COUNTS: &[&str] = &[
    "resource_hash",
    "spreadsheet_hash",
    "payload_bytes",
    "primary_rows",
    "secondary_rows",
    "registered_primary",
    "duplicate_primary",
    "runtime_groups",
    "grouped_members",
    "group8_members",
    "selection_calls",
    "rng_draws",
];

fn validate_actor_definitions(actor: &Value) -> Result<()> {
    if actor.is_null() {
        return Ok(());
    }
    let fields: Vec<_> = ACTOR_COUNTS
        .iter()
        .copied()
        .chain([
            "resource_shared",
            "primary_present",
            "secondary_present",
            "fresh_cursors_zero",
            "default_record_bound",
            "special_records_bound",
            "original_cursor_history_verified",
            "rng_bound",
            "native9_constructor_ready",
            "rng_algorithm",
            "rng_origin",
            "rng_state_sha256",
            "original_rng_history_verified",
            "state_sha256",
            "scope",
        ])
        .collect();
    closed(actor, &fields)?;
    for field in ACTOR_COUNTS {
        count(actor, field)?;
    }
    ensure!(
        actor["resource_hash"] == 0x0109_0013
            && actor["spreadsheet_hash"] == 0x1400_0021
            && actor["resource_shared"] == true
            && [
                "default_record_bound",
                "special_records_bound",
                "original_cursor_history_verified",
                "original_rng_history_verified"
            ]
            .into_iter()
            .all(|field| actor[field] == false)
            && actor["rng_algorithm"] == "source_two_step_lcg32"
            && actor["rng_origin"] == "fresh_constructor_snapshot",
        "actor definitions identity or source-snapshot scope differs"
    );
    let primary = count(actor, "primary_rows")?;
    let registered = count(actor, "registered_primary")?;
    let members = count(actor, "grouped_members")?;
    let calls = count(actor, "selection_calls")?;
    let rng_bound = actor["rng_bound"]
        .as_bool()
        .context("actor RNG binding flag absent")?;
    let ready = actor["native9_constructor_ready"]
        .as_bool()
        .context("actor constructor scope flag absent")?;
    let fresh = actor["fresh_cursors_zero"]
        .as_bool()
        .context("actor fresh cursor flag absent")?;
    ensure!(
        count(actor, "rng_draws")? <= calls
            && (rng_bound || actor["rng_draws"] == 0)
            && (!fresh || calls == 0)
            && (!ready || (rng_bound && calls > 0 && !fresh)),
        "actor RNG/cursor counters or constructor scope disagree"
    );
    ensure!(
        primary == registered + count(actor, "duplicate_primary")?
            && count(actor, "runtime_groups")? <= members
            && members <= registered
            && count(actor, "group8_members")? <= members
            && actor["primary_present"]
                .as_bool()
                .is_some_and(|present| present || primary == 0)
            && actor["secondary_present"]
                .as_bool()
                .is_some_and(|present| present || actor["secondary_rows"] == 0)
            && ["state_sha256", "rng_state_sha256"]
                .into_iter()
                .all(
                    |field| actor[field].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                )
            && actor["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "actor definition rows, groups, provenance or digest invalid"
    );
    Ok(())
}

fn validate_resource_states(states: &Value, radius03: u64) -> Result<()> {
    if states.is_null() {
        return Ok(());
    }
    closed(
        states,
        &[
            "origin",
            "lifetime",
            "shared_owner",
            "records",
            "capacity",
            "nonzero_values",
            "reset_limit",
            "lookups",
            "hits",
            "misses",
            "snapshot_bound",
            "original_history_verified",
            "state_sha256",
            "scope",
        ],
    )?;
    for field in [
        "records",
        "capacity",
        "nonzero_values",
        "reset_limit",
        "lookups",
        "hits",
        "misses",
    ] {
        count(states, field)?;
    }
    ensure!(
        matches!(
            states["origin"].as_str(),
            Some("fresh_constructor_snapshot" | "supplied_snapshot")
        ) && states["lifetime"] == "native_window_controller"
            && states["shared_owner"] == true
            && states["snapshot_bound"].is_boolean()
            && states["original_history_verified"] == false
            && count(states, "reset_limit")? <= u64::from(u32::MAX)
            && count(states, "records")? <= count(states, "capacity")?
            && count(states, "nonzero_values")? <= count(states, "records")?
            && count(states, "hits")? + count(states, "misses")? == count(states, "lookups")?
            && count(states, "lookups")? <= radius03
            && (states["records"] != 0 || states["hits"] == 0)
            && states["state_sha256"]
                .as_str()
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            && states["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "resource-state ownership, lookup accounting or snapshot provenance invalid"
    );
    Ok(())
}

const NAVIGATION_COUNTS: &[&str] = &[
    "source_placements",
    "eligible_placements",
    "owned_wrappers",
    "bound_wrappers",
    "initialized_caches",
    "vertices",
    "faces",
    "category_slots",
    "nonempty_categories",
    "category_members",
    "boundary_pairs",
];

fn validate_navigation(navigation: &Value) -> Result<()> {
    if navigation.is_null() {
        return Ok(());
    }
    let fields: Vec<_> = NAVIGATION_COUNTS
        .iter()
        .copied()
        .chain([
            "origin",
            "initialized",
            "bindings_sha256",
            "categories_sha256",
            "cache_sha256",
            "state_sha256",
            "original_history_verified",
            "native_precision_verified",
            "queries_executed",
            "scope",
        ])
        .collect();
    closed(navigation, &fields)?;
    for field in NAVIGATION_COUNTS {
        count(navigation, field)?;
    }
    let initialized = navigation["initialized"]
        .as_bool()
        .context("navigation initialization flag absent")?;
    let owned = count(navigation, "owned_wrappers")?;
    let bound = count(navigation, "bound_wrappers")?;
    let caches = count(navigation, "initialized_caches")?;
    let eligible = count(navigation, "eligible_placements")?;
    let faces = count(navigation, "faces")?;
    let members = count(navigation, "category_members")?;
    let nonempty = count(navigation, "nonempty_categories")?;
    ensure!(
        navigation["origin"] == "fresh_owner_snapshot"
            && [
                "original_history_verified",
                "native_precision_verified",
                "queries_executed"
            ]
            .into_iter()
            .all(|field| navigation[field] == false)
            && eligible <= count(navigation, "source_placements")?
            && owned <= eligible
            && bound <= owned
            && caches <= bound
            && count(navigation, "category_slots")? == owned * 15
            && nonempty <= count(navigation, "category_slots")?
            && nonempty <= members
            && (nonempty == 0) == (members == 0)
            && members <= faces
            && count(navigation, "boundary_pairs")? <= faces * 3
            && (owned != 0 || (count(navigation, "vertices")? == 0 && faces == 0))
            && (!initialized || (eligible == owned && owned == bound && bound == caches))
            && [
                "bindings_sha256",
                "categories_sha256",
                "cache_sha256",
                "state_sha256"
            ]
            .into_iter()
            .all(
                |field| navigation[field]
                    .as_str()
                    .is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            )
            && navigation["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "navigation ownership, category/cache counts or source-snapshot scope invalid"
    );
    Ok(())
}

const POST_TRIGGER_COUNTS: &[&str] = &[
    "ordered_requests",
    "resolved_requests",
    "retained_files",
    "retained_sections",
    "retained_file_bytes",
    "retained_section_bytes",
];

fn validate_post_trigger_resources(receipt: &Value) -> Result<()> {
    if receipt.is_null() {
        return Ok(());
    }
    let fields: Vec<_> = POST_TRIGGER_COUNTS
        .iter()
        .copied()
        .chain([
            "origin",
            "provider",
            "operation_complete",
            "selection_performed",
            "selected_definition_bound",
            "tail_reset_applied",
            "requests_sha256",
            "residency_sha256",
            "state_sha256",
            "original_cursor_history_verified",
            "original_rng_history_verified",
            "original_stream_history_verified",
            "native_stream_timing_verified",
            "native_stream_readiness_verified",
            "scope",
        ])
        .collect();
    closed(receipt, &fields)?;
    for field in POST_TRIGGER_COUNTS {
        count(receipt, field)?;
    }
    let complete = receipt["operation_complete"]
        .as_bool()
        .context("post-trigger completion flag absent")?;
    let selected = receipt["selection_performed"]
        .as_bool()
        .context("post-trigger selection flag absent")?;
    let bound = receipt["selected_definition_bound"]
        .as_bool()
        .context("post-trigger definition flag absent")?;
    let reset = receipt["tail_reset_applied"]
        .as_bool()
        .context("post-trigger reset flag absent")?;
    let ordered = count(receipt, "ordered_requests")?;
    let resolved = count(receipt, "resolved_requests")?;
    let files = count(receipt, "retained_files")?;
    let sections = count(receipt, "retained_sections")?;
    let file_bytes = count(receipt, "retained_file_bytes")?;
    let section_bytes = count(receipt, "retained_section_bytes")?;
    ensure!(
        receipt["origin"] == "source_hook_snapshot"
            && receipt["provider"] == "synchronous_supplied_data_residency"
            && [
                "original_cursor_history_verified",
                "original_rng_history_verified",
                "original_stream_history_verified",
                "native_stream_timing_verified",
                "native_stream_readiness_verified"
            ]
            .into_iter()
            .all(|field| receipt[field] == false)
            && (!bound || selected)
            && (bound || ordered == 0)
            && resolved <= ordered
            && ordered <= 2
            && files <= ordered
            && sections <= resolved
            && (files == 0) == (file_bytes == 0)
            && (sections != 0 || section_bytes == 0)
            && (files != 0 || sections == 0)
            && section_bytes <= file_bytes
            && (!complete
                || (reset
                    && if bound {
                        selected && ordered == 2 && resolved == 2 && files > 0
                    } else {
                        !selected && ordered == 0
                    }))
            && ["requests_sha256", "residency_sha256", "state_sha256"]
                .into_iter()
                .all(
                    |field| receipt[field].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                )
            && receipt["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "post-trigger ordered requests, retained residency or source-history scope invalid"
    );
    Ok(())
}

const WORLD_HOOK_COUNTS: &[&str] = &[
    "clock_reads",
    "constructor_clock_reads",
    "completed_clock_resets",
    "clock_frame_updates",
    "clock_seconds_bits",
    "clock_timescale_bits",
    "scoped_buffers",
    "buffers_cleared",
    "buffers_preserved",
    "backings_released",
    "bytes_released",
    "retained_backing_bytes",
    "literal_noops_reached",
    "callback_invocations",
];

fn validate_world_hooks(hooks: &Value) -> Result<()> {
    if hooks.is_null() {
        return Ok(());
    }
    let fields: Vec<_> = WORLD_HOOK_COUNTS
        .iter()
        .copied()
        .chain([
            "origin",
            "lifetime",
            "clock_provider",
            "operation_complete",
            "clock_reset_complete",
            "clock_snapshot_bound",
            "leaf_reset_complete",
            "binding_released",
            "callback_installed",
            "clock_state_sha256",
            "final_state_sha256",
            "state_sha256",
            "original_history_verified",
            "native_clock_precision_verified",
            "native_timing_verified",
            "world_ready_emitted",
            "scope",
        ])
        .collect();
    closed(hooks, &fields)?;
    for field in WORLD_HOOK_COUNTS {
        count(hooks, field)?;
    }
    for field in [
        "operation_complete",
        "clock_reset_complete",
        "clock_snapshot_bound",
        "leaf_reset_complete",
        "binding_released",
        "callback_installed",
    ] {
        ensure!(hooks[field].is_boolean(), "world hook flag {field} absent");
    }
    let complete = hooks["operation_complete"] == true;
    let clock_complete = hooks["clock_reset_complete"] == true;
    let seconds_bits = u32::try_from(count(hooks, "clock_seconds_bits")?)
        .context("clock seconds bits exceedu32")?;
    let seconds = f32::from_bits(seconds_bits);
    let scoped = count(hooks, "scoped_buffers")?;
    let cleared = count(hooks, "buffers_cleared")?;
    let preserved = count(hooks, "buffers_preserved")?;
    let released = count(hooks, "backings_released")?;
    ensure!(
        hooks["origin"] == "fresh_constructor_snapshot"
            && hooks["lifetime"] == "native_window_controller"
            && hooks["clock_provider"] == "host_frame_elapsed_f32"
            && seconds.is_finite()
            && seconds >= 0.0
            && hooks["clock_timescale_bits"] == 0x3f80_0000u32
            && [
                "original_history_verified",
                "native_clock_precision_verified",
                "native_timing_verified",
                "world_ready_emitted"
            ]
            .into_iter()
            .all(|field| hooks[field] == false)
            && cleared + preserved <= scoped
            && released <= cleared
            && (released != 0 || hooks["bytes_released"] == 0)
            && (!clock_complete
                || (hooks["clock_snapshot_bound"] == true
                    && hooks["clock_reads"] == 16
                    && hooks["constructor_clock_reads"] == 16
                    && count(hooks, "completed_clock_resets")? >= 2))
            && (!complete
                || (clock_complete
                    && scoped == 24
                    && cleared == 22
                    && preserved == 2
                    && released <= 22
                    && hooks["leaf_reset_complete"] == true
                    && hooks["literal_noops_reached"] == 2
                    && hooks["callback_installed"] == true
                    && hooks["callback_invocations"] == 0))
            && ["clock_state_sha256", "final_state_sha256", "state_sha256"]
                .into_iter()
                .all(
                    |field| hooks[field].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
                )
            && hooks["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "world hook clock/provider, scoped buffer ownership or readiness scope invalid"
    );
    Ok(())
}

const LOADER_COUNTS: &[&str] = &[
    "entry_phase",
    "current_phase",
    "maps_constructed",
    "map_zones",
    "map_component_slots",
    "map_context_bindings",
    "map_sections",
    "map_sections_published",
    "map_zones_published",
    "map_bindings",
    "checkpoint_payload_bytes",
    "checkpoint_skips",
    "source_bank_calls",
    "metadata_jobs_submitted",
    "backend_commands",
    "backend_sequence",
    "retained_banks",
    "retained_bank_bytes",
    "sound_entries",
    "wave_entries",
    "sound_info_rows",
    "sound_info_bytes",
    "work_completed",
    "work_pending",
    "work_failed",
    "barrier_promotions",
    "resource_polls",
    "work_busy_polls",
    "phase8_byte",
    "phase8_resets",
    "loader_c4_bits",
    "loader_c0_bits",
    "catalog_phase18_entries",
    "readiness_events",
    "readiness_local_stores",
    "readiness_owner_flags",
    "readiness_peer_flags",
    "readiness_user_flags",
    "readiness_local_flags",
];
const LOADER_FLAGS: &[&str] = &[
    "operation_complete",
    "pre_hooks_complete",
    "checkpoint_bound",
    "checkpoint_restore_active",
    "checkpoint_payload_initialized",
    "sound_info_bound",
    "world_ready_emitted",
    "player_created",
    "original_history_verified",
    "native_sdk_dispatch_verified",
    "native_work_timing_verified",
    "banks_playback_verified",
];

fn validate_loader(loader: &Value, world: &Value) -> Result<bool> {
    if loader.is_null() {
        return Ok(false);
    }
    let fields: Vec<_> = LOADER_COUNTS
        .iter()
        .chain(LOADER_FLAGS)
        .copied()
        .chain([
            "origin",
            "lifetime",
            "provider",
            "pending_phase",
            "pre_state_sha256",
            "maps_state_sha256",
            "banks_state_sha256",
            "phase_state_sha256",
            "state_sha256",
            "scope",
        ])
        .collect();
    closed(loader, &fields)?;
    for field in LOADER_COUNTS {
        count(loader, field)?;
    }
    for field in LOADER_FLAGS {
        ensure!(loader[field].is_boolean(), "loader flag {field} absent");
    }
    let complete = loader["operation_complete"] == true;
    let current = count(loader, "current_phase")?;
    let pending = if loader["pending_phase"].is_null() {
        None
    } else {
        Some(count(loader, "pending_phase")?)
    };
    let commands = count(loader, "backend_commands")?;
    ensure!(
        loader["origin"] == "fresh_constructor_snapshot"
            && loader["lifetime"] == "native_window_controller"
            && loader["provider"] == "scoped_supplied_audio_and_base_map"
            && loader["entry_phase"] == 7
            && if current == 28 {
                pending.is_some_and(|next| matches!(next, 8 | 10..=19 | 21))
            } else {
                pending.is_none()
                    && (matches!(current, 7 | 8 | 10..=19) || (complete && current == 21))
            }
            && [
                "backend_sequence",
                "loader_c4_bits",
                "loader_c0_bits",
                "readiness_owner_flags",
                "readiness_peer_flags",
                "readiness_user_flags",
                "readiness_local_flags"
            ]
            .into_iter()
            .all(|field| loader[field]
                .as_u64()
                .is_some_and(|value| value <= u64::from(u32::MAX)))
            && count(loader, "phase8_byte")? <= u64::from(u8::MAX)
            && [
                "player_created",
                "original_history_verified",
                "native_sdk_dispatch_verified",
                "native_work_timing_verified",
                "banks_playback_verified"
            ]
            .into_iter()
            .all(|field| loader[field] == false)
            && loader["world_ready_emitted"] == complete
            && count(loader, "maps_constructed")? <= count(world, "maps_activated")?
            && count(loader, "map_sections_published")? <= count(loader, "map_sections")?
            && count(loader, "map_zones_published")? <= count(loader, "map_zones")?
            && count(loader, "map_context_bindings")? <= count(loader, "maps_constructed")?
            && (loader["checkpoint_bound"] == true
                || (loader["checkpoint_payload_bytes"] == 0
                    && loader["checkpoint_skips"] == 0
                    && loader["checkpoint_payload_initialized"] == false))
            && (loader["checkpoint_bound"] == false
                || loader["checkpoint_payload_bytes"] == 0x37f8)
            && count(loader, "source_bank_calls")? <= 3
            && count(loader, "metadata_jobs_submitted")? <= 1
            && (loader["retained_banks"] != 0
                || ["retained_bank_bytes", "sound_entries", "wave_entries"]
                    .into_iter()
                    .all(|field| loader[field] == 0))
            && (loader["retained_banks"] == 0 || count(loader, "retained_bank_bytes")? > 0)
            && count(loader, "sound_entries")? <= count(loader, "retained_bank_bytes")?
            && count(loader, "wave_entries")? <= count(loader, "retained_bank_bytes")?
            && if loader["sound_info_bound"] == true {
                count(loader, "sound_info_bytes")? > 0
                    && count(loader, "sound_info_rows")? <= count(loader, "sound_info_bytes")?
            } else {
                loader["sound_info_rows"] == 0 && loader["sound_info_bytes"] == 0
            }
            && count(loader, "barrier_promotions")? <= 12
            && count(loader, "resource_polls")? >= count(loader, "barrier_promotions")?
            && if complete {
                current == 21
                    && pending.is_none()
                    && loader["pre_hooks_complete"] == true
                    && loader["maps_constructed"] == world["maps_activated"]
                    && loader["checkpoint_bound"] == true
                    && loader["checkpoint_restore_active"] == false
                    && loader["checkpoint_skips"] == 2
                    && loader["sound_info_bound"] == true
                    && commands >= count(loader, "source_bank_calls")?
                    && count(loader, "work_completed")? > commands
                    && loader["work_pending"] == 0
                    && loader["work_failed"] == 0
                    && loader["barrier_promotions"] == 12
                    && loader["resource_polls"] == 12
                    && count(loader, "work_busy_polls")? >= commands
                    && loader["phase8_byte"] == 10
                    && count(loader, "phase8_resets")? >= 2
                    && loader["loader_c4_bits"] == 0x44bb_8000u32
                    && loader["loader_c0_bits"] == 0x3a83_126fu32
                    && world["catalog_complete"] == true
                    && loader["catalog_phase18_entries"] == world["start_points"]
                    && world["navigation"]["initialized"] == true
                    && world["post_trigger_resources"]["operation_complete"] == true
                    && world["world_hooks"]["operation_complete"] == true
                    && loader["readiness_events"] == 2
                    && loader["readiness_local_stores"] == 4
                    && [
                        "readiness_owner_flags",
                        "readiness_peer_flags",
                        "readiness_user_flags",
                    ]
                    .into_iter()
                    .all(|field| loader[field] == 0x108)
                    && loader["readiness_local_flags"] == 0x109
            } else {
                [
                    "readiness_events",
                    "readiness_local_stores",
                    "readiness_owner_flags",
                    "readiness_peer_flags",
                    "readiness_user_flags",
                ]
                .into_iter()
                .all(|field| loader[field] == 0)
                    && matches!(loader["readiness_local_flags"].as_u64(), Some(0 | 1))
            }
            && [
                "pre_state_sha256",
                "maps_state_sha256",
                "banks_state_sha256",
                "phase_state_sha256",
                "state_sha256"
            ]
            .into_iter()
            .all(
                |field| loader[field].as_str().is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            )
            && loader["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "loader phase/provider ownership or readiness evidence is contradictory"
    );
    Ok(complete)
}

const PLAYER_COUNTS: &[&str] = &[
    "operation_entries",
    "creation_dispatches",
    "catalog_entries",
    "selected_catalog_index",
    "selected_source_ordinal",
    "requested_start_point_before",
    "requested_start_point_after",
    "handler_rng_reads",
    "base_reset_calls",
    "persistent_base_resets",
    "inventory_records",
    "inventory_capacity",
    "inventory_growth",
    "world_items",
    "world_actor_lists",
    "world_membership_lists",
    "actor_registrations",
    "shared_count",
    "shared_live_leases",
    "shared_allocations",
    "shared_native_bytes",
    "shared_host_capacity_bytes",
    "shared_reset_calls",
    "shared_entry_noops",
    "shared_cache_stores",
    "scratch_capacity",
    "scratch_growth",
    "scratch_records",
    "scratch_native_bytes",
    "scratch_host_capacity_bytes",
    "scratch_reallocations",
    "character_resource",
    "character_key",
    "character_source_index",
    "guns_resource",
    "guns_payload_bytes",
    "metadata_source_index",
    "metadata_sorted_index",
    "weapon_registry_keys",
    "primary_order_keys",
    "secondary_order_keys",
    "inventory_primary_order_index",
    "inventory_interface_callbacks",
    "inventory_selected_record",
];
const PLAYER_FLAGS: &[&str] = &[
    "operation_complete",
    "attempt_bound",
    "catalog_bound",
    "base_constructor_complete",
    "handler_kind_bound",
    "physics_bound",
    "completion_published",
    "player_created",
    "camera_ready",
    "original_history_verified",
    "native_timing_verified",
    "shared_reset_complete",
    "runtime_base_reset_complete",
    "derived_reset_prefix_complete",
    "scratch_prepared",
    "character_bound",
    "inventory_selection_complete",
];
const PLAYER_HASHES: &[&str] = &[
    "base_state_sha256",
    "prelude_state_sha256",
    "world_state_sha256",
    "persistent_state_sha256",
    "state_sha256",
    "shared_state_sha256",
    "reset_state_sha256",
    "scratch_state_sha256",
    "loadout_state_sha256",
];

const VIRTUAL_COUNTS: &[&str] = &[
    "item_registry_records",
    "item_registry_capacity",
    "item_registry_growth",
    "item_registry_counter",
    "item_registry_native_bytes",
    "item_registry_host_capacity_bytes",
    "origin_zone_queries",
    "origin_zone_nodes",
    "desired_membership",
    "item_membership",
    "item_flags",
    "item_priority",
];
const VIRTUAL_FLAGS: &[&str] = &[
    "factory_complete",
    "item_registry_bound",
    "actor_registered",
    "handler_bound",
    "origin_zone_queried",
    "physics_constructed",
    "physics_backlink_bound",
    "physics_activated",
    "initializer_complete",
    "scalar_pending",
    "scalar_value_known",
];
const VIRTUAL_HASHES: &[&str] = &[
    "item_state_sha256",
    "physics_state_sha256",
    "registry_state_sha256",
];

/// A null pair is the explicit unknown-argument reset-only branch. Present
/// objects prove retained owners and the basic initializer return, separately
/// from PlayerNew completion and the unresolved physics scalar.
fn validate_player_virtual_owners(player: &Value) -> Result<bool> {
    let flag = &player["constructor_flag"];
    let virtual_init = &player["virtual_initialization"];
    if flag.is_null() && virtual_init.is_null() {
        return Ok(false);
    }
    closed(
        flag,
        &[
            "origin",
            "raw",
            "changed",
            "interface_callbacks",
            "original_history_verified",
        ],
    )?;
    ensure!(
        flag["origin"] == "captured_member_snapshot"
            && count(flag, "raw")? <= 255
            && flag["changed"].is_boolean()
            && flag["interface_callbacks"] == 0
            && flag["original_history_verified"] == false,
        "captured constructor argument evidence is invalid"
    );
    let fields: Vec<_> = VIRTUAL_COUNTS
        .iter()
        .chain(VIRTUAL_FLAGS)
        .chain(VIRTUAL_HASHES)
        .copied()
        .chain(["stage", "pending_dependency", "scope"])
        .collect();
    closed(virtual_init, &fields)?;
    for field in VIRTUAL_COUNTS {
        count(virtual_init, field)?;
    }
    let records = count(virtual_init, "item_registry_records")?;
    let capacity = count(virtual_init, "item_registry_capacity")?;
    ensure!(
        virtual_init["stage"] == "player_basic_complete"
            && virtual_init["pending_dependency"] == "tp_fp_components"
            && VIRTUAL_FLAGS[..10]
                .iter()
                .all(|field| virtual_init[field] == true)
            && virtual_init["scalar_value_known"] == false
            && records >= 1
            && records <= count(player, "world_items")?
            && capacity >= records
            && capacity <= u64::from(i32::MAX as u32)
            && virtual_init["item_registry_growth"] == 16
            && count(virtual_init, "item_registry_counter")? >= records
            && count(virtual_init, "item_registry_counter")? <= u64::from(u32::MAX)
            && capacity.checked_mul(8) == Some(count(virtual_init, "item_registry_native_bytes")?)
            && count(virtual_init, "item_registry_native_bytes")? <= u64::from(u32::MAX)
            && count(virtual_init, "item_registry_host_capacity_bytes")?
                >= count(virtual_init, "item_registry_native_bytes")?
            && virtual_init["origin_zone_queries"] == 1
            && count(virtual_init, "origin_zone_nodes")? >= 1
            && virtual_init["desired_membership"] == 0x2d
            && virtual_init["item_membership"] == 0x3d
            && virtual_init["item_flags"] == 0x2b
            && virtual_init["item_priority"] == 255
            && count(player, "actor_registrations")? >= 1
            && VIRTUAL_HASHES
                .iter()
                .all(|field| virtual_init[field]
                    .as_str()
                    .is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))))
            && virtual_init["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "virtual item/registry/actor/physics owner evidence is contradictory"
    );
    Ok(true)
}

fn validate_player_components(component: &Value, reached: bool) -> Result<()> {
    if !reached {
        ensure!(
            component.is_null(),
            "reset-only attempt cannot own a component receipt"
        );
        return Ok(());
    }
    closed(
        component,
        &[
            "stage",
            "pending_dependency",
            "native_component_bytes",
            "constructed_slots",
            "active_slots",
            "retained_file_bytes",
            "retained_section_bytes",
            "third_person_constructed",
            "third_person_attached",
            "first_person_constructed",
            "skin_request_resolved",
            "texture_dependencies_enumerated",
            "texture_directory_outcome",
            "texture_requests",
            "texture_targets_resident",
            "texture_residency_complete",
            "texture_requests_state_sha256",
            "component_binding_complete",
            "preparation_state_sha256",
            "target_state_sha256",
            "skeleton_binding",
            "bind_refresh",
            "scope",
        ],
    )?;
    ensure!(
        component["stage"] == "tp_attachment_pending"
            && component["pending_dependency"] == "tp_attachment"
            && component["native_component_bytes"] == 0x1b8
            && component["constructed_slots"] == 4
            && component["active_slots"] == 1
            && component["third_person_constructed"] == true
            && component["skin_request_resolved"] == true
            && component["texture_residency_complete"] == true
            && component["component_binding_complete"] == true
            && [
                "enumerated",
                "entity_directory_absent",
                "texture_directory_absent"
            ]
            .iter()
            .any(|outcome| component["texture_directory_outcome"] == *outcome)
            && component["texture_dependencies_enumerated"]
                == (component["texture_directory_outcome"] == "enumerated")
            && count(component, "texture_requests")? <= 4096
            && count(component, "texture_requests")?
                == count(component, "texture_targets_resident")?
            && (component["texture_directory_outcome"] == "enumerated"
                || component["texture_requests"] == 0)
            && ["third_person_attached", "first_person_constructed"]
                .into_iter()
                .all(|field| component[field] == false)
            && (0xc8..=64 * 1024 * 1024).contains(&count(component, "retained_file_bytes")?)
            && count(component, "retained_section_bytes")?
                <= count(component, "retained_file_bytes")?
            && [
                "preparation_state_sha256",
                "target_state_sha256",
                "texture_requests_state_sha256"
            ]
            .into_iter()
            .all(
                |field| component[field]
                    .as_str()
                    .is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            )
            && component["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "component constructor/residency evidence is contradictory"
    );
    player_skeleton::validate(&component["skeleton_binding"])?;
    player_refresh::validate(&component["bind_refresh"])?;
    Ok(())
}

fn validate_player_initialization(player: &Value, world: &Value, snapshot: &Value) -> Result<()> {
    if player.is_null() {
        return Ok(());
    }
    let fields: Vec<_> = PLAYER_COUNTS
        .iter()
        .chain(PLAYER_FLAGS)
        .chain(PLAYER_HASHES)
        .copied()
        .chain([
            "origin",
            "lifetime",
            "stage",
            "pending_dependency",
            "scope",
            "constructor_flag",
            "virtual_initialization",
            "component_initialization",
        ])
        .collect();
    closed(player, &fields)?;
    for field in PLAYER_COUNTS {
        count(player, field)?;
    }
    for field in PLAYER_FLAGS {
        ensure!(
            player[field].is_boolean(),
            "player initialization flag absent"
        );
    }
    let virtual_reached = validate_player_virtual_owners(player)?;
    validate_player_components(&player["component_initialization"], virtual_reached)?;
    ensure!(
        player["origin"] == "fresh_constructor_snapshot"
            && player["lifetime"] == "controller_arena"
            && (if virtual_reached {
                player["stage"] == "player_components_pending"
                    && player["pending_dependency"] == "tp_attachment"
            } else {
                player["stage"] == "player_reset_complete"
                    && player["pending_dependency"] == "constructor_source_flag"
            })
            && player["physics_bound"] == virtual_reached
            && [
                "operation_complete",
                "handler_kind_bound",
                "completion_published",
                "player_created",
                "camera_ready",
                "original_history_verified",
                "native_timing_verified"
            ]
            .into_iter()
            .all(|field| player[field] == false)
            && [
                "attempt_bound",
                "catalog_bound",
                "base_constructor_complete",
                "runtime_base_reset_complete",
                "derived_reset_prefix_complete",
                "shared_reset_complete",
                "scratch_prepared",
                "character_bound",
                "inventory_selection_complete"
            ]
            .into_iter()
            .all(|field| player[field] == true)
            && world["loader"]["operation_complete"] == true
            && world["catalog_complete"] == true
            && snapshot["world_registered"] == true
            && snapshot["player_created"] == false
            && snapshot["camera_ready"] == false
            && count(player, "creation_dispatches")? == count(snapshot, "local_users")?
            && count(player, "base_reset_calls")? == 2 * count(player, "creation_dispatches")?
            && player["handler_rng_reads"] == player["creation_dispatches"]
            && count(player, "persistent_base_resets")? >= count(player, "base_reset_calls")?
            && count(player, "shared_count")? >= 1
            && player["shared_count"] == player["shared_live_leases"]
            && player["shared_reset_calls"] == player["shared_live_leases"]
            && player["shared_allocations"] == 3
            && player["shared_native_bytes"] == 63_680
            && count(player, "shared_host_capacity_bytes")?
                >= count(player, "shared_native_bytes")?
            && player["shared_entry_noops"] == 98
            && player["shared_cache_stores"] == 1
            && player["catalog_entries"] == world["catalog_entries"]
            && count(player, "selected_catalog_index")? < count(player, "catalog_entries")?
            && count(player, "selected_source_ordinal")? < count(world, "original_slots")?
            && player["requested_start_point_before"] == snapshot["start_point_id"]
            && count(player, "requested_start_point_after")? <= u64::from(u32::MAX)
            && count(player, "actor_registrations")? <= count(player, "world_items")?
            && player["inventory_growth"] == 16
            && player["inventory_records"] == 1
            && player["inventory_capacity"] == 16
            && player["inventory_selected_record"] == 0
            && player["inventory_interface_callbacks"] == 0
            && player["scratch_capacity"] == 32
            && player["scratch_growth"] == 16
            && player["scratch_records"] == 0
            && player["scratch_native_bytes"] == 4736
            && count(player, "scratch_host_capacity_bytes")? >= 4736
            && player["scratch_reallocations"] == 1
            && player["character_resource"] == 0x0109_0013
            && count(player, "character_key")? >> 16 == 0x4d00
            && count(player, "character_key")? & 0xffff != 0
            && player["guns_resource"] == 0x0109_000f
            && count(player, "guns_payload_bytes")? > 0
            && count(player, "metadata_sorted_index")? < count(player, "weapon_registry_keys")?
            && count(player, "inventory_primary_order_index")?
                < count(player, "primary_order_keys")?
            && count(player, "secondary_order_keys")? > 0
            && count(player, "world_actor_lists")? > 0
            && count(player, "world_membership_lists")? >= 5
            && count(
                world
                    .get("actor_definitions")
                    .context("player actor owner absent")?,
                "rng_draws"
            )? >= count(player, "handler_rng_reads")?
            && count(
                world
                    .get("resource_states")
                    .context("player resource owner absent")?,
                "records"
            )? > 0
            && PLAYER_HASHES
                .iter()
                .all(
                    |field| player[field].as_str().is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
                )
            && player["scope"]
                .as_str()
                .is_some_and(|text| !text.is_empty() && text.len() <= 4096),
        "player virtual reset, scratch/loadout ownership, dependency or readiness evidence is contradictory"
    );
    Ok(())
}

fn validate_world(snapshot: &Value) -> Result<()> {
    let world = &snapshot["world_initialization"];
    let fields: Vec<_> = WORLD_COUNTS
        .iter()
        .copied()
        .chain([
            "catalog_complete",
            "native_zone_precision_verified",
            "state_sha256",
            "scope",
            "actor_definitions",
            "resource_states",
            "navigation",
            "post_trigger_resources",
            "world_hooks",
            "loader",
            "player_initialization",
        ])
        .collect();
    closed(world, &fields)?;
    for field in WORLD_COUNTS {
        count(world, field)?;
    }
    validate_navigation(&world["navigation"])?;
    validate_post_trigger_resources(&world["post_trigger_resources"])?;
    validate_world_hooks(&world["world_hooks"])?;
    validate_loader(&world["loader"], world)?;
    validate_player_initialization(&world["player_initialization"], world, snapshot)?;
    validate_actor_definitions(&world["actor_definitions"])?;
    validate_resource_states(
        &world["resource_states"],
        count(world, "radius03_triggers")?,
    )?;
    ensure!(
        world["state_sha256"]
            .as_str()
            .is_some_and(|hash| hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
            && world["scope"]
                .as_str()
                .is_some_and(|scope| !scope.is_empty() && scope.len() <= 4096),
        "world initialization digest or scope invalid"
    );
    let owned = count(world, "owned_triggers")?;
    let start_points = count(world, "start_points")?;
    let generic = count(world, "generic_triggers")?;
    let common_be = count(world, "common_be_triggers")?;
    let resource08 = count(world, "resource08_triggers")?;
    let active91 = count(world, "active91_triggers")?;
    let range0c = count(world, "range0c_triggers")?;
    let radius06 = count(world, "radius06_triggers")?;
    let actor12 = count(world, "actor12_triggers")?;
    let range82 = count(world, "range82_triggers")?;
    let a3 = count(world, "a3_triggers")?;
    let range27 = count(world, "range27_triggers")?;
    let common09 = count(world, "common09_triggers")?;
    let ad = count(world, "ad_triggers")?;
    let radius03 = count(world, "radius03_triggers")?;
    let common8a = count(world, "common8a_triggers")?;
    let radius1b = count(world, "radius1b_triggers")?;
    let radius74 = count(world, "radius74_triggers")?;
    let shape71 = count(world, "shape71_triggers")?;
    let radius88 = count(world, "radius88_triggers")?;
    let auxac = count(world, "auxac_triggers")?;
    let staged = count(world, "staged_triggers")?;
    ensure!(
        owned
            == start_points
                + generic
                + common_be
                + resource08
                + active91
                + range0c
                + radius06
                + actor12
                + range82
                + a3
                + range27
                + common09
                + ad
                + radius03
                + common8a
                + radius1b
                + radius74
                + shape71
                + radius88
                + auxac
            && staged
                <= generic
                    + resource08
                    + range0c
                    + radius06
                    + actor12
                    + range82
                    + range27
                    + radius03
                    + common8a
                    + shape71
            && staged <= count(world, "unsupported_factory_slots")?,
        "world trigger kinds or staged factory counts differ"
    );
    let members = owned - staged;
    let slots = [
        "disabled_slots",
        "filtered_slots",
        "unclassified_slots",
        "source_null_slots",
        "unsupported_factory_slots",
        "unsupported_shape_slots",
        "owned_triggers",
    ]
    .into_iter()
    .try_fold(0u64, |sum, field| -> Result<u64> {
        Ok(sum + count(world, field)?)
    })? - staged;
    let initialized = count(world, "animators_initialized")?;
    let active = count(world, "active_triggers")?;
    ensure!(
        count(world, "maps_activated")? == count(snapshot, "map_instances")?
            && count(world, "original_slots")? == slots
            && count(world, "zone_queries")? == slots
            && count(world, "common_scratch")? == owned
            && count(world, "shape_selectors_initialized")? <= owned
            && count(world, "materialized_shapes")? <= shape71
            && count(world, "map_initialization_blocked")? <= actor12
            && (world["actor_definitions"]["native9_constructor_ready"] != true
                || (actor12 > 0 && world["map_initialization_blocked"] == 0))
            && count(world, "membership_entries")? == members
            && active <= members
            && start_points == count(snapshot, "source_start_points")?
            && initialized + count(world, "animators_failed")? <= active
            && count(world, "stopped_animators")? == members - active
            && count(world, "blocked_references")? + count(world, "resolved_references")?
                <= owned * 8
            && count(world, "create_events_denied")? == initialized
            && count(world, "registration_events_unmapped")? == initialized
            && count(world, "execution_slots")? == initialized * 16,
        "world initialization ownership/ordinal/event counts differ"
    );
    let catalog_complete = world["catalog_complete"]
        .as_bool()
        .context("trigger catalog completion flag absent")?;
    let catalog_entries = count(world, "catalog_entries")?;
    ensure!(
        catalog_entries <= start_points
            && if catalog_complete {
                catalog_entries == start_points
                    && count(world, "materialized_shapes")? == shape71
                    && count(world, "maps_activated")? > 0
                    && [
                        "unclassified_slots",
                        "unsupported_factory_slots",
                        "unsupported_shape_slots",
                        "staged_triggers",
                        "blocked_references",
                        "map_initialization_blocked",
                        "animators_failed",
                    ]
                    .into_iter()
                    .all(|field| world[field] == 0)
            } else {
                catalog_entries == 0
            },
        "trigger catalog completion contradicts constructor, fixup or map initialization blockers"
    );
    ensure!(
        world["animators_failed"] == 0
            && count(world, "unwritten_scratch_words")? == initialized * 2
            && world["active_executions"] == 0
            && world["native_zone_precision_verified"] == false,
        "trigger/animator checkpoint differs or promotes native precision"
    );
    Ok(())
}

/// This gate runs only the supplied Tutorial New Game. These independently
/// observed source counts are separate from generic invariants/authored data.
fn validate_tutorial_world(probe: &Value) -> Result<()> {
    let world = &probe["window"]["game_session"]["active"]["world_initialization"];
    for (field, expected) in [
        ("maps_activated", 2),
        ("original_slots", 196),
        ("zone_queries", 196),
        ("disabled_slots", 0),
        ("filtered_slots", 1),
        ("unclassified_slots", 0),
        ("source_null_slots", 1),
        ("unsupported_factory_slots", 0),
        ("unsupported_shape_slots", 0),
        ("owned_triggers", 194),
        ("common_scratch", 194),
        ("membership_entries", 194),
        ("active_triggers", 10),
        ("start_points", 3),
        ("generic_triggers", 70),
        ("common_be_triggers", 40),
        ("resource08_triggers", 5),
        ("active91_triggers", 7),
        ("range0c_triggers", 4),
        ("radius06_triggers", 19),
        ("actor12_triggers", 10),
        ("range82_triggers", 3),
        ("a3_triggers", 9),
        ("range27_triggers", 5),
        ("common09_triggers", 1),
        ("ad_triggers", 2),
        ("radius03_triggers", 3),
        ("common8a_triggers", 1),
        ("radius1b_triggers", 1),
        ("radius74_triggers", 4),
        ("shape71_triggers", 4),
        ("materialized_shapes", 4),
        ("radius88_triggers", 2),
        ("auxac_triggers", 1),
        ("map_initialization_blocked", 0),
        ("shape_selectors_initialized", 9),
        ("staged_triggers", 0),
        ("stopped_animators", 184),
        ("blocked_references", 0),
        ("resolved_references", 242),
        ("animators_initialized", 3),
        ("animators_failed", 0),
        ("shared_variables", 72),
        ("unwritten_scratch_words", 6),
        ("state_flags", 3),
        ("execution_slots", 48),
        ("active_executions", 0),
        ("create_events_denied", 3),
        ("registration_events_unmapped", 3),
        ("catalog_entries", 3),
    ] {
        ensure!(
            world[field] == expected,
            "Tutorial world source count {field} differs"
        );
    }
    ensure!(
        world["catalog_complete"] == true,
        "Tutorial trigger catalog construction incomplete"
    );
    let navigation = &world["navigation"];
    validate_navigation(navigation)?;
    ensure!(
        navigation.is_object() && navigation["initialized"] == true,
        "Tutorial navigation owner initialization absent or incomplete"
    );
    for (field, expected) in [
        ("source_placements", 1),
        ("eligible_placements", 1),
        ("owned_wrappers", 1),
        ("bound_wrappers", 1),
        ("initialized_caches", 1),
        ("vertices", 28),
        ("faces", 28),
        ("category_slots", 15),
        ("nonempty_categories", 0),
        ("category_members", 0),
        ("boundary_pairs", 28),
    ] {
        ensure!(
            navigation[field] == expected,
            "Tutorial navigation source count {field} differs"
        );
    }
    let post_trigger = &world["post_trigger_resources"];
    validate_post_trigger_resources(post_trigger)?;
    ensure!(
        post_trigger.is_object()
            && [
                "operation_complete",
                "selection_performed",
                "selected_definition_bound",
                "tail_reset_applied"
            ]
            .into_iter()
            .all(|field| post_trigger[field] == true),
        "Tutorial post-trigger resource operation absent or incomplete"
    );
    for (field, expected) in [
        ("ordered_requests", 2),
        ("resolved_requests", 2),
        ("retained_files", 1),
        ("retained_sections", 1),
        ("retained_file_bytes", 5_524_960),
        ("retained_section_bytes", 647_392),
    ] {
        ensure!(
            post_trigger[field] == expected,
            "Tutorial post-trigger resource count {field} differs"
        );
    }
    let hooks = &world["world_hooks"];
    validate_world_hooks(hooks)?;
    ensure!(
        hooks.is_object()
            && hooks["operation_complete"] == true
            && hooks["binding_released"] == false
            && count(hooks, "clock_frame_updates")? >= 1,
        "Tutorial world hook lifecycle or frame clock absent"
    );
    for field in [
        "backings_released",
        "bytes_released",
        "retained_backing_bytes",
    ] {
        ensure!(
            hooks[field] == 0,
            "Tutorial world hook {field} differs from fresh backing ownership"
        );
    }
    let loader = &world["loader"];
    ensure!(
        validate_loader(loader, world)?,
        "Tutorial loader pipeline did not complete"
    );
    for (field, expected) in [
        ("maps_constructed", 2),
        ("map_zones", 24),
        ("map_component_slots", 2),
        ("map_context_bindings", 2),
        ("map_sections", 8),
        ("map_sections_published", 2),
        ("map_zones_published", 1),
        ("map_bindings", 1),
        ("retained_banks", 3),
        ("retained_bank_bytes", 3_899_392),
        ("sound_entries", 431),
        ("wave_entries", 594),
        ("sound_info_rows", 8_082),
        ("sound_info_bytes", 131_072),
        ("catalog_phase18_entries", 3),
    ] {
        ensure!(
            loader[field] == expected,
            "Tutorial loader source count {field} differs"
        );
    }
    ensure!(
        (1..=3).contains(&count(loader, "source_bank_calls")?)
            && (probe["window"]["game_session"]["requested"] != 1
                || loader["backend_commands"] == loader["source_bank_calls"])
            && loader["checkpoint_payload_initialized"] == false,
        "Tutorial bank callbacks or fresh unwritten checkpoint scope differ"
    );
    let actor = &world["actor_definitions"];
    validate_actor_definitions(actor)?;
    ensure!(
        actor.is_object()
            && actor["primary_present"] == true
            && actor["secondary_present"] == true
            && actor["fresh_cursors_zero"] == false
            && actor["rng_bound"] == true
            && actor["native9_constructor_ready"] == true,
        "Tutorial actor definition source groups absent"
    );
    for (field, expected) in [
        ("payload_bytes", 23680),
        ("primary_rows", 314),
        ("secondary_rows", 65),
        ("registered_primary", 314),
        ("duplicate_primary", 0),
        ("runtime_groups", 53),
        ("grouped_members", 239),
        ("group8_members", 4),
        ("selection_calls", 11),
        ("rng_draws", 2),
    ] {
        ensure!(
            actor[field] == expected,
            "Tutorial actor definition source count {field} differs"
        );
    }
    ensure!(
        actor["state_sha256"] == "429948e1e53152cab380c18c511be2fe031134bb6f05b00f3ff448ef2bbc29b8"
            && actor["rng_state_sha256"]
                == "e7296b219d88c09463334a9d0091795c632b1d600785d62f8a0e40a494cb7d90",
        "Tutorial shared actor/handler RNG fingerprint differs from independent source proof"
    );
    let states = &world["resource_states"];
    validate_resource_states(states, count(world, "radius03_triggers")?)?;
    ensure!(
        states.is_object()
            && states["origin"] == "fresh_constructor_snapshot"
            && states["snapshot_bound"] == false,
        "Tutorial resource-state snapshot absent or changed"
    );
    for (field, expected) in [
        ("records", 1),
        ("capacity", 32),
        ("nonzero_values", 0),
        ("reset_limit", 2500),
        ("lookups", 2),
        ("hits", 0),
        ("misses", 2),
    ] {
        ensure!(
            states[field] == expected,
            "Tutorial resource-state count {field} differs"
        );
    }
    ensure!(
        states["state_sha256"]
            == "cc908123ee13a67a303300c7d4922c01806dfff734045cf99f3c1b1b5f54d77b",
        "Tutorial reset resource-state fingerprint differs from independent source proof"
    );
    let player = &world["player_initialization"];
    ensure!(
        player.is_object(),
        "Tutorial player prelude was not recorded"
    );
    validate_player_initialization(player, world, &probe["window"]["game_session"]["active"])?;
    let flag = &player["constructor_flag"];
    let virtual_init = &player["virtual_initialization"];
    ensure!(
        flag.is_object()
            && virtual_init.is_object()
            && flag["raw"] == 0
            && flag["changed"] == false,
        "Tutorial must capture the matching fresh metadata argument and reach real virtual owners"
    );
    let requested = count(&probe["window"]["game_session"], "requested")?;
    for value in [
        count(player, "world_items")?,
        count(player, "actor_registrations")?,
        count(virtual_init, "item_registry_records")?,
        count(virtual_init, "item_registry_counter")?,
    ] {
        ensure!(
            (1..=requested).contains(&value),
            "Tutorial retained item/registry state exceeds actual attempts"
        );
    }
    if requested == 1 {
        ensure!(
            player["world_items"] == 1
                && player["actor_registrations"] == 1
                && virtual_init["item_registry_records"] == 1
                && virtual_init["item_registry_counter"] == 1
                && virtual_init["item_registry_capacity"] == 16,
            "Tutorial first factory/actor registration differs from fresh constructor state"
        );
    }
    for (field, expected) in [
        ("operation_entries", 0),
        ("creation_dispatches", 1),
        ("catalog_entries", 3),
        ("selected_catalog_index", 0),
        ("selected_source_ordinal", 0),
        ("requested_start_point_before", 1),
        ("requested_start_point_after", 1),
        ("handler_rng_reads", 1),
        ("base_reset_calls", 2),
        ("shared_allocations", 3),
        ("shared_native_bytes", 63_680),
        ("shared_entry_noops", 98),
        ("shared_cache_stores", 1),
        ("world_actor_lists", 1),
        ("world_membership_lists", 10),
        ("scratch_capacity", 32),
        ("scratch_growth", 16),
        ("scratch_records", 0),
        ("scratch_native_bytes", 4736),
        ("scratch_reallocations", 1),
        ("character_resource", 0x0109_0013),
        ("character_key", 0x4d00_01b9),
        ("character_source_index", 1),
        ("guns_resource", 0x0109_000f),
        ("metadata_source_index", 0),
        ("metadata_sorted_index", 23),
        ("weapon_registry_keys", 69),
        ("primary_order_keys", 29),
        ("secondary_order_keys", 7),
        ("inventory_primary_order_index", 27),
        ("inventory_interface_callbacks", 0),
        ("inventory_selected_record", 0),
    ] {
        ensure!(
            player[field] == expected,
            "Tutorial player prelude count {field} differs"
        );
    }
    ensure!(
        (2..=2 * count(&probe["window"]["game_session"], "requested")?)
            .contains(&count(player, "persistent_base_resets")?),
        "Tutorial persistent base reset count exceeds actual attempts"
    );
    ensure!(
        (1..=count(&probe["window"]["game_session"], "requested")?)
            .contains(&count(player, "shared_live_leases")?),
        "Tutorial shared constructor leases exceed actual attempts"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum Check {
    DifficultyPage,
    Loaded { difficulty: u8, reentry: bool },
    ReturnedMenu,
}

pub(super) fn run(disc: &str, preferences: &str, folder: &Path) -> Result<Value> {
    let difficulty_page = super::probe(
        disc,
        preferences,
        folder,
        "new-game-difficulty",
        "confirm",
        true,
    )?;
    validate(&difficulty_page, Check::DifficultyPage)?;
    let mut difficulties = Vec::new();
    for difficulty in 0..=3 {
        let mut inputs = vec!["confirm"];
        inputs.extend(std::iter::repeat_n("down", usize::from(difficulty)));
        // The second confirm deliberately reaches the shared handler after
        // dispatch. It must produce no second request or accepted effect.
        inputs.extend(["confirm", "confirm"]);
        let probe = super::probe(
            disc,
            preferences,
            folder,
            &format!("new-game-{difficulty}"),
            &inputs.join(","),
            difficulty != 0,
        )?;
        validate(
            &probe,
            Check::Loaded {
                difficulty,
                reentry: false,
            },
        )?;
        validate_tutorial_world(&probe)?;
        difficulties.push(probe);
    }
    let cancel_reentry = super::probe(
        disc,
        preferences,
        folder,
        "new-game-cancel-reentry",
        "confirm,confirm,back,confirm,down,confirm",
        true,
    )?;
    validate(
        &cancel_reentry,
        Check::Loaded {
            difficulty: 1,
            reentry: true,
        },
    )?;
    validate_tutorial_world(&cancel_reentry)?;
    let returned_menu = super::probe(
        disc,
        preferences,
        folder,
        "new-game-return",
        "confirm,confirm,back",
        true,
    )?;
    validate(&returned_menu, Check::ReturnedMenu)?;
    Ok(json!({
        "difficulty_page": difficulty_page, "difficulty_probes": difficulties,
        "cancel_reentry": cancel_reentry, "returned_menu": returned_menu,
        "reference_compared": false,
        "scope": "Seven hidden native shared-handler probes present the tutorial difficulty page before allocating a session, load each source difficulty into one retained CPU session, suppress repeat confirmation, cancel/reenter Agent and return to the menu. One unmuted case verifies suspended primary menu music and zero reported device errors. Captures and counters do not establish OS controls, speaker output, native provider timing, registered gameplay world, created player, camera or original visual/runtime parity."
    }))
}

fn closed(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .context("session metadata object absent")?;
    ensure!(
        object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field)),
        "session metadata has missing or unexpected fields"
    );
    Ok(())
}

fn count(value: &Value, field: &str) -> Result<u64> {
    let result = value[field]
        .as_u64()
        .with_context(|| format!("session count {field} absent"))?;
    ensure!(
        result <= 9_007_199_254_740_991,
        "session count {field} exceeds public numeric range"
    );
    Ok(result)
}

fn expected_steps(check: Check) -> Vec<(&'static str, &'static str, &'static str, u64)> {
    let confirm = ("confirm", "accept", "new_game_difficulty", 0);
    match check {
        Check::DifficultyPage => vec![confirm],
        Check::Loaded {
            difficulty: 1,
            reentry: true,
        } => vec![
            confirm,
            confirm,
            ("back", "back", "main_menu", 0),
            confirm,
            ("down", "navigate", "new_game_difficulty", 1),
            ("confirm", "accept", "new_game_difficulty", 1),
        ],
        Check::Loaded { difficulty, .. } => {
            let mut steps = vec![confirm];
            for selected in 1..=difficulty {
                steps.push((
                    "down",
                    "navigate",
                    "new_game_difficulty",
                    u64::from(selected),
                ));
            }
            steps.push((
                "confirm",
                "accept",
                "new_game_difficulty",
                u64::from(difficulty),
            ));
            steps.push((
                "confirm",
                "none",
                "new_game_difficulty",
                u64::from(difficulty),
            ));
            steps
        }
        Check::ReturnedMenu => vec![confirm, confirm, ("back", "back", "main_menu", 0)],
    }
}

fn validate(probe: &Value, check: Check) -> Result<()> {
    let window = &probe["window"];
    ensure!(
        probe["reference_compared"] == false
            && window["hidden"] == true
            && window["frames_presented"] == 120
            && window["scene_uploads"] == 1
            && window["game_session_shutdown_complete"] == true,
        "game session presentation or shutdown did not complete"
    );
    let game = &window["game_session"];
    closed(game, CONTROLLER_FIELDS)?;
    for field in [
        "requested",
        "loaded",
        "cancelled",
        "failed",
        "retired_workers",
        "retired_worker_failures",
    ] {
        count(game, field)?;
    }
    ensure!(
        game["provider"] == "host_local_semantic"
            && game["native_provider_timing_verified"] == false
            && game["scope"]
                .as_str()
                .is_some_and(|text| !text.is_empty() && text.len() <= 4096)
            && game["failed"] == 0
            && game["retired_worker_failures"] == 0
            && count(game, "retired_workers")? <= count(game, "cancelled")?.min(2),
        "game session provider/failure counters invalid"
    );
    let (selected, requested, cancelled, loaded_min, loaded_max, active, unmuted) = match check {
        Check::DifficultyPage => (0, 0, 0, 0, 0, false, false),
        Check::Loaded {
            difficulty,
            reentry,
        } => {
            ensure!(
                difficulty <= 3 && (!reentry || difficulty == 1),
                "invalid gate difficulty/reentry case"
            );
            (
                u64::from(difficulty),
                if reentry { 2 } else { 1 },
                u64::from(reentry),
                1,
                if reentry { 2 } else { 1 },
                true,
                difficulty == 0 && !reentry,
            )
        }
        Check::ReturnedMenu => (0, 1, 1, 0, 1, false, false),
    };
    ensure!(
        game["requested"] == requested
            && game["cancelled"] == cancelled
            && (loaded_min..=loaded_max).contains(&count(game, "loaded")?),
        "game session request/load/cancel counters differ from shared-handler inputs"
    );
    let menu = &window["menu"];
    closed(
        menu,
        &["screen", "selected", "has_save", "settings", "parents"],
    )?;
    let returned = matches!(check, Check::ReturnedMenu);
    ensure!(
        menu["screen"]
            == if returned {
                "main_menu"
            } else {
                "new_game_difficulty"
            }
            && menu["selected"] == selected
            && menu["has_save"] == false
            && menu["parents"]
                == if returned {
                    json!([])
                } else {
                    json!([["main_menu", 0]])
                },
        "game session did not preserve the source menu/difficulty return boundary"
    );
    let session = &window["menu_session"];
    ensure!(
        session["writes"] == 0 && session["dirty"] == false,
        "New Game inputs changed preferences"
    );
    let steps = session["probe"]
        .as_array()
        .context("New Game shared-handler trace absent")?;
    let expected = expected_steps(check);
    ensure!(
        steps.len() == expected.len(),
        "New Game input trace incomplete"
    );
    for (index, (step, (input, effect, screen, selected))) in steps.iter().zip(expected).enumerate()
    {
        ensure!(
            step["frame"] == (index as u64 + 1) * 6
                && step["input"] == input
                && step["effect"] == effect
                && step["screen"] == screen
                && step["selected"] == selected
                && step["settings"] == menu["settings"],
            "New Game input trace step {index} differs"
        );
    }
    let playback = &window["playback"];
    ensure!(
        playback["stage"] == "main_menu"
            && playback["muted"] == !unmuted
            && playback["session_suspended"] == active,
        "New Game primary music suspension/return state differs"
    );
    if unmuted {
        let audio = &playback["audio"];
        ensure!(
            audio.is_object()
                && audio["primary_gain"].as_f64() == Some(0.)
                && audio["device_errors"] == 0,
            "New Game did not suspend primary music cleanly"
        );
    } else {
        ensure!(
            playback["audio"].is_null() && playback["menu_audio"].is_null(),
            "muted game probe opened audio output"
        );
    }
    if active {
        let snapshot = &game["active"];
        closed(snapshot, SNAPSHOT_FIELDS)?;
        ensure!(
            snapshot["difficulty"] == selected
                && snapshot["level_hash"] == 0x0181_016bu64
                && snapshot["start_point_id"] == 1
                && snapshot["session_active"] == true
                && snapshot["local_users"] == 1
                && snapshot["mode"] == 3
                && snapshot["backend"] == 2
                && snapshot["source_create_counter"] == 7
                && snapshot["created_session_selected"] == true
                && snapshot["failure"].is_null(),
            "New Game semantic session admission/request metadata differs"
        );
        for field in ["resources_retained", "map_instances", "source_start_points"] {
            ensure!(
                count(snapshot, field)? > 0,
                "New Game retained resource count {field} absent"
            );
        }
        ensure!(
            count(snapshot, "map_instances")? <= count(snapshot, "resources_retained")?
                && ["player_created", "camera_ready"]
                    .iter()
                    .all(|field| snapshot[*field] == false),
            "New Game retained CPU resources falsely promote live world/player/camera readiness"
        );
        validate_world(snapshot)?;
        let world = &snapshot["world_initialization"];
        let complete_loader = world
            .get("loader")
            .is_some_and(|loader| loader["operation_complete"] == true);
        ensure!(
            if complete_loader {
                snapshot["stage"] == "awaiting_player"
                    && snapshot["world_registered"] == true
                    && snapshot["manager_ready_flags_raw"] == 0x108
                    && snapshot["user_ready_flags_raw"] == 0x108
            } else {
                snapshot["stage"] == "awaiting_world"
                    && snapshot["world_registered"] == false
                    && snapshot["manager_ready_flags_raw"] == 0
                    && snapshot["user_ready_flags_raw"] == 0
            },
            "session stage/readiness is not backed by its actual completed loader"
        );
    } else {
        ensure!(
            game["active"].is_null(),
            "returned/unconfirmed menu retained an active session"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_fixture() -> Value {
        // One of the two Active91 objects has a disabled header: kind count
        // is not active membership. Source-kind totals remain authored here.
        let mut world = json!({"maps_activated":2,"original_slots":41,"disabled_slots":1,"filtered_slots":3,
            "unclassified_slots":4,"unsupported_factory_slots":1,"unsupported_shape_slots":0,
            "owned_triggers":30,"common_scratch":30,"membership_entries":30,"active_triggers":3,
            "start_points":2,"generic_triggers":2,"common_be_triggers":1,"staged_triggers":0,
            "resource08_triggers":1,"active91_triggers":2,"range0c_triggers":1,"radius06_triggers":1,
            "stopped_animators":27,"blocked_references":1,"resolved_references":4,
            "animators_initialized":2,"animators_failed":0,"shared_variables":8,"unwritten_scratch_words":4,
            "state_flags":4,"execution_slots":32,"active_executions":0,"create_events_denied":2,
            "registration_events_unmapped":2,"zone_queries":41,"catalog_entries":0,
            "catalog_complete":false,"native_zone_precision_verified":false,"state_sha256":"a".repeat(64),
            "shape_selectors_initialized":1,"actor_definitions":actor_fixture(),
            "scope":"Authored ordinal/owned-animator fixture"});
        world["actor12_triggers"] = json!(2);
        world["range82_triggers"] = json!(1);
        world["a3_triggers"] = json!(2);
        world["range27_triggers"] = json!(1);
        world["common09_triggers"] = json!(2);
        world["ad_triggers"] = json!(1);
        world["radius03_triggers"] = json!(1);
        world["common8a_triggers"] = json!(2);
        world["radius1b_triggers"] = json!(2);
        world["radius74_triggers"] = json!(1);
        world["shape71_triggers"] = json!(2);
        world["materialized_shapes"] = json!(1);
        world["radius88_triggers"] = json!(1);
        world["auxac_triggers"] = json!(2);
        world["source_null_slots"] = json!(2);
        world["map_initialization_blocked"] = json!(0);
        world["resource_states"] = resource_states_fixture();
        world["navigation"] = Value::Null;
        world["post_trigger_resources"] = Value::Null;
        world["world_hooks"] = Value::Null;
        world["loader"] = Value::Null;
        world["player_initialization"] = Value::Null;
        world
    }

    fn resource_states_fixture() -> Value {
        json!({"origin":"supplied_snapshot","lifetime":"native_window_controller","shared_owner":true,
            "records":2,"capacity":4,"nonzero_values":1,"reset_limit":333,"lookups":1,"hits":1,"misses":0,
            "snapshot_bound":false,"original_history_verified":false,"state_sha256":"e".repeat(64),
            "scope":"Authored supplied resource-state owner; no captured buffer or original history"})
    }

    fn complete_fixture(check: Check) -> Value {
        let mut probe = fixture(check);
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        let removed = world["unclassified_slots"].as_u64().unwrap()
            + world["unsupported_factory_slots"].as_u64().unwrap();
        world["original_slots"] = json!(world["original_slots"].as_u64().unwrap() - removed);
        world["zone_queries"] = world["original_slots"].clone();
        world["unclassified_slots"] = json!(0);
        world["unsupported_factory_slots"] = json!(0);
        world["blocked_references"] = json!(0);
        world["materialized_shapes"] = world["shape71_triggers"].clone();
        world["catalog_entries"] = world["start_points"].clone();
        world["catalog_complete"] = json!(true);
        probe
    }

    fn actor_fixture() -> Value {
        json!({"resource_hash":0x01090013,"spreadsheet_hash":0x14000021,"payload_bytes":640,
            "primary_rows":6,"secondary_rows":3,"registered_primary":5,"duplicate_primary":1,
            "runtime_groups":2,"grouped_members":4,"group8_members":2,"selection_calls":2,
            "resource_shared":true,"primary_present":true,"secondary_present":true,"fresh_cursors_zero":false,
            "default_record_bound":false,"special_records_bound":false,"original_cursor_history_verified":false,
            "rng_bound":true,"native9_constructor_ready":true,"state_sha256":"c".repeat(64),
            "rng_algorithm":"source_two_step_lcg32","rng_origin":"fresh_constructor_snapshot",
            "rng_draws":1,"rng_state_sha256":"d".repeat(64),"original_rng_history_verified":false,
            "scope":"Authored mutable actor definition snapshot; original cursor/RNG history unavailable"})
    }

    fn fixture(check: Check) -> Value {
        let (requested, cancelled, loaded, active, difficulty) = match check {
            Check::DifficultyPage => (0, 0, 0, false, 0),
            Check::Loaded {
                difficulty,
                reentry,
            } => (
                if reentry { 2 } else { 1 },
                u64::from(reentry),
                1,
                true,
                difficulty,
            ),
            Check::ReturnedMenu => (1, 1, 0, false, 0),
        };
        let settings =
            json!({"music_volume":62,"sound_volume":96,"dialog_volume":100,"brightness":100});
        let unmuted = matches!(
            check,
            Check::Loaded {
                difficulty: 0,
                reentry: false
            }
        );
        let steps = expected_steps(check).into_iter().enumerate().map(|(index, (input,effect,screen,selected))| json!({
            "frame":(index as u64+1)*6,"input":input,"effect":effect,"screen":screen,"selected":selected,"settings":settings,
        })).collect::<Vec<_>>();
        let snapshot = active.then(|| json!({
            "stage":"awaiting_world","difficulty":difficulty,"level_hash":0x0181016b,"start_point_id":1,
            "session_active":true,"local_users":1,"resources_retained":3,"map_instances":2,"source_start_points":2,
            "world_registered":false,"player_created":false,"camera_ready":false,"mode":3,"backend":2,
            "manager_ready_flags_raw":0,"user_ready_flags_raw":0,"source_create_counter":7,
            "created_session_selected":true,"failure":null,"world_initialization":world_fixture(),
        }));
        json!({"reference_compared":false,"window":{
            "hidden":true,"frames_presented":120,"scene_uploads":1,"game_session_shutdown_complete":true,
            "menu":{"screen":if matches!(check,Check::ReturnedMenu){"main_menu"}else{"new_game_difficulty"},
                "selected":difficulty,"has_save":false,"settings":settings,
                "parents":if matches!(check,Check::ReturnedMenu){json!([])}else{json!([["main_menu",0]])}},
            "menu_session":{"writes":0,"dirty":false,"probe":steps},
            "playback":{"stage":"main_menu","muted":!unmuted,"session_suspended":active,
                "audio":unmuted.then(||json!({"primary_gain":0.0,"device_errors":0})),"menu_audio":null},
            "game_session":{"requested":requested,"loaded":loaded,"cancelled":cancelled,"failed":0,
                "retired_workers":0,"retired_worker_failures":0,"active":snapshot,
                "provider":"host_local_semantic","native_provider_timing_verified":false,"scope":"Authored host fixture"},
        }})
    }

    #[test]
    fn admitted_difficulties_and_both_cancel_timings_preserve_cpu_checkpoint_scope() {
        validate(&fixture(Check::DifficultyPage), Check::DifficultyPage).unwrap();
        for difficulty in 0..=3 {
            let check = Check::Loaded {
                difficulty,
                reentry: false,
            };
            validate(&fixture(check), check).unwrap();
        }
        let reentry = Check::Loaded {
            difficulty: 1,
            reentry: true,
        };
        let mut value = fixture(reentry);
        validate(&value, reentry).unwrap();
        value["window"]["game_session"]["loaded"] = json!(2);
        validate(&value, reentry).unwrap();
        let mut returned = fixture(Check::ReturnedMenu);
        validate(&returned, Check::ReturnedMenu).unwrap();
        returned["window"]["game_session"]["loaded"] = json!(1);
        validate(&returned, Check::ReturnedMenu).unwrap();
    }

    #[test]
    fn source_flags_readiness_private_fields_and_missing_resource_ownership_fail_closed() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        for (field, content) in [
            ("level_hash", json!(0x0181013f)),
            ("start_point_id", json!(0)),
            ("local_users", json!(8)),
            ("source_create_counter", json!(8)),
            ("manager_ready_flags_raw", json!(0x100)),
            ("user_ready_flags_raw", json!(0x100)),
            ("mode", json!(2)),
            ("backend", json!(3)),
            ("created_session_selected", json!(false)),
            ("session_active", json!(false)),
            ("resources_retained", json!(0)),
            ("map_instances", json!(4)),
            ("source_start_points", json!(0)),
            ("world_registered", json!(true)),
            ("player_created", json!(true)),
            ("camera_ready", json!(true)),
            ("failure", json!("loading")),
            ("stage", json!("loading")),
            ("native_pointer", json!(0x805d5460u64)),
            ("translation_bits", json!([1, 2, 3, 4])),
        ] {
            let mut value = fixture(check);
            value["window"]["game_session"]["active"][field] = content;
            assert!(validate(&value, check).is_err(), "{field}");
        }
        for (field, content) in [
            ("native_provider_timing_verified", json!(true)),
            ("failed", json!(1)),
            ("retired_worker_failures", json!(1)),
            ("provider", json!("native")),
            ("retired_workers", json!(1)),
            ("native_pointer", json!(0x805d5460u64)),
        ] {
            let mut value = fixture(check);
            value["window"]["game_session"][field] = content;
            assert!(validate(&value, check).is_err(), "{field}");
        }
        let mut value = fixture(check);
        value["window"]["playback"]["audio"]["primary_gain"] = json!(0.5);
        assert!(validate(&value, check).is_err());
        value = fixture(check);
        value["window"]["playback"]["audio"]["device_errors"] = json!(1);
        assert!(validate(&value, check).is_err());
        value = fixture(check);
        value["window"]["game_session_shutdown_complete"] = json!(false);
        assert!(validate(&value, check).is_err());
    }

    #[test]
    fn repeat_dispatch_cancel_return_and_trace_contradictions_cannot_pass() {
        let check = Check::Loaded {
            difficulty: 1,
            reentry: false,
        };
        let mut repeated = fixture(check);
        repeated["window"]["game_session"]["requested"] = json!(2);
        assert!(validate(&repeated, check).is_err());
        repeated = fixture(check);
        repeated["window"]["menu_session"]["probe"][3]["effect"] = json!("accept");
        assert!(validate(&repeated, check).is_err());
        let reentry = Check::Loaded {
            difficulty: 1,
            reentry: true,
        };
        for (field, value) in [
            ("requested", json!(1)),
            ("cancelled", json!(0)),
            ("loaded", json!(0)),
        ] {
            let mut context = fixture(reentry);
            context["window"]["game_session"][field] = value;
            assert!(validate(&context, reentry).is_err());
        }
        let mut returned = fixture(Check::ReturnedMenu);
        returned["window"]["playback"]["session_suspended"] = json!(true);
        assert!(validate(&returned, Check::ReturnedMenu).is_err());
        returned = fixture(Check::ReturnedMenu);
        returned["window"]["menu"]["screen"] = json!("new_game_difficulty");
        assert!(validate(&returned, Check::ReturnedMenu).is_err());
        let mut page = fixture(Check::DifficultyPage);
        page["window"]["game_session"]["requested"] = json!(1);
        assert!(validate(&page, Check::DifficultyPage).is_err());
    }

    #[test]
    fn source_null_slots_complete_catalog_passes_without_creating_objects_or_ready_world() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        let probe = complete_fixture(check);
        validate(&probe, check).unwrap();
        let snapshot = &probe["window"]["game_session"]["active"];
        let world = &snapshot["world_initialization"];
        assert_eq!(world["source_null_slots"], 2);
        assert_eq!(world["owned_triggers"], 30);
        assert_eq!(world["membership_entries"], 30);
        assert_eq!(world["stopped_animators"], 27);
        assert_eq!(world["catalog_entries"], 2);
        for field in ["world_registered", "player_created", "camera_ready"] {
            assert_eq!(snapshot[field], false);
            let mut promoted = probe.clone();
            promoted["window"]["game_session"]["active"][field] = json!(true);
            assert!(validate(&promoted, check).is_err(), "{field}");
        }
        let mut conservative = probe.clone();
        conservative["window"]["game_session"]["active"]["world_initialization"]["catalog_complete"] =
            json!(false);
        conservative["window"]["game_session"]["active"]["world_initialization"]["catalog_entries"] =
            json!(0);
        validate(&conservative, check).unwrap();
    }

    #[test]
    fn completed_catalog_rejects_unfinished_dependencies_and_partial_or_stale_entries() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        for field in [
            "unclassified_slots",
            "unsupported_factory_slots",
            "unsupported_shape_slots",
        ] {
            let mut probe = complete_fixture(check);
            let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
            world[field] = json!(1);
            world["original_slots"] = json!(world["original_slots"].as_u64().unwrap() + 1);
            world["zone_queries"] = world["original_slots"].clone();
            assert!(validate(&probe, check).is_err(), "{field}");
        }
        for (field, value) in [
            ("blocked_references", json!(1)),
            ("materialized_shapes", json!(1)),
            ("catalog_entries", json!(1)),
            ("catalog_complete", json!(false)),
            ("catalog_complete", json!("true")),
            ("source_null_slots", json!(-1)),
        ] {
            let mut probe = complete_fixture(check);
            probe["window"]["game_session"]["active"]["world_initialization"][field] = value;
            assert!(validate(&probe, check).is_err(), "{field}");
        }
        let mut missing = complete_fixture(check);
        missing["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("source_null_slots");
        assert!(validate(&missing, check).is_err());
    }

    #[test]
    fn world_ordinal_ownership_digest_and_readiness_contradictions_fail_closed() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        for (field, value) in [
            ("maps_activated", json!(1)),
            ("original_slots", json!(18)),
            ("zone_queries", json!(18)),
            ("common_scratch", json!(2)),
            ("membership_entries", json!(2)),
            ("active_triggers", json!(11)),
            ("start_points", json!(3)),
            ("generic_triggers", json!(3)),
            ("common_be_triggers", json!(2)),
            ("resource08_triggers", json!(2)),
            ("active91_triggers", json!(3)),
            ("range0c_triggers", json!(2)),
            ("radius06_triggers", json!(2)),
            ("actor12_triggers", json!(3)),
            ("range82_triggers", json!(2)),
            ("a3_triggers", json!(3)),
            ("a3_triggers", json!(-1)),
            ("range27_triggers", json!(2)),
            ("common09_triggers", json!(3)),
            ("common09_triggers", json!(-1)),
            ("ad_triggers", json!(2)),
            ("radius03_triggers", json!(2)),
            ("common8a_triggers", json!(3)),
            ("radius1b_triggers", json!(3)),
            ("radius74_triggers", json!(2)),
            ("shape71_triggers", json!(3)),
            ("materialized_shapes", json!(3)),
            ("materialized_shapes", json!(-1)),
            ("radius88_triggers", json!(2)),
            ("auxac_triggers", json!(3)),
            ("source_null_slots", json!(-1)),
            ("source_null_slots", json!(3)),
            ("map_initialization_blocked", json!(3)),
            ("shape_selectors_initialized", json!(31)),
            ("staged_triggers", json!(6)),
            ("stopped_animators", json!(2)),
            ("blocked_references", json!(241)),
            ("resolved_references", json!(240)),
            ("animators_failed", json!(1)),
            ("shared_variables", json!(-1)),
            ("unwritten_scratch_words", json!(0)),
            ("state_flags", json!(-1)),
            ("execution_slots", json!(31)),
            ("active_executions", json!(1)),
            ("create_events_denied", json!(1)),
            ("registration_events_unmapped", json!(1)),
            ("catalog_entries", json!(3)),
            ("catalog_complete", json!(true)),
            ("native_zone_precision_verified", json!(true)),
            ("state_sha256", json!("A".repeat(64))),
            ("state_sha256", json!("short")),
            ("original_properties", json!([1, 2, 3])),
        ] {
            let mut probe = fixture(check);
            probe["window"]["game_session"]["active"]["world_initialization"][field] = value;
            assert!(validate(&probe, check).is_err(), "{field}");
        }
        let mut probe = fixture(check);
        probe["window"]["game_session"]["active"]["world_initialization"] = Value::Null;
        assert!(validate(&probe, check).is_err());
        probe["window"]["game_session"]["active"]
            .as_object_mut()
            .unwrap()
            .remove("world_initialization");
        assert!(validate(&probe, check).is_err());
    }

    #[test]
    fn staged_objects_remain_outside_membership_and_count_as_factory_blockers() {
        let check = Check::Loaded {
            difficulty: 1,
            reentry: false,
        };
        let mut probe = fixture(check);
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        // Staging three objects exceeds the two Generic objects: resource and
        // range/radius kinds also participate in the checked staging budget.
        world["staged_triggers"] = json!(3);
        world["membership_entries"] = json!(27);
        world["stopped_animators"] = json!(24);
        world["unsupported_factory_slots"] = json!(4);
        validate(&probe, check).unwrap();
        assert!(
            validate_tutorial_world(&probe).is_err(),
            "authored counts are not the source Tutorial corpus"
        );
        probe["window"]["game_session"]["active"]["world_initialization"]["unsupported_factory_slots"] =
            json!(0);
        assert!(validate(&probe, check).is_err());
    }

    #[test]
    fn provider_and_geometry_staging_excludes_provider_free_kinds() {
        let check = Check::Loaded {
            difficulty: 1,
            reentry: false,
        };
        let mut probe = fixture(check);
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        // Fourteen objects can stage on providers or required geometry.
        // Radius74 and the other provider-free kinds do not add capacity.
        world["staged_triggers"] = json!(14);
        world["membership_entries"] = json!(16);
        world["stopped_animators"] = json!(13);
        world["unsupported_factory_slots"] = json!(15);
        world["materialized_shapes"] = json!(0);
        world["actor_definitions"]["native9_constructor_ready"] = json!(false);
        world["resource_states"] = Value::Null;
        validate(&probe, check).unwrap();
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        world["staged_triggers"] = json!(15);
        world["membership_entries"] = json!(15);
        world["stopped_animators"] = json!(12);
        world["unsupported_factory_slots"] = json!(16);
        assert!(validate(&probe, check).is_err());
        let mut missing = fixture(check);
        missing["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("range27_triggers");
        assert!(validate(&missing, check).is_err());
        let mut missing_ad = fixture(check);
        missing_ad["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("ad_triggers");
        assert!(validate(&missing_ad, check).is_err());
    }

    #[test]
    fn resolved_references_do_not_replace_remaining_constructor_or_readiness_gates() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        let mut probe = fixture(check);
        let snapshot = &mut probe["window"]["game_session"]["active"];
        snapshot["world_initialization"]["blocked_references"] = json!(0);
        validate(&probe, check).unwrap();
        probe["window"]["game_session"]["active"]["world_initialization"]["catalog_complete"] =
            json!(true);
        assert!(validate(&probe, check).is_err());
        probe["window"]["game_session"]["active"]["world_initialization"]["catalog_complete"] =
            json!(false);
        probe["window"]["game_session"]["active"]["world_registered"] = json!(true);
        assert!(validate(&probe, check).is_err());
        let mut missing = fixture(check);
        missing["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("radius1b_triggers");
        assert!(validate(&missing, check).is_err());
    }

    #[test]
    fn initialized_shape_tags_are_separate_from_selector_bindings_and_later_stage_completion() {
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        let mut probe = fixture(check);
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        world["shape_selectors_initialized"] = json!(0);
        validate(&probe, check).unwrap();
        // Initialized payload tags may survive a later unsupported transform
        // stage; no invented materialized-versus-staged exclusion is valid.
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        world["materialized_shapes"] = json!(2);
        world["staged_triggers"] = json!(1);
        world["membership_entries"] = json!(29);
        world["stopped_animators"] = json!(26);
        world["unsupported_factory_slots"] = json!(2);
        validate(&probe, check).unwrap();
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("materialized_shapes");
        assert!(validate(&probe, check).is_err());
    }

    #[test]
    fn resource_states_validate_owned_snapshots_without_exporting_original_entries() {
        for (field, value) in [
            ("origin", json!("original_runtime")),
            ("lifetime", json!("mission_worker")),
            ("shared_owner", json!(false)),
            ("records", json!(5)),
            ("capacity", json!(1)),
            ("nonzero_values", json!(3)),
            ("reset_limit", json!(0x1_0000_0000u64)),
            ("hits", json!(2)),
            ("misses", json!(1)),
            ("snapshot_bound", json!("false")),
            ("original_history_verified", json!(true)),
            ("state_sha256", json!("E".repeat(64))),
            ("scope", json!("")),
            ("keys", json!([1, 2])),
            ("values", json!([0, 1])),
        ] {
            let mut states = resource_states_fixture();
            states[field] = value;
            assert!(validate_resource_states(&states, 1).is_err(), "{field}");
        }
        let mut excessive = resource_states_fixture();
        excessive["lookups"] = json!(2);
        excessive["misses"] = json!(1);
        assert!(validate_resource_states(&excessive, 1).is_err());
        let mut missing = resource_states_fixture();
        missing.as_object_mut().unwrap().remove("reset_limit");
        assert!(validate_resource_states(&missing, 1).is_err());
        let mut empty_hit = resource_states_fixture();
        empty_hit["records"] = json!(0);
        empty_hit["capacity"] = json!(0);
        empty_hit["nonzero_values"] = json!(0);
        assert!(validate_resource_states(&empty_hit, 1).is_err());
    }

    #[test]
    fn resource_lookup_miss_is_distinct_from_missing_provider_and_capture_is_independent() {
        let mut states = resource_states_fixture();
        // Supplied owner origin does not require a separate capture buffer.
        validate_resource_states(&states, 1).unwrap();
        states["origin"] = json!("fresh_constructor_snapshot");
        states["records"] = json!(0);
        states["capacity"] = json!(0);
        states["nonzero_values"] = json!(0);
        states["hits"] = json!(0);
        states["misses"] = json!(1);
        states["snapshot_bound"] = json!(true);
        validate_resource_states(&states, 1).unwrap();
        validate_resource_states(&Value::Null, 1).unwrap();
        let check = Check::Loaded {
            difficulty: 0,
            reentry: false,
        };
        let mut probe = fixture(check);
        probe["window"]["game_session"]["active"]["world_initialization"]["resource_states"] =
            Value::Null;
        validate(&probe, check).unwrap();
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("resource_states");
        assert!(validate(&probe, check).is_err());
    }

    #[test]
    fn actor_definitions_reject_unsafe_counts_cursor_claims_and_original_exports() {
        for (field, value) in [
            ("resource_hash", json!(1)),
            ("spreadsheet_hash", json!(2)),
            ("primary_rows", json!(7)),
            ("registered_primary", json!(4)),
            ("duplicate_primary", json!(0)),
            ("runtime_groups", json!(5)),
            ("grouped_members", json!(6)),
            ("group8_members", json!(5)),
            ("primary_present", json!(false)),
            ("secondary_present", json!(false)),
            ("resource_shared", json!(false)),
            ("selection_calls", json!(0)),
            ("fresh_cursors_zero", json!(true)),
            ("default_record_bound", json!(true)),
            ("special_records_bound", json!(true)),
            ("original_cursor_history_verified", json!(true)),
            ("rng_bound", json!(false)),
            ("rng_draws", json!(3)),
            ("rng_algorithm", json!("host_random")),
            ("rng_origin", json!("original_runtime")),
            ("rng_state_sha256", json!("D".repeat(64))),
            ("original_rng_history_verified", json!(true)),
            ("state_sha256", json!("C".repeat(64))),
            ("scope", json!("")),
            ("rows", json!([[1, 2, 3]])),
        ] {
            let mut actor = actor_fixture();
            actor[field] = value;
            assert!(validate_actor_definitions(&actor).is_err(), "{field}");
        }
        validate_actor_definitions(&Value::Null).unwrap();
        let mut empty = actor_fixture();
        for field in [
            "primary_rows",
            "registered_primary",
            "duplicate_primary",
            "runtime_groups",
            "grouped_members",
            "group8_members",
            "secondary_rows",
            "selection_calls",
            "rng_draws",
        ] {
            empty[field] = json!(0);
        }
        empty["primary_present"] = json!(false);
        empty["secondary_present"] = json!(false);
        empty["rng_bound"] = json!(false);
        empty["native9_constructor_ready"] = json!(false);
        empty["fresh_cursors_zero"] = json!(true);
        validate_actor_definitions(&empty).unwrap();
        empty["primary_present"] = json!(true);
        empty["secondary_present"] = json!(true);
        validate_actor_definitions(&empty).unwrap();
        let mut missing = actor_fixture();
        missing.as_object_mut().unwrap().remove("secondary_rows");
        assert!(validate_actor_definitions(&missing).is_err());
    }

    #[test]
    fn blocked_actor_map_initialization_retains_partial_state_without_promoting_ready() {
        let mut probe = fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        let snapshot = &mut probe["window"]["game_session"]["active"];
        snapshot["world_initialization"]["map_initialization_blocked"] = json!(1);
        assert!(
            validate_world(snapshot).is_err(),
            "ready actor flag contradicts blocked map callback"
        );
        snapshot["world_initialization"]["actor_definitions"]["native9_constructor_ready"] =
            json!(false);
        validate_world(snapshot).unwrap();
        assert_eq!(snapshot["world_registered"], false);
        assert_eq!(snapshot["player_created"], false);
        assert_eq!(snapshot["camera_ready"], false);
        snapshot["world_initialization"]["map_initialization_blocked"] = json!(3);
        assert!(validate_world(snapshot).is_err());
    }

    fn navigation_fixture() -> Value {
        json!({"origin":"fresh_owner_snapshot","initialized":true,
            "source_placements":5,"eligible_placements":2,"owned_wrappers":2,"bound_wrappers":2,
            "initialized_caches":2,"vertices":9,"faces":7,"category_slots":30,
            "nonempty_categories":2,"category_members":3,"boundary_pairs":8,
            "bindings_sha256":"1".repeat(64),"categories_sha256":"2".repeat(64),
            "cache_sha256":"3".repeat(64),"state_sha256":"4".repeat(64),
            "original_history_verified":false,"native_precision_verified":false,"queries_executed":false,
            "scope":"Authored retained navigation owner and integer cache, no live queries"})
    }

    #[test]
    fn navigation_requires_owned_bindings_and_closed_private_evidence() {
        validate_navigation(&navigation_fixture()).unwrap();
        validate_navigation(&Value::Null).unwrap();
        for (field, value) in [
            ("source_placements", json!(1)),
            ("bound_wrappers", json!(1)),
            ("initialized_caches", json!(1)),
            ("category_slots", json!(29)),
            ("category_members", json!(8)),
            ("nonempty_categories", json!(0)),
            ("boundary_pairs", json!(22)),
            ("vertices", json!(-1)),
            ("initialized", json!("true")),
            ("origin", json!("original_runtime")),
            ("bindings_sha256", json!("X".repeat(64))),
            ("categories_sha256", Value::Null),
            ("original_history_verified", json!(true)),
            ("native_precision_verified", json!(true)),
            ("queries_executed", json!(true)),
            ("scope", json!("")),
            ("positions", json!([[1, 2, 3]])),
        ] {
            let mut navigation = navigation_fixture();
            navigation[field] = value;
            assert!(validate_navigation(&navigation).is_err(), "{field}");
        }
        let mut missing = navigation_fixture();
        missing.as_object_mut().unwrap().remove("cache_sha256");
        assert!(validate_navigation(&missing).is_err());
    }

    #[test]
    fn navigation_partial_and_empty_owners_do_not_require_dummy_caches() {
        let mut partial = navigation_fixture();
        partial["initialized"] = json!(false);
        partial["bound_wrappers"] = json!(1);
        partial["initialized_caches"] = json!(0);
        validate_navigation(&partial).unwrap();
        partial["initialized"] = json!(true);
        assert!(validate_navigation(&partial).is_err());
        let mut empty = navigation_fixture();
        for field in NAVIGATION_COUNTS {
            empty[*field] = json!(0);
        }
        validate_navigation(&empty).unwrap();
        empty["eligible_placements"] = json!(1);
        empty["source_placements"] = json!(1);
        assert!(validate_navigation(&empty).is_err());
        empty["initialized"] = json!(false);
        validate_navigation(&empty).unwrap();
    }

    #[test]
    fn navigation_completion_never_promotes_session_readiness() {
        let mut probe = fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        probe["window"]["game_session"]["active"]["world_initialization"]["navigation"] =
            navigation_fixture();
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        for field in ["world_registered", "player_created", "camera_ready"] {
            let mut promoted = probe.clone();
            promoted["window"]["game_session"]["active"][field] = json!(true);
            assert!(
                validate(
                    &promoted,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("navigation");
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }

    fn post_trigger_fixture() -> Value {
        json!({"origin":"source_hook_snapshot","provider":"synchronous_supplied_data_residency",
            "operation_complete":true,"selection_performed":true,"selected_definition_bound":true,
            "ordered_requests":2,"resolved_requests":2,"retained_files":1,"retained_sections":1,
            "retained_file_bytes":960,"retained_section_bytes":240,"tail_reset_applied":true,
            "requests_sha256":"5".repeat(64),"residency_sha256":"6".repeat(64),"state_sha256":"7".repeat(64),
            "original_cursor_history_verified":false,"original_rng_history_verified":false,
            "original_stream_history_verified":false,"native_stream_timing_verified":false,
            "native_stream_readiness_verified":false,"scope":"Authored shared-file ordered request residency"})
    }

    #[test]
    fn post_trigger_receipts_reject_unsafe_fields_and_completion_contradictions() {
        validate_post_trigger_resources(&post_trigger_fixture()).unwrap();
        validate_post_trigger_resources(&Value::Null).unwrap();
        for (field, value) in [
            ("ordered_requests", json!(3)),
            ("resolved_requests", json!(1)),
            ("retained_files", json!(3)),
            ("retained_sections", json!(3)),
            ("retained_sections", json!(0)),
            ("retained_file_bytes", json!(-1)),
            ("retained_section_bytes", json!(961)),
            ("selection_performed", json!(false)),
            ("selected_definition_bound", json!(false)),
            ("operation_complete", json!("true")),
            ("tail_reset_applied", json!(false)),
            ("provider", json!("native_streaming")),
            ("origin", json!("original_runtime")),
            ("requests_sha256", json!("Q".repeat(64))),
            ("residency_sha256", Value::Null),
            ("original_cursor_history_verified", json!(true)),
            ("original_rng_history_verified", json!(true)),
            ("original_stream_history_verified", json!(true)),
            ("native_stream_timing_verified", json!(true)),
            ("native_stream_readiness_verified", json!(true)),
            ("scope", json!("")),
            ("section_payload", json!([1, 2, 3])),
        ] {
            let mut receipt = post_trigger_fixture();
            receipt[field] = value;
            assert!(
                validate_post_trigger_resources(&receipt).is_err(),
                "{field}"
            );
        }
        let mut missing = post_trigger_fixture();
        missing.as_object_mut().unwrap().remove("retained_sections");
        assert!(validate_post_trigger_resources(&missing).is_err());
    }

    #[test]
    fn post_trigger_partial_miss_and_base_sections_are_distinct_from_completed_skip() {
        let mut partial = post_trigger_fixture();
        partial["operation_complete"] = json!(false);
        partial["resolved_requests"] = json!(0);
        partial["retained_sections"] = json!(0);
        partial["retained_section_bytes"] = json!(0);
        partial["tail_reset_applied"] = json!(false);
        validate_post_trigger_resources(&partial).unwrap(); // Base owner retained even with target miss.
        partial["operation_complete"] = json!(true);
        assert!(validate_post_trigger_resources(&partial).is_err());
        let mut base = post_trigger_fixture();
        base["retained_sections"] = json!(0);
        base["retained_section_bytes"] = json!(0);
        validate_post_trigger_resources(&base).unwrap(); // Base targets have no streamed sections.
        let mut empty_span = post_trigger_fixture();
        empty_span["retained_section_bytes"] = json!(0);
        validate_post_trigger_resources(&empty_span).unwrap(); // A checked nonbase span can be empty.
        // Both targets can resolve to resident base section0.
        let mut skip = post_trigger_fixture();
        skip["selection_performed"] = json!(false);
        skip["selected_definition_bound"] = json!(false);
        for field in POST_TRIGGER_COUNTS {
            skip[*field] = json!(0);
        }
        validate_post_trigger_resources(&skip).unwrap();
        skip["selection_performed"] = json!(true);
        assert!(validate_post_trigger_resources(&skip).is_err()); // Missing definition is not source skip.
    }

    #[test]
    fn post_trigger_residency_does_not_promote_world_player_or_camera() {
        let mut probe = fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        probe["window"]["game_session"]["active"]["world_initialization"]["post_trigger_resources"] =
            post_trigger_fixture();
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        for field in ["world_registered", "player_created", "camera_ready"] {
            let mut promoted = probe.clone();
            promoted["window"]["game_session"]["active"][field] = json!(true);
            assert!(
                validate(
                    &promoted,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("post_trigger_resources");
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }

    fn world_hook_fixture() -> Value {
        json!({"origin":"fresh_constructor_snapshot","lifetime":"native_window_controller",
            "clock_provider":"host_frame_elapsed_f32","operation_complete":true,"clock_reset_complete":true,
            "clock_reads":16,"constructor_clock_reads":16,"completed_clock_resets":4,"clock_frame_updates":9,
            "clock_seconds_bits":1.25f32.to_bits(),"clock_timescale_bits":1.0f32.to_bits(),"clock_snapshot_bound":true,
            "scoped_buffers":24,"buffers_cleared":22,"buffers_preserved":2,"backings_released":2,
            "bytes_released":48,"retained_backing_bytes":32,"leaf_reset_complete":true,"literal_noops_reached":2,
            "binding_released":true,"callback_installed":true,"callback_invocations":0,
            "clock_state_sha256":"8".repeat(64),"final_state_sha256":"9".repeat(64),"state_sha256":"a".repeat(64),
            "original_history_verified":false,"native_clock_precision_verified":false,"native_timing_verified":false,
            "world_ready_emitted":false,"scope":"Authored persistent host clock and owned buffer resets"})
    }

    #[test]
    fn world_hooks_validate_clock_snapshots_buffers_and_private_scope() {
        validate_world_hooks(&world_hook_fixture()).unwrap();
        validate_world_hooks(&Value::Null).unwrap();
        for (field, value) in [
            ("clock_provider", json!("mission_zero")),
            ("lifetime", json!("mission_worker")),
            ("clock_reads", json!(15)),
            ("constructor_clock_reads", json!(0)),
            ("completed_clock_resets", json!(1)),
            ("clock_snapshot_bound", json!(false)),
            ("clock_reset_complete", json!(false)),
            ("clock_seconds_bits", json!(f32::NAN.to_bits())),
            ("clock_seconds_bits", json!(f32::INFINITY.to_bits())),
            ("clock_seconds_bits", json!((-1.0f32).to_bits())),
            ("clock_seconds_bits", json!(u64::from(u32::MAX) + 1)),
            ("clock_timescale_bits", json!(0)),
            ("buffers_cleared", json!(21)),
            ("buffers_preserved", json!(3)),
            ("scoped_buffers", json!(23)),
            ("backings_released", json!(23)),
            ("leaf_reset_complete", json!(false)),
            ("literal_noops_reached", json!(1)),
            ("callback_installed", json!(false)),
            ("callback_invocations", json!(1)),
            ("clock_state_sha256", json!("X".repeat(64))),
            ("final_state_sha256", Value::Null),
            ("original_history_verified", json!(true)),
            ("native_clock_precision_verified", json!(true)),
            ("native_timing_verified", json!(true)),
            ("world_ready_emitted", json!(true)),
            ("buffer_payload", json!([1, 2, 3])),
            ("operation_complete", json!("true")),
        ] {
            let mut hooks = world_hook_fixture();
            hooks[field] = value;
            assert!(validate_world_hooks(&hooks).is_err(), "{field}");
        }
        let mut zero_releases = world_hook_fixture();
        zero_releases["backings_released"] = json!(0);
        assert!(validate_world_hooks(&zero_releases).is_err());
        zero_releases["bytes_released"] = json!(0);
        validate_world_hooks(&zero_releases).unwrap();
        let mut missing = world_hook_fixture();
        missing
            .as_object_mut()
            .unwrap()
            .remove("constructor_clock_reads");
        assert!(validate_world_hooks(&missing).is_err());
    }

    #[test]
    fn world_hook_partial_state_and_dynamic_clock_receipts_are_not_native_readiness() {
        let mut partial = world_hook_fixture();
        partial["operation_complete"] = json!(false);
        partial["clock_reset_complete"] = json!(false);
        partial["clock_snapshot_bound"] = json!(false);
        partial["clock_reads"] = json!(3);
        partial["completed_clock_resets"] = json!(1);
        partial["leaf_reset_complete"] = json!(false);
        partial["callback_installed"] = json!(false);
        validate_world_hooks(&partial).unwrap();
        for (seconds, updates, resets) in [(0.0f32, 1, 2), (12.5f32, 89, 5)] {
            let mut clock = world_hook_fixture();
            clock["clock_seconds_bits"] = json!(seconds.to_bits());
            clock["clock_frame_updates"] = json!(updates);
            clock["completed_clock_resets"] = json!(resets);
            validate_world_hooks(&clock).unwrap();
        }
    }

    #[test]
    fn world_hooks_never_promote_world_player_or_camera_flags() {
        let mut probe = fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        probe["window"]["game_session"]["active"]["world_initialization"]["world_hooks"] =
            world_hook_fixture();
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        for field in ["world_registered", "player_created", "camera_ready"] {
            let mut promoted = probe.clone();
            promoted["window"]["game_session"]["active"][field] = json!(true);
            assert!(
                validate(
                    &promoted,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("world_hooks");
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }

    fn loader_fixture() -> Value {
        let mut loader = json!({"origin":"fresh_constructor_snapshot","lifetime":"native_window_controller",
            "provider":"scoped_supplied_audio_and_base_map","operation_complete":true,"pending_phase":null,
            "pre_hooks_complete":true,"checkpoint_bound":true,"checkpoint_restore_active":false,
            "checkpoint_payload_initialized":false,"sound_info_bound":true,"world_ready_emitted":true,
            "player_created":false,"original_history_verified":false,"native_sdk_dispatch_verified":false,
            "native_work_timing_verified":false,"banks_playback_verified":false,
            "pre_state_sha256":"1".repeat(64),"maps_state_sha256":"2".repeat(64),"banks_state_sha256":"3".repeat(64),
            "phase_state_sha256":"4".repeat(64),"state_sha256":"5".repeat(64),"scope":"Authored actual loader ownership checkpoint"});
        for field in LOADER_COUNTS {
            loader[*field] = json!(0);
        }
        for (field, value) in [
            ("entry_phase", 7),
            ("current_phase", 21),
            ("maps_constructed", 2),
            ("map_zones", 9),
            ("map_component_slots", 1),
            ("map_context_bindings", 2),
            ("map_sections", 11),
            ("map_sections_published", 2),
            ("map_zones_published", 2),
            ("map_bindings", 2),
            ("checkpoint_payload_bytes", 0x37f8),
            ("checkpoint_skips", 2),
            ("source_bank_calls", 2),
            ("metadata_jobs_submitted", 1),
            ("backend_commands", 2),
            ("retained_banks", 2),
            ("retained_bank_bytes", 960),
            ("sound_entries", 7),
            ("wave_entries", 9),
            ("sound_info_rows", 8),
            ("sound_info_bytes", 128),
            ("work_completed", 6),
            ("barrier_promotions", 12),
            ("resource_polls", 12),
            ("work_busy_polls", 3),
            ("phase8_byte", 10),
            ("phase8_resets", 4),
            ("loader_c4_bits", 0x44bb8000),
            ("loader_c0_bits", 0x3a83126f),
            ("catalog_phase18_entries", 2),
            ("readiness_events", 2),
            ("readiness_local_stores", 4),
            ("readiness_owner_flags", 0x108),
            ("readiness_peer_flags", 0x108),
            ("readiness_user_flags", 0x108),
            ("readiness_local_flags", 0x109),
        ] {
            loader[field] = json!(value);
        }
        loader
    }

    fn loader_probe_fixture() -> Value {
        let mut probe = complete_fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        let snapshot = &mut probe["window"]["game_session"]["active"];
        let world = &mut snapshot["world_initialization"];
        world["navigation"] = navigation_fixture();
        world["post_trigger_resources"] = post_trigger_fixture();
        world["world_hooks"] = world_hook_fixture();
        world["loader"] = loader_fixture();
        snapshot["stage"] = json!("awaiting_player");
        snapshot["world_registered"] = json!(true);
        snapshot["manager_ready_flags_raw"] = json!(0x108);
        snapshot["user_ready_flags_raw"] = json!(0x108);
        probe
    }

    #[test]
    fn completed_loader_owns_world_readiness_but_not_a_player_or_camera() {
        let probe = loader_probe_fixture();
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        let snapshot = &probe["window"]["game_session"]["active"];
        assert_eq!(snapshot["stage"], "awaiting_player");
        assert_eq!(snapshot["player_created"], false);
        assert_eq!(snapshot["camera_ready"], false);
        assert_eq!(
            snapshot["world_initialization"]["world_hooks"]["world_ready_emitted"],
            false
        );
        let legacy = fixture(Check::Loaded {
            difficulty: 0,
            reentry: false,
        });
        validate(
            &legacy,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
    }

    #[test]
    fn loader_contract_rejects_early_readiness_private_exports_and_incoherent_stage() {
        for (field, value) in [
            ("current_phase", json!(19)),
            ("pending_phase", json!(21)),
            ("pre_hooks_complete", json!(false)),
            ("map_sections_published", json!(12)),
            ("map_zones_published", json!(10)),
            ("map_context_bindings", json!(4)),
            ("checkpoint_restore_active", json!(true)),
            ("checkpoint_payload_bytes", json!(0)),
            ("source_bank_calls", json!(4)),
            ("metadata_jobs_submitted", json!(2)),
            ("backend_sequence", json!(u64::from(u32::MAX) + 1)),
            ("sound_info_bound", json!(false)),
            ("work_pending", json!(1)),
            ("work_failed", json!(1)),
            ("work_completed", json!(2)),
            ("resource_polls", json!(11)),
            ("phase8_resets", json!(1)),
            ("loader_c4_bits", json!(0)),
            ("catalog_phase18_entries", json!(1)),
            ("readiness_events", json!(1)),
            ("readiness_local_stores", json!(2)),
            ("readiness_peer_flags", json!(0)),
            ("world_ready_emitted", json!(false)),
            ("player_created", json!(true)),
            ("original_history_verified", json!(true)),
            ("native_sdk_dispatch_verified", json!(true)),
            ("native_work_timing_verified", json!(true)),
            ("banks_playback_verified", json!(true)),
            ("maps_state_sha256", json!("X".repeat(64))),
            ("bank_payload", json!([1, 2, 3])),
        ] {
            let mut probe = loader_probe_fixture();
            probe["window"]["game_session"]["active"]["world_initialization"]["loader"][field] =
                value;
            assert!(
                validate(
                    &probe,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("stage", json!("awaiting_world")),
            ("world_registered", json!(false)),
            ("manager_ready_flags_raw", json!(0)),
            ("player_created", json!(true)),
            ("camera_ready", json!(true)),
        ] {
            let mut probe = loader_probe_fixture();
            probe["window"]["game_session"]["active"][field] = value;
            assert!(
                validate(
                    &probe,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
        let mut missing = loader_probe_fixture();
        missing["window"]["game_session"]["active"]["world_initialization"]["loader"]
            .as_object_mut()
            .unwrap()
            .remove("work_completed");
        assert!(
            validate(
                &missing,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }

    #[test]
    fn unresolved_wait28_keeps_pending_phase_and_legacy_zero_flags() {
        let mut probe = loader_probe_fixture();
        let snapshot = &mut probe["window"]["game_session"]["active"];
        snapshot["stage"] = json!("awaiting_world");
        snapshot["world_registered"] = json!(false);
        snapshot["manager_ready_flags_raw"] = json!(0);
        snapshot["user_ready_flags_raw"] = json!(0);
        let loader = &mut snapshot["world_initialization"]["loader"];
        loader["operation_complete"] = json!(false);
        loader["world_ready_emitted"] = json!(false);
        loader["current_phase"] = json!(28);
        loader["pending_phase"] = json!(19);
        loader["work_pending"] = json!(1);
        loader["barrier_promotions"] = json!(11);
        loader["resource_polls"] = json!(11);
        for field in [
            "readiness_events",
            "readiness_local_stores",
            "readiness_owner_flags",
            "readiness_peer_flags",
            "readiness_user_flags",
        ] {
            loader[field] = json!(0);
        }
        loader["readiness_local_flags"] = json!(1);
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        probe["window"]["game_session"]["active"]["world_initialization"]["loader"]["pending_phase"] =
            Value::Null;
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }

    fn player_probe_fixture() -> Value {
        let mut probe = loader_probe_fixture();
        let world = &mut probe["window"]["game_session"]["active"]["world_initialization"];
        world["actor_definitions"]["rng_draws"] = json!(2);
        world["player_initialization"] = json!({
            "origin":"fresh_constructor_snapshot","lifetime":"controller_arena","stage":"player_components_pending",
            "pending_dependency":"tp_attachment","operation_complete":false,"attempt_bound":true,"catalog_bound":true,
            "operation_entries":0,"creation_dispatches":1,"catalog_entries":2,"selected_catalog_index":1,"selected_source_ordinal":4,
            "requested_start_point_before":1,"requested_start_point_after":1,"base_constructor_complete":true,
            "handler_rng_reads":1,"base_reset_calls":2,"persistent_base_resets":2,"handler_kind_bound":false,"inventory_records":1,"inventory_capacity":16,
            "inventory_growth":16,"world_items":1,"world_actor_lists":1,"world_membership_lists":10,"actor_registrations":1,
            "physics_bound":true,"completion_published":false,"player_created":false,"camera_ready":false,
            "original_history_verified":false,"native_timing_verified":false,"base_state_sha256":"1".repeat(64),
            "prelude_state_sha256":"2".repeat(64),"world_state_sha256":"3".repeat(64),
            "persistent_state_sha256":"4".repeat(64),"state_sha256":"5".repeat(64),
            "scope":"Authored retained actor and capsule prefix; deferred scalar; basic return does not complete PlayerNew"});
        let shared = json!({
            "shared_count":1,"shared_live_leases":1,"shared_allocations":3,"shared_native_bytes":63680,"shared_host_capacity_bytes":64000,
            "shared_reset_calls":1,"shared_entry_noops":98,"shared_cache_stores":1,"shared_reset_complete":true,
            "runtime_base_reset_complete":true,"derived_reset_prefix_complete":true,
            "shared_state_sha256":"6".repeat(64),"reset_state_sha256":"7".repeat(64)});
        world["player_initialization"]
            .as_object_mut()
            .unwrap()
            .extend(shared.as_object().unwrap().clone());
        let loadout = json!({
            "scratch_prepared":true,"character_bound":true,"inventory_selection_complete":true,
            "scratch_capacity":32,"scratch_growth":16,"scratch_records":0,"scratch_native_bytes":4736,
            "scratch_host_capacity_bytes":4800,"scratch_reallocations":1,
            "character_resource":0x0109_0013,"character_key":0x4d00_00a7,"character_source_index":8,
            "guns_resource":0x0109_000f,"guns_payload_bytes":1234,"metadata_source_index":4,
            "metadata_sorted_index":5,"weapon_registry_keys":9,"primary_order_keys":8,"secondary_order_keys":3,
            "inventory_primary_order_index":6,"inventory_interface_callbacks":0,"inventory_selected_record":0,
            "scratch_state_sha256":"8".repeat(64),"loadout_state_sha256":"9".repeat(64)});
        world["player_initialization"]
            .as_object_mut()
            .unwrap()
            .extend(loadout.as_object().unwrap().clone());
        world["player_initialization"]["constructor_flag"] = json!({"origin":"captured_member_snapshot",
            "raw":0,"changed":false,"interface_callbacks":0,"original_history_verified":false});
        let mut virtual_init = json!({"stage":"player_basic_complete","pending_dependency":"tp_fp_components",
            "item_registry_records":1,"item_registry_capacity":16,"item_registry_growth":16,"item_registry_counter":1,
            "item_registry_native_bytes":128,"item_registry_host_capacity_bytes":512,
            "origin_zone_queries":1,"origin_zone_nodes":2,"desired_membership":0x2d,"item_membership":0x3d,
            "item_flags":0x2b,"item_priority":255});
        let links = json!({"factory_complete":true,"item_registry_bound":true,"actor_registered":true,
            "handler_bound":true,"origin_zone_queried":true,"physics_constructed":true,"physics_backlink_bound":true,
            "physics_activated":true,"initializer_complete":true,"scalar_pending":true,"scalar_value_known":false,"item_state_sha256":"a".repeat(64),
            "physics_state_sha256":"b".repeat(64),"registry_state_sha256":"c".repeat(64),
            "scope":"Authored basic return and activation with deferred scalar, no completed PlayerNew"});
        virtual_init
            .as_object_mut()
            .unwrap()
            .extend(links.as_object().unwrap().clone());
        world["player_initialization"]["virtual_initialization"] = virtual_init;
        world["player_initialization"]["component_initialization"] = json!({
            "stage":"tp_attachment_pending","pending_dependency":"tp_attachment",
            "native_component_bytes":0x1b8,"constructed_slots":4,"active_slots":1,
            "retained_file_bytes":1024,"retained_section_bytes":256,
            "third_person_constructed":true,"third_person_attached":false,"first_person_constructed":false,
            "skin_request_resolved":true,"texture_dependencies_enumerated":true,"component_binding_complete":true,
            "texture_directory_outcome":"enumerated","texture_requests":3,"texture_targets_resident":3,
            "texture_residency_complete":true,"texture_requests_state_sha256":"f".repeat(64),
            "preparation_state_sha256":"d".repeat(64),"target_state_sha256":"e".repeat(64),
            "skeleton_binding":player_skeleton::tests::fixture(),
            "bind_refresh":player_refresh::tests::fixture(),
            "scope":"Authored TP bind return; attachment remains pending"});
        probe
    }

    #[test]
    fn player_prelude_accepts_relative_catalog_and_preserves_live_boundaries() {
        let probe = player_probe_fixture();
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        let player = &probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"];
        assert_eq!(player.as_object().unwrap().len(), 78);
        assert_eq!(
            player["virtual_initialization"].as_object().unwrap().len(),
            29
        );
        assert_eq!(player["selected_catalog_index"], 1);
        assert_eq!(player["player_created"], false);
        assert_eq!(player["operation_complete"], false);
    }

    #[test]
    fn player_prelude_rejects_private_exports_invalid_counts_and_early_completion() {
        for (field, value) in [
            ("operation_complete", json!(true)),
            ("physics_bound", json!(false)),
            ("completion_published", json!(true)),
            ("player_created", json!(true)),
            ("camera_ready", json!(true)),
            ("handler_kind_bound", json!(true)),
            ("base_constructor_complete", json!(false)),
            ("attempt_bound", json!(false)),
            ("catalog_bound", json!(false)),
            ("base_reset_calls", json!(1)),
            ("handler_rng_reads", json!(0)),
            ("selected_catalog_index", json!(2)),
            ("selected_source_ordinal", json!(u64::MAX)),
            ("requested_start_point_before", json!(2)),
            ("inventory_records", json!(0)),
            ("inventory_capacity", json!(0)),
            ("world_items", json!(0)),
            ("actor_registrations", json!(2)),
            ("stage", json!("player_created")),
            ("pending_dependency", json!("none")),
            ("base_state_sha256", json!("X".repeat(64))),
            ("original_history_verified", json!(true)),
            ("native_timing_verified", json!(true)),
            ("shared_count", json!(0)),
            ("shared_live_leases", json!(2)),
            ("shared_reset_calls", json!(2)),
            ("shared_allocations", json!(2)),
            ("shared_native_bytes", json!(63_679)),
            ("shared_host_capacity_bytes", json!(63_679)),
            ("shared_entry_noops", json!(97)),
            ("shared_cache_stores", json!(2)),
            ("shared_reset_complete", json!(false)),
            ("runtime_base_reset_complete", json!(false)),
            ("derived_reset_prefix_complete", json!(false)),
            ("shared_state_sha256", json!("short")),
            ("reset_state_sha256", json!("X".repeat(64))),
            ("scratch_prepared", json!(false)),
            ("character_bound", json!(false)),
            ("inventory_selection_complete", json!(false)),
            ("scratch_capacity", json!(31)),
            ("scratch_growth", json!(0)),
            ("scratch_records", json!(1)),
            ("scratch_native_bytes", json!(4735)),
            ("scratch_host_capacity_bytes", json!(4735)),
            ("scratch_reallocations", json!(2)),
            ("character_resource", json!(0)),
            ("character_key", json!(0x4d00_0000_u32)),
            ("guns_resource", json!(0)),
            ("guns_payload_bytes", json!(0)),
            ("weapon_registry_keys", json!(5)),
            ("primary_order_keys", json!(6)),
            ("secondary_order_keys", json!(0)),
            ("inventory_interface_callbacks", json!(1)),
            ("inventory_selected_record", json!(1)),
            ("scratch_state_sha256", json!("X".repeat(64))),
            ("loadout_state_sha256", json!("short")),
            ("pose_bits", json!([0, 0, 0, 0])),
            ("constructor_flag", Value::Null),
            ("virtual_initialization", Value::Null),
            ("native_bytes", json!([1, 2, 3])),
        ] {
            let mut probe = player_probe_fixture();
            probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"]
                [field] = value;
            assert!(
                validate(
                    &probe,
                    Check::Loaded {
                        difficulty: 0,
                        reentry: false
                    }
                )
                .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn player_reset_requires_all_scratch_and_loadout_fields_without_exporting_source_payloads() {
        for field in [
            "scratch_prepared",
            "character_bound",
            "inventory_selection_complete",
            "scratch_capacity",
            "scratch_growth",
            "scratch_records",
            "scratch_native_bytes",
            "scratch_host_capacity_bytes",
            "scratch_reallocations",
            "character_resource",
            "character_key",
            "character_source_index",
            "guns_resource",
            "guns_payload_bytes",
            "metadata_source_index",
            "metadata_sorted_index",
            "weapon_registry_keys",
            "primary_order_keys",
            "secondary_order_keys",
            "inventory_primary_order_index",
            "inventory_interface_callbacks",
            "inventory_selected_record",
            "scratch_state_sha256",
            "loadout_state_sha256",
            "constructor_flag",
            "virtual_initialization",
        ] {
            let mut probe = player_probe_fixture();
            probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"]
                .as_object_mut().unwrap().remove(field);
            let snapshot = &probe["window"]["game_session"]["active"];
            let world = &snapshot["world_initialization"];
            assert!(
                validate_player_initialization(&world["player_initialization"], world, snapshot)
                    .is_err(),
                "{field}"
            );
        }
        // This deliberately uses authored values rather than Tutorial corpus
        // counts: generic validation checks relationships, not its row count.
        let probe = player_probe_fixture();
        let snapshot = &probe["window"]["game_session"]["active"];
        let world = &snapshot["world_initialization"];
        let player = &world["player_initialization"];
        assert_eq!(player["weapon_registry_keys"], 9);
        assert_eq!(player["metadata_sorted_index"], 5);
        assert_eq!(player["primary_order_keys"], 8);
        assert_eq!(player["inventory_primary_order_index"], 6);
        validate_player_initialization(player, world, snapshot).unwrap();
    }

    #[test]
    fn virtual_owners_reject_broken_links_activation_invalid_registry_and_private_payloads() {
        for (field, value) in [
            ("factory_complete", json!(false)),
            ("item_registry_bound", json!(false)),
            ("actor_registered", json!(false)),
            ("handler_bound", json!(false)),
            ("origin_zone_queried", json!(false)),
            ("physics_constructed", json!(false)),
            ("physics_backlink_bound", json!(false)),
            ("physics_activated", json!(false)),
            ("initializer_complete", json!(false)),
            ("scalar_pending", json!(false)),
            ("scalar_value_known", json!(true)),
            ("item_registry_records", json!(0)),
            ("item_registry_capacity", json!(0)),
            ("item_registry_growth", json!(0)),
            ("item_registry_counter", json!(0)),
            ("item_registry_native_bytes", json!(127)),
            ("item_registry_counter", json!(u64::from(u32::MAX) + 1)),
            ("item_registry_host_capacity_bytes", json!(127)),
            ("origin_zone_queries", json!(0)),
            ("origin_zone_nodes", json!(0)),
            ("item_membership", json!(0x2d)),
            ("desired_membership", json!(0x2f)),
            ("item_flags", json!(0x0b)),
            ("item_priority", json!(0)),
            ("stage", json!("complete")),
            ("pending_dependency", json!("none")),
            ("physics_state_sha256", json!("X".repeat(64))),
            ("registry_state_sha256", json!("short")),
            ("capsule", json!([1, 2, 3])),
        ] {
            let mut probe = player_probe_fixture();
            let snapshot = &mut probe["window"]["game_session"]["active"];
            snapshot["world_initialization"]["player_initialization"]["virtual_initialization"]
                [field] = value;
            let world = &snapshot["world_initialization"];
            assert!(
                validate_player_initialization(&world["player_initialization"], world, snapshot)
                    .is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("origin", json!("player_994")),
            ("raw", json!(256)),
            ("changed", json!(1)),
            ("interface_callbacks", json!(1)),
            ("original_history_verified", json!(true)),
            ("native_fields", json!([0, 0, 0, 0])),
        ] {
            let mut probe = player_probe_fixture();
            let snapshot = &mut probe["window"]["game_session"]["active"];
            snapshot["world_initialization"]["player_initialization"]["constructor_flag"][field] =
                value;
            let world = &snapshot["world_initialization"];
            assert!(
                validate_player_initialization(&world["player_initialization"], world, snapshot)
                    .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn tp_component_evidence_rejects_attachment_texture_and_private_payload_shortcuts() {
        for (field, value) in [
            ("third_person_constructed", json!(false)),
            ("third_person_attached", json!(true)),
            ("first_person_constructed", json!(true)),
            ("skin_request_resolved", json!(false)),
            ("texture_dependencies_enumerated", json!(false)),
            ("texture_directory_outcome", json!("unknown")),
            ("texture_requests", json!(4097)),
            ("texture_targets_resident", json!(2)),
            ("texture_residency_complete", json!(false)),
            ("texture_requests_state_sha256", json!("short")),
            ("component_binding_complete", json!(false)),
            ("native_component_bytes", json!(0)),
            ("constructed_slots", json!(1)),
            ("active_slots", json!(4)),
            ("retained_file_bytes", json!(0)),
            ("retained_section_bytes", json!(2048)),
            ("target_state_sha256", json!("short")),
            ("source_rows", json!([1, 2, 3])),
        ] {
            let mut probe = player_probe_fixture();
            let snapshot = &mut probe["window"]["game_session"]["active"];
            snapshot["world_initialization"]["player_initialization"]["component_initialization"]
                [field] = value;
            let world = &snapshot["world_initialization"];
            assert!(
                validate_player_initialization(&world["player_initialization"], world, snapshot)
                    .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn unknown_current_flag_keeps_reset_only_state_with_retained_older_actors() {
        let mut probe = player_probe_fixture();
        let snapshot = &mut probe["window"]["game_session"]["active"];
        let player = &mut snapshot["world_initialization"]["player_initialization"];
        player["constructor_flag"] = Value::Null;
        player["virtual_initialization"] = Value::Null;
        player["component_initialization"] = Value::Null;
        player["stage"] = json!("player_reset_complete");
        player["pending_dependency"] = json!("constructor_source_flag");
        player["physics_bound"] = json!(false);
        player["world_items"] = json!(3);
        player["actor_registrations"] = json!(2);
        let world = &snapshot["world_initialization"];
        validate_player_initialization(&world["player_initialization"], world, snapshot).unwrap();
        assert!(!validate_player_virtual_owners(&world["player_initialization"]).unwrap());
        snapshot["world_initialization"]["player_initialization"]["physics_bound"] = json!(true);
        let world = &snapshot["world_initialization"];
        assert!(
            validate_player_initialization(&world["player_initialization"], world, snapshot)
                .is_err()
        );
    }

    #[test]
    fn retained_registry_and_actor_totals_can_include_older_complete_or_partial_items() {
        let mut probe = player_probe_fixture();
        let snapshot = &mut probe["window"]["game_session"]["active"];
        let player = &mut snapshot["world_initialization"]["player_initialization"];
        player["world_items"] = json!(3);
        player["actor_registrations"] = json!(2);
        let state = &mut player["virtual_initialization"];
        state["item_registry_records"] = json!(2);
        state["item_registry_counter"] = json!(3);
        state["item_registry_host_capacity_bytes"] = json!(1024);
        let world = &snapshot["world_initialization"];
        validate_player_initialization(&world["player_initialization"], world, snapshot).unwrap();
        snapshot["world_initialization"]["player_initialization"]["virtual_initialization"]["item_registry_records"] =
            json!(4);
        let world = &snapshot["world_initialization"];
        assert!(
            validate_player_initialization(&world["player_initialization"], world, snapshot)
                .is_err()
        );
    }

    #[test]
    fn shared_player_reset_retains_cancelled_leases_without_reallocating_or_claiming_player_completion()
     {
        for persistent_resets in [2, 3, 4] {
            let mut probe = player_probe_fixture();
            let player = &mut probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"];
            player["persistent_base_resets"] = json!(persistent_resets);
            player["shared_host_capacity_bytes"] = json!(70_000);
            player["shared_count"] = json!(2);
            player["shared_live_leases"] = json!(2);
            player["shared_reset_calls"] = json!(2);
            player["shared_state_sha256"] = json!("a".repeat(64));
            player["reset_state_sha256"] = json!("b".repeat(64));
            let snapshot = &probe["window"]["game_session"]["active"];
            let world = &snapshot["world_initialization"];
            validate_player_initialization(&world["player_initialization"], world, snapshot)
                .unwrap();
            assert_eq!(world["player_initialization"]["shared_allocations"], 3);
            assert_eq!(
                world["player_initialization"]["shared_reset_complete"],
                true
            );
            assert_eq!(world["player_initialization"]["scratch_reallocations"], 1);
        }
        let mut probe = player_probe_fixture();
        probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("shared_state_sha256");
        let snapshot = &probe["window"]["game_session"]["active"];
        let world = &snapshot["world_initialization"];
        assert!(
            validate_player_initialization(&world["player_initialization"], world, snapshot)
                .is_err()
        );
    }

    #[test]
    fn player_prelude_null_is_legacy_but_missing_or_unready_world_is_invalid() {
        let mut probe = player_probe_fixture();
        probe["window"]["game_session"]["active"]["world_initialization"]["player_initialization"] =
            Value::Null;
        validate(
            &probe,
            Check::Loaded {
                difficulty: 0,
                reentry: false,
            },
        )
        .unwrap();
        probe["window"]["game_session"]["active"]["world_initialization"]
            .as_object_mut()
            .unwrap()
            .remove("player_initialization");
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
        let mut probe = player_probe_fixture();
        probe["window"]["game_session"]["active"]["world_initialization"]["loader"]["operation_complete"] =
            json!(false);
        assert!(
            validate(
                &probe,
                Check::Loaded {
                    difficulty: 0,
                    reentry: false
                }
            )
            .is_err()
        );
    }
}
