//! Source-bound retained mission metadata checks. This gate never claims a
//! registered world, initialized player, or original-runtime parity.
mod player;
mod registration;
mod residency;
mod spawn;
mod world;

use crate::{cargo, executable, now, output, record, report};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

const TUTORIAL: u32 = 0x0181_016b;
const DAM: u32 = 0x0181_013f;
const PRIMARY_LIMIT: u64 = 4_096;
const SECONDARY_LIMIT: u64 = 1_024;
const RESOURCE_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Initialization {
    Resources,
    Layers,
    RawFrame,
    Prewarm,
    UncachedCameraBone,
    CachedCameraBone,
}

pub fn run(disc: &Path) -> Result<()> {
    let hash = crate::source_hash()?;
    let result = (|| -> Result<Value> {
        cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
        let disc = disc.to_str().context("mission disc path must be UTF-8")?;
        let inventory: Value =
            serde_json::from_slice(&output(&executable("release"), &["inspect", disc])?)?;
        ensure!(
            inventory["disc"]["game_id"] == "SJBE52" && inventory["disc"]["revision"] == 0,
            "mission context gate requires SJBE52 revision zero"
        );
        let folder =
            Path::new("local/mission-context").join(format!("{}-{}", now(), std::process::id()));
        fs::create_dir_all("local/mission-context")?;
        fs::create_dir(&folder)?;
        let mut contexts = Vec::new();
        for difficulty in 0..=3 {
            contexts.push(probe(disc, TUTORIAL, difficulty, &folder)?);
        }
        contexts.push(probe(disc, DAM, 0, &folder)?);
        let fallback = probe_for_request(disc, TUTORIAL, 0, 99, &folder)?;
        let zero = probe_for_request(disc, TUTORIAL, 0, 0, &folder)?;
        ensure!(
            fallback["spawn"]["pose_sha256"] == zero["spawn"]["pose_sha256"],
            "ID-zero fallback changed source pose"
        );
        contexts.push(fallback);
        contexts.push(zero);
        for inputs in [
            [0x0900_071e, 0, 0x0900_065e],
            [0x0900_0b70, 0, 0x0900_000b],
            [0x0900_071e, 0x09ff_fffe, 0],
            [0x09ff_ffff, 0, 0],
        ] {
            contexts.push(probe_config(disc, TUTORIAL, 0, 1, &folder, Some(inputs))?);
        }
        for level in [TUTORIAL, DAM] {
            contexts.push(probe_options(
                disc,
                level,
                0,
                1,
                &folder,
                None,
                Initialization::Layers,
            )?);
            contexts.push(probe_options(
                disc,
                level,
                0,
                1,
                &folder,
                None,
                Initialization::RawFrame,
            )?);
            contexts.push(probe_options(
                disc,
                level,
                0,
                1,
                &folder,
                None,
                Initialization::Prewarm,
            )?);
            for initialization in [
                Initialization::UncachedCameraBone,
                Initialization::CachedCameraBone,
            ] {
                contexts.push(probe_options(
                    disc,
                    level,
                    0,
                    1,
                    &folder,
                    None,
                    initialization,
                )?);
            }
        }
        let rejected_file = folder.join("unknown-level.json");
        let rejected = Command::new(executable("release"))
            .args([
                "mission-context",
                disc,
                "--level",
                "deadbeef",
                "--difficulty",
                "0",
                "--output",
                rejected_file
                    .to_str()
                    .context("mission output path must be UTF-8")?,
            ])
            .output()?;
        fs::write(folder.join("unknown-level.stderr.txt"), &rejected.stderr)?;
        validate_unknown(
            rejected.status.success(),
            &rejected.stdout,
            &rejected.stderr,
            rejected_file.exists(),
        )?;
        println!(
            "Mission context: twenty-one Tutorial/Dam resource, layer, raw-frame, prewarm and camera-bone probes passed; unknown level rejected. {}",
            folder.display()
        );
        Ok(json!({
            "disc":inventory["disc"],"probe_count":contexts.len(),"contexts":contexts,
            "unknown_level_rejected":true,"reference_compared":false,
            "scope":"Twenty-one local mission-context probes retain the source catalog and discovered resource owners, bind actual spatial/BSP/placement data, prepare source StartPoint plans and initialize the selected first-person skin bone storage/tag binding. All four Tutorial difficulties, a Dam context and ID-zero fallback/direct selection are exercised. Four additional explicit mode plans check the source primary/secondary requests, previous-miss fallback and missing target with independent fingerprints; they do not supply live initial weapon/history or create animation children. Source registry indices remain separate from loaded ordinals; pose/rig fingerprints export no coordinates or bone arrays. Constructor identity is not a sampled animation frame; player physics and camera matrices remain incomplete. Tutorial and Dam additionally admit the source default inventory, verify supplied DOL identity while retaining DATA5 only, remap source mode requests and construct real owned control/script/clip objects with checked layer membership. Both post-selector head blend setters execute with source-initial values. Two additional probes select the owned clip and decode a privately cached raw polynomial frame, with independent fingerprints and explicit host-sqrt uncertainty. Fixed reset-prewarm probes additionally advance the source-supported child clocks, compose the captured control and skin/root-adjusted script at saturated fade, and commit all 48 local bone poses through scratch flags. Native quaternion estimates remain unverified; external scheduler input, matrix refresh and PlayerNew/camera state remain incomplete. Four additional camera-bone probes bind the actual tag against committed locals and retain full/ancestor matrices privately. Fresh FP actor-cache queries exercise parent-first misses and a parent hit; explicit uncached inspection preserves its different ancestor association. Fingerprints match independent source/host reconstruction, with provisional input and matrix arithmetic provenance. Owner-world transforms, full FP/TP global cache setup/history and final camera adjustment remain unavailable. An unknown level fails without emitting a context receipt. Counts describe loaded data, not completion. No registered gameplay world, initialized player, mission execution, original asset text/code export, or original-runtime parity."
        }))
    })();
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Mission context gate failed: {error:#}");
            (
                false,
                json!({"message":"Mission context gate failed; inspect local terminal output.","reference_compared":false}),
            )
        }
    };
    record("mission", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "mission context gate failed");
    Ok(())
}

fn probe(disc: &str, level: u32, difficulty: u8, folder: &Path) -> Result<Value> {
    probe_for_request(disc, level, difficulty, 1, folder)
}
fn probe_for_request(
    disc: &str,
    level: u32,
    difficulty: u8,
    start_point_id: u32,
    folder: &Path,
) -> Result<Value> {
    probe_config(disc, level, difficulty, start_point_id, folder, None)
}

fn probe_config(
    disc: &str,
    level: u32,
    difficulty: u8,
    start_point_id: u32,
    folder: &Path,
    animation: Option<[u32; 3]>,
) -> Result<Value> {
    probe_options(
        disc,
        level,
        difficulty,
        start_point_id,
        folder,
        animation,
        Initialization::Resources,
    )
}

fn probe_options(
    disc: &str,
    level: u32,
    difficulty: u8,
    start_point_id: u32,
    folder: &Path,
    animation: Option<[u32; 3]>,
    initialization: Initialization,
) -> Result<Value> {
    let initialize_layers = !matches!(initialization, Initialization::Resources);
    let decode_frame = matches!(initialization, Initialization::RawFrame);
    let uncached_camera = matches!(initialization, Initialization::UncachedCameraBone);
    let cached_camera = matches!(initialization, Initialization::CachedCameraBone);
    let prewarm =
        matches!(initialization, Initialization::Prewarm) || uncached_camera || cached_camera;
    ensure!(
        !initialize_layers || animation.is_none(),
        "layer snapshot conflicts with manual mode inputs"
    );
    let level_arg = format!("{level:08x}");
    let difficulty_arg = difficulty.to_string();
    let start_point_arg = start_point_id.to_string();
    let mut suffix = animation
        .map(|[r, p, f]| format!("-animation-{r:08x}-{p:08x}-{f:08x}"))
        .unwrap_or_default();
    if initialize_layers {
        suffix.push_str("-owned-layers");
    }
    if decode_frame {
        suffix.push_str("-raw-frame");
    }
    if prewarm {
        suffix.push_str("-prewarm-pose");
    }
    if uncached_camera {
        suffix.push_str("-uncached-camera-bone");
    }
    if cached_camera {
        suffix.push_str("-cached-camera-bone");
    }
    let path = folder.join(format!(
        "{level_arg}-difficulty-{difficulty}-start-{start_point_id}{suffix}.json"
    ));
    let mut args = [
        "mission-context",
        disc,
        "--level",
        &level_arg,
        "--difficulty",
        &difficulty_arg,
        "--start-point-id",
        &start_point_arg,
        "--output",
        path.to_str().context("mission output path must be UTF-8")?,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if let Some([requested, previous, fallback]) = animation {
        for (flag, value) in [
            ("--animation-request", requested),
            ("--animation-previous", previous),
            ("--animation-fallback", fallback),
        ] {
            args.push(flag.to_owned());
            args.push(format!("{value:08x}"));
        }
    }
    if initialize_layers {
        args.push("--initialize-animation-layers".to_owned());
    }
    if decode_frame {
        args.push("--decode-initial-frame".to_owned());
    }
    if prewarm {
        args.push("--evaluate-prewarm-pose".to_owned());
    }
    if uncached_camera {
        args.push("--evaluate-uncached-camera-bone".to_owned());
    }
    if cached_camera {
        args.push("--evaluate-cached-camera-bone".to_owned());
    }
    let bytes = output(
        &executable("release"),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "mission metadata output exceeds host gate budget"
    );
    ensure!(
        fs::read(&path)? == bytes,
        "mission stdout differs from its saved receipt"
    );
    let context: Value =
        serde_json::from_slice(&bytes).context("mission context did not emit JSON")?;
    if start_point_id == 1 {
        validate(&context, level, difficulty)?;
    } else {
        validate_for_request(&context, level, difficulty, start_point_id)?;
    }
    if let Some(inputs) = animation {
        player::validate_mode_probe(&context["player_components"]["mode_plan"], inputs)?;
    } else {
        ensure!(
            context["player_components"]["mode_plan"].is_null(),
            "default context invented initial animation inputs"
        );
    }
    if initialize_layers {
        player::validate_initial_layers(&context["player_components"]["initial_layers"])?;
        if decode_frame {
            ensure!(
                context["player_components"]["initial_layers"]["raw_frame"].is_object(),
                "raw-frame probe did not decode a frame"
            );
        } else {
            ensure!(
                context["player_components"]["initial_layers"]["raw_frame"].is_null(),
                "unrequested raw frame was decoded"
            );
        }
        ensure!(
            context["player_components"]["initial_layers"]["prewarm_pose"].is_object() == prewarm,
            "prewarm pose presence differs from the explicit probe request"
        );
    } else {
        ensure!(
            context["player_components"]["initial_layers"].is_null(),
            "unrequested source layer objects initialized"
        );
    }
    let camera = &context["player_components"]["camera_bone_transform"];
    ensure!(
        camera.is_object() == (uncached_camera || cached_camera),
        "camera-bone transform presence differs from the explicit probe request"
    );
    if uncached_camera || cached_camera {
        ensure!(
            camera["cache_selection"]
                == if cached_camera {
                    "fresh_fp_actor_cache"
                } else {
                    "explicit_disabled_inspection"
                },
            "camera-bone probe selected a different cache boundary"
        );
    }
    Ok(context)
}

fn closed(value: &Value, keys: &[&str]) -> Result<()> {
    let fields = value
        .as_object()
        .context("mission metadata object is absent")?;
    ensure!(
        fields.len() == keys.len() && keys.iter().all(|key| fields.contains_key(*key)),
        "mission metadata contains missing or unexpected fields"
    );
    Ok(())
}

fn number(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .with_context(|| format!("mission metadata integer {key} is absent"))
}

fn validate(value: &Value, level: u32, difficulty: u8) -> Result<()> {
    validate_for_request(value, level, difficulty, 1)
}
fn validate_for_request(
    value: &Value,
    level: u32,
    difficulty: u8,
    start_point_id: u32,
) -> Result<()> {
    closed(
        value,
        &[
            "state",
            "catalog",
            "selection",
            "resource",
            "residency",
            "registration",
            "spawn",
            "map_instances",
            "player_components",
            "resources_retained",
            "world_registered",
            "player_initialized",
            "reference_compared",
            "scope",
        ],
    )?;
    ensure!(
        value["state"] == "resources_retained"
            && value["resources_retained"] == true
            && value["world_registered"] == false
            && value["player_initialized"] == false
            && value["reference_compared"] == false,
        "mission metadata promoted incomplete gameplay readiness"
    );
    ensure!(
        value["scope"]
            .as_str()
            .is_some_and(|scope| !scope.is_empty() && scope.len() <= 2_048),
        "mission metadata scope is absent or oversized"
    );
    let catalog = &value["catalog"];
    closed(
        catalog,
        &[
            "resource_hash",
            "payload_bytes",
            "primary_sheet_hash",
            "secondary_sheet_hash",
            "primary_rows_read",
            "secondary_rows_read",
            "primary_rows_registered",
            "secondary_rows_registered",
            "duplicate_rows",
        ],
    )?;
    ensure!(
        catalog["resource_hash"] == 0x0109_0006u32
            && catalog["primary_sheet_hash"] == 0x1400_0011u32
            && catalog["secondary_sheet_hash"] == 0x1400_003au32,
        "mission catalog source identities differ"
    );
    let payload = number(catalog, "payload_bytes")?;
    ensure!(
        (0xc8..=1024 * 1024).contains(&payload),
        "mission catalog payload is unbounded"
    );
    let primary = number(catalog, "primary_rows_read")?;
    let secondary = number(catalog, "secondary_rows_read")?;
    let registered_primary = number(catalog, "primary_rows_registered")?;
    let registered_secondary = number(catalog, "secondary_rows_registered")?;
    ensure!(
        (1..=PRIMARY_LIMIT).contains(&primary)
            && (1..=SECONDARY_LIMIT).contains(&secondary)
            && (1..=primary).contains(&registered_primary)
            && (1..=secondary).contains(&registered_secondary)
            && number(catalog, "duplicate_rows")?
                == primary + secondary - registered_primary - registered_secondary,
        "mission catalog counts or duplicate diagnostics disagree"
    );
    let selection = &value["selection"];
    closed(
        selection,
        &[
            "level_hash",
            "difficulty",
            "difficulty_label",
            "source_row_index",
            "source_row_address",
            "text_page_hash",
            "mission_object_resource_raw",
            "io_resource_raw",
            "intro_movie_hash",
            "outro_movie_hash",
            "bank_hashes",
            "character_request",
            "tuning_hash",
        ],
    )?;
    let labels = ["Operative", "Agent", "007", "007 Classic"];
    ensure!(
        difficulty <= 3
            && selection["level_hash"] == level
            && selection["difficulty"] == difficulty
            && selection["difficulty_label"] == labels[usize::from(difficulty)]
            && selection["tuning_hash"] == level,
        "mission selection or difficulty changed"
    );
    ensure!(
        number(selection, "source_row_index")? < primary
            && number(selection, "source_row_address")? >= 0xc8
            && number(selection, "source_row_address")?
                .checked_add(180)
                .is_some_and(|end| end <= payload),
        "mission selected row provenance is outside catalog"
    );
    let (text, objects, io, intro, banks, map) = match level {
        TUTORIAL => (
            0x0801_0019u32,
            0u32,
            0u32,
            0u32,
            [0x2d50_0079u32, u32::MAX, u32::MAX],
            0x8500_0000u32,
        ),
        DAM => (
            0x0801_000bu32,
            0x0109_0048u32,
            0x0106_0036u32,
            0x4210_003fu32,
            [0x2d50_001cu32, u32::MAX, 0x2d50_005bu32],
            0x0500_0024u32,
        ),
        _ => anyhow::bail!("mission gate expectation has an unknown level"),
    };
    ensure!(
        selection["text_page_hash"] == text
            && selection["mission_object_resource_raw"] == objects
            && selection["io_resource_raw"] == io
            && selection["intro_movie_hash"] == intro
            && selection["outro_movie_hash"] == 0
            && selection["bank_hashes"] == json!(banks),
        "mission source fields differ from the selected level contract"
    );
    validate_resource(&value["resource"], level, map)?;
    residency::validate(&value["residency"], &value["resource"], level)?;
    registration::validate(&value["registration"], &value["residency"])?;
    player::validate(&value["player_components"], selection)?;
    world::validate(
        &value["map_instances"],
        &value["residency"],
        &value["registration"],
    )?;
    spawn::validate(
        &value["spawn"],
        level,
        start_point_id,
        &value["registration"],
        &value["residency"],
    )?;
    Ok(())
}

fn validate_resource(resource: &Value, level: u32, expected_map: u32) -> Result<()> {
    closed(
        resource,
        &[
            "resource_hash",
            "payload_bytes",
            "base_bytes",
            "maps",
            "entity_descriptors",
            "script_descriptors",
            "textures",
            "requested_map_reference",
            "resolved_map_hash",
            "next_link",
        ],
    )?;
    let bytes = number(resource, "payload_bytes")?;
    let base = number(resource, "base_bytes")?;
    ensure!(
        resource["resource_hash"] == level
            && resource["requested_map_reference"] == 0x8500_0000u32
            && resource["resolved_map_hash"] == expected_map
            && (0xc8..=RESOURCE_LIMIT).contains(&bytes)
            && (0xc8..=bytes).contains(&base),
        "mission resource identity or payload bounds differ"
    );
    let entities = number(resource, "entity_descriptors")?;
    ensure!(
        (1..=32_767).contains(&entities)
            && (1..=32_767).contains(&number(resource, "script_descriptors")?)
            && number(resource, "textures")? <= 32_767,
        "mission resource descriptors are absent or unbounded"
    );
    let maps = resource["maps"]
        .as_array()
        .context("mission resource maps are absent")?;
    ensure!(
        !maps.is_empty() && maps.len() <= 128,
        "mission map count is empty or unbounded"
    );
    let mut found = false;
    for (index, map) in maps.iter().enumerate() {
        closed(
            map,
            &[
                "directory_index",
                "hash",
                "zones",
                "bsp_present",
                "bsp_nodes",
                "placements",
                "first_placement_reference",
                "first_placement_entity",
            ],
        )?;
        ensure!(
            number(map, "directory_index")? == index as u64
                && number(map, "hash")? <= u64::from(u32::MAX)
                && number(map, "zones")? <= 65_536
                && number(map, "bsp_nodes")? <= 1_000_000
                && map["bsp_present"].is_boolean()
                && (map["bsp_present"] != false || number(map, "bsp_nodes")? == 0),
            "mission map metadata is malformed"
        );
        let placements = number(map, "placements")?;
        ensure!(
            placements <= 1_000_000,
            "mission placement count is unbounded"
        );
        if placements == 0 {
            ensure!(
                map["first_placement_reference"].is_null()
                    && map["first_placement_entity"].is_null(),
                "empty mission map invented a first placement"
            );
        } else {
            ensure!(
                number(map, "first_placement_reference")? <= u64::from(u32::MAX),
                "mission first placement reference is invalid"
            );
            let entity = &map["first_placement_entity"];
            closed(entity, &["directory_index", "hash", "entity_type"])?;
            ensure!(
                number(entity, "directory_index")? < entities
                    && number(entity, "hash")? <= u64::from(u32::MAX)
                    && number(entity, "entity_type")? <= u64::from(u32::MAX),
                "mission first placement binding is invalid"
            );
        }
        if map["hash"] == expected_map {
            found = true;
            if level == TUTORIAL {
                ensure!(
                    placements > 0
                        && map["first_placement_reference"] == 0x8200_0000u32
                        && map["first_placement_entity"]["directory_index"] == 0
                        && map["first_placement_entity"]["hash"] == 0x0200_0358u32
                        && map["first_placement_entity"]["entity_type"] == 0x607,
                    "tutorial coded navmesh placement was not retained"
                );
            }
        }
    }
    ensure!(found, "mission source map descriptor is absent");
    let expected = match level {
        TUTORIAL => Some((0x018b_002a, 0x0500_0070, 40, 10)),
        DAM => Some((0x018b_0006, 0x0500_0000, 0, 0)),
        _ => None,
    };
    validate_link(&resource["next_link"], bytes, expected_map, expected)?;
    Ok(())
}

fn validate_link(
    link: &Value,
    payload: u64,
    source_map: u32,
    expected: Option<(u32, u32, u64, u64)>,
) -> Result<()> {
    if link.is_null() {
        ensure!(
            expected.is_none(),
            "required mission linked-map result is absent"
        );
        return Ok(());
    }
    closed(link, &["file", "map", "locator"])?;
    let file = number(link, "file")?;
    let map = number(link, "map")?;
    ensure!(
        file <= u64::from(u32::MAX) && map <= u64::from(u32::MAX),
        "mission linked-map properties are outside their stored width"
    );
    let locator = &link["locator"];
    closed(
        locator,
        &[
            "map_reference",
            "map_directory_index",
            "map_hash",
            "map_address",
            "trigger_block",
            "trigger_ordinal",
            "record_address",
            "schema_index",
            "schema_address",
            "native_type_index",
            "flags_raw",
        ],
    )?;
    let flags = number(locator, "flags_raw")?;
    ensure!(
        locator["map_reference"] == 0x8500_0000u32
            && locator["map_directory_index"] == 0
            && locator["map_hash"] == source_map
            && locator["native_type_index"] == 62
            && number(locator, "trigger_ordinal")? < 65_536
            && number(locator, "schema_index")? < 65_536
            && flags <= u64::from(u32::MAX)
            && (flags & 1 != 0 || file == u64::from(u32::MAX))
            && (flags & 2 != 0 || map == u64::from(u32::MAX)),
        "mission linked-map source identity, flags or schema provenance differ"
    );
    for (field, required) in [
        ("map_address", 0x5c),
        ("trigger_block", 16),
        ("record_address", 0x38),
        ("schema_address", 4),
    ] {
        let address = number(locator, field)?;
        ensure!(
            address >= 0xc8
                && address
                    .checked_add(required)
                    .is_some_and(|end| end <= payload),
            "mission linked-map address is outside the retained resource"
        );
    }
    if let Some((expected_file, expected_map, ordinal, schema)) = expected {
        ensure!(
            file == u64::from(expected_file)
                && map == u64::from(expected_map)
                && locator["trigger_ordinal"] == ordinal
                && locator["schema_index"] == schema
                && flags == 3,
            "mission first linked-map selection differs from source data"
        );
    }
    Ok(())
}

fn validate_unknown(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
    receipt_exists: bool,
) -> Result<()> {
    let message = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    ensure!(
        !success
            && stdout.is_empty()
            && !receipt_exists
            && (message.contains("mission") || message.contains("level"))
            && !message.contains("panicked"),
        "unknown mission did not fail cleanly without emitting a context"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
