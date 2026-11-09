mod actor_action_request;
mod actor_goal_request;
mod actor_target_request;
mod bounds_audit;
mod candidates;
mod collision_audit;
mod frame_audit;
mod frontend;
mod frontend_audio;
mod frontend_cue;
mod frontend_media;
mod frontend_media_audit;
mod frontend_music;
mod frontend_playback;
mod frontend_title;
mod frontend_title_audit;
mod frontend_window;
mod game;
mod geometry_source;
mod goal_request;
mod grenade_ascent;
mod grenade_line;
mod grenade_request;
mod initial_bounds;
mod line_mesh;
mod menu;
mod mesh_audit;
mod mesh_query;
mod mission;
mod navigation;
mod path;
mod placement_audit;
mod portal_audit;
mod request;
mod spatial_audit;
mod startup_resources;
mod static_collect;
mod viewer;
mod world_audit;
mod world_line;
mod world_line_audit;
mod world_query;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ge_assets::{disc::GameDisc, filelist::Filelist, resource::GameAssets};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(version, about = "GoldenEye Wii independent Rust reconstruction tools")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Args)]
struct PlacementArgs {
    /// Instance map placements; fail if any entity is unsupported.
    #[arg(long = "placements")]
    include_placements: bool,
    /// Inspection only: report unsupported entity types and render known placements.
    #[arg(long, requires = "include_placements")]
    allow_incomplete: bool,
}
impl PlacementArgs {
    fn policy(&self) -> ge_assets::scene::PlacementPolicy {
        use ge_assets::scene::PlacementPolicy;
        if !self.include_placements {
            PlacementPolicy::StaticOnly
        } else if self.allow_incomplete {
            PlacementPolicy::AllowUnsupported
        } else {
            PlacementPolicy::RequireAll
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Load and retain a selected mission's metadata; live world/player setup remains incomplete.
    MissionContext {
        #[command(flatten)]
        options: mission::cli::Options,
    },
    /// Check the local title visual program through repeated holds, color loops and its outro.
    FrontendTitleAudit {
        disc: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Decode local startup video/audio and emit bounded metadata/hash diagnostics.
    FrontendMediaAudit {
        #[command(flatten)]
        options: frontend_media_audit::Options,
    },
    /// Open the native main-menu checkpoint using a local user-owned image.
    Play {
        #[command(flatten)]
        options: frontend::PlayOptions,
    },
    /// Bind the grenade's constructed-then-lifted line to authored static worlds; no actor pass.
    GrenadeWorldLine {
        #[command(flatten)]
        options: grenade_line::Options,
    },
    /// Query an authored loaded-world segment through initialized static line geometry.
    CollisionWorldLine {
        #[command(flatten)]
        options: world_line::Options,
    },
    /// Prepare one grenade ascent box and query authored initialized worlds; no later line stages.
    GrenadeAscentBox {
        #[command(flatten)]
        options: grenade_ascent::Options,
    },
    /// Bind an authored loaded-world snapshot through BSP/portals to static box contacts.
    CollisionWorldBox {
        #[command(flatten)]
        options: world_query::Options,
    },
    /// Query initialized static geometry with authored type-three box snapshots.
    CollisionBox {
        #[command(flatten)]
        options: mesh_query::Options,
    },
    /// Collect initialized static-map geometry records; no triangle/contact/clearance result.
    CollisionCollect {
        #[command(flatten)]
        options: static_collect::Options,
    },
    /// Refresh and query one initial static placement hierarchy; no contact/clearance result.
    CollisionCandidates {
        #[command(flatten)]
        options: candidates::Options,
    },
    /// Resolve local grenade prototype metadata and prepare a launch packet; does not spawn a body.
    GrenadeLaunch {
        #[command(flatten)]
        options: grenade_request::Options,
    },
    /// Run one authored target-action snapshot, including angular gate and selected motion/state update.
    NavActorAction {
        #[command(flatten)]
        options: actor_action_request::Options,
    },
    /// Bind authored caller snapshots through distinct owner/instance frames, then run observed actor policies.
    NavActorTarget {
        #[command(flatten)]
        options: actor_target_request::Options,
    },
    /// Search with observed actor goal/edge/heuristic callbacks and explicit raw origin/point inputs.
    NavActorPath {
        #[command(flatten)]
        options: actor_goal_request::Options,
    },
    /// Alternate mesh-local selector with explicit authored acceptable faces and squared point heuristic.
    NavGoalPath {
        #[command(flatten)]
        options: goal_request::Options,
    },
    /// Probe deferred dispatch using static placed navmeshes or an explicit root context.
    NavRequest {
        #[command(flatten)]
        options: request::Options,
    },
    /// Bind world endpoints through an explicit translation frame, then select a default triangle chain.
    NavWorldPath {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash,required_unless_present="index",conflicts_with="index")]
        hash: Option<u32>,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        translation: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        start: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        goal: Vec<f32>,
        /// Also build centroid-seeded greedy waypoints from the original request positions.
        #[arg(long)]
        waypoints: bool,
        /// Host shortcut-test limit in waypoint mode (default 100,000).
        #[arg(long, requires = "waypoints")]
        max_shortcut_tests: Option<usize>,
        /// Host vertex-fan step limit in waypoint mode (default 1,000,000).
        #[arg(long, requires = "waypoints")]
        max_fan_steps: Option<usize>,
        /// Probe an explicit world position against the generated route; does not move an actor.
        #[arg(long, num_args = 3, allow_hyphen_values = true, requires_all = ["waypoints", "corridor_radius"])]
        probe_position: Option<Vec<f32>>,
        /// Explicit corridor radius, squared by the original consumer (zero yields no hit).
        #[arg(long, allow_hyphen_values = true, requires = "probe_position")]
        corridor_radius: Option<f32>,
        /// Limit the corridor scan to the first N segments; zero scans none, absent scans all.
        #[arg(long, requires = "probe_position")]
        corridor_segments: Option<usize>,
        #[arg(long, default_value_t = 100_000)]
        max_expansions: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Select a triangle chain with reconstructed default path costs/policies; not movement waypoints.
    NavPath {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash,required_unless_present="index",conflicts_with="index")]
        hash: Option<u32>,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long)]
        start_triangle: usize,
        #[arg(long)]
        goal_triangle: usize,
        #[arg(long, default_value_t = 100_000)]
        max_expansions: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Audit sampled farthest-reachable triangle paths; no original-runtime comparison.
    ScanPaths {
        disc: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Query a mesh-local navmesh ray; storage-first and nearest have distinct endpoint rules.
    NavRay {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash,required_unless_present="index",conflicts_with="index")]
        hash: Option<u32>,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        origin: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        direction: Vec<f32>,
        #[arg(long, allow_hyphen_values = true)]
        max_t: f32,
        #[arg(long,default_value_t=-1,allow_hyphen_values=true)]
        group: i32,
        #[arg(long, value_enum, default_value_t = navigation::Selection::First)]
        selection: navigation::Selection,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Test a full displacement segment against one navmesh triangle, including both endpoints.
    NavSegment {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash,required_unless_present="index",conflicts_with="index")]
        hash: Option<u32>,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long)]
        triangle: usize,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        origin: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        displacement: Vec<f32>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Depth-first connected-face segment lookup with the observed default policy; not a movement path.
    NavConnectedSegment {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash,required_unless_present="index",conflicts_with="index")]
        hash: Option<u32>,
        #[arg(long)]
        index: Option<usize>,
        #[arg(long)]
        start_triangle: usize,
        #[arg(long)]
        previous_triangle: Option<usize>,
        #[arg(long, allow_hyphen_values = true)]
        depth: i32,
        /// Independent host guard; exhausting it is an error, not a miss.
        #[arg(long, default_value_t = 100_000)]
        face_limit: usize,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        origin: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
        displacement: Vec<f32>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Probe reconstructed surface queries on sampled local navmesh geometry; no reference execution.
    ScanNav {
        disc: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Open a native map inspection window (orbit/pan/zoom; gameplay not implemented).
    ViewMap {
        #[command(flatten)]
        placements: PlacementArgs,
        #[command(flatten)]
        options: viewer::Options,
        disc: PathBuf,
        resource: String,
        #[arg(long,default_value="85000000",value_parser=parse_hash)]
        map: u32,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 720)]
        height: u32,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        eye: Option<Vec<f32>>,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        target: Option<Vec<f32>>,
    },
    /// Decode 0x607 navigation mesh data; original traversal behavior remains unverified.
    EntityGraph {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash, required_unless_present="index", conflicts_with="index")]
        hash: Option<u32>,
        /// Root directory index; explicitly disambiguates duplicated entity hashes.
        #[arg(long)]
        index: Option<usize>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Render actual static map zones through the native GPU to a local PNG (unlit preview).
    RenderMap {
        #[command(flatten)]
        placements: PlacementArgs,
        disc: PathBuf,
        resource: String,
        output: PathBuf,
        #[arg(long,default_value="85000000",value_parser=parse_hash)]
        map: u32,
        #[arg(long, default_value_t = 1280)]
        width: u32,
        #[arg(long, default_value_t = 720)]
        height: u32,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        eye: Option<Vec<f32>>,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        target: Option<Vec<f32>>,
    },
    /// Decode a static map-zone graph and report geometry/material ranges.
    Scene {
        #[command(flatten)]
        placements: PlacementArgs,
        disc: PathBuf,
        resource: String,
        #[arg(long,default_value="85000000",value_parser=parse_hash)]
        map: u32,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Audit all main EDB descriptors/TPL ranges; optionally decode every local image mip.
    ScanEdb {
        disc: PathBuf,
        #[arg(long)]
        decode_textures: bool,
        /// Decode root 0x607 graph bodies and report topology diagnostics separately.
        #[arg(long)]
        /// Also decode root navmeshes and static single-map navigation registrations.
        decode_graphs: bool,
        /// Decode collision/BSP/portal inputs and native initial placement matrices; sample queries.
        #[arg(long)]
        decode_collision: bool,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Inspect checked EDB v263 metadata and texture records by virtual name or hash.
    Edb {
        disc: PathBuf,
        resource: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Decode an EDB texture image/mip to a separate local PNG.
    Texture {
        disc: PathBuf,
        resource: String,
        #[arg(value_parser=parse_hash)]
        hash: u32,
        output: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: usize,
        #[arg(long, default_value_t = 0)]
        mip: u8,
    },
    /// Decode an EngineX v7 archive index from the game image.
    Archive {
        disc: PathBuf,
        #[arg(default_value = "Filelist.bin")]
        index: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Identify the image and inventory the game data partition as JSON.
    Inspect {
        disc: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Read a bounded region of an asset, optionally saving a local probe.
    Probe {
        disc: PathBuf,
        path: String,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long, default_value_t = 256)]
        length: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command.unwrap_or_else(|| Command::Play {
        options: frontend::PlayOptions::default(),
    }) {
        Command::Play { options } => frontend::run(options)?,
        Command::MissionContext { options } => mission::cli::run(options)?,
        Command::FrontendTitleAudit { disc, output } => {
            let mut assets = GameAssets::open(&disc, "Filelist.bin")?;
            anyhow::ensure!(
                assets.disc.identity.game_id == "SJBE52" && assets.disc.identity.revision == 0,
                "title audit requires SJBE52 revision zero"
            );
            let edb = assets.load_edb("fe_main.edb")?;
            let report = frontend_title_audit::audit(&edb)?;
            let json = serde_json::to_vec_pretty(&report)?;
            write_new(&output, &json)?;
            io::stdout().lock().write_all(&json)?;
        }
        Command::FrontendMediaAudit { options } => {
            let report = frontend_media_audit::run(&options)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::GrenadeWorldLine { options } => {
            let json = serde_json::to_vec_pretty(&grenade_line::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::CollisionWorldLine { options } => {
            let json = serde_json::to_vec_pretty(&world_line::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::GrenadeAscentBox { options } => {
            let json = serde_json::to_vec_pretty(&grenade_ascent::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::CollisionWorldBox { options } => {
            let json = serde_json::to_vec_pretty(&world_query::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::GrenadeLaunch { options } => {
            let json = serde_json::to_vec_pretty(&grenade_request::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::CollisionCandidates { options } => {
            let json = serde_json::to_vec_pretty(&candidates::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::CollisionCollect { options } => {
            let json = serde_json::to_vec_pretty(&static_collect::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::CollisionBox { options } => {
            let json = serde_json::to_vec_pretty(&mesh_query::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavActorAction { options } => {
            let json = serde_json::to_vec_pretty(&actor_action_request::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavActorTarget { options } => {
            let json = serde_json::to_vec_pretty(&actor_target_request::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavActorPath { options } => {
            let json = serde_json::to_vec_pretty(&actor_goal_request::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavGoalPath { options } => {
            let json = serde_json::to_vec_pretty(&goal_request::run(&options)?)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavRequest { options } => {
            let result = request::run(&options)?;
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = options.output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavWorldPath {
            disc,
            resource,
            hash,
            index,
            translation,
            start,
            goal,
            waypoints,
            max_shortcut_tests,
            max_fan_steps,
            probe_position,
            corridor_radius,
            corridor_segments,
            max_expansions,
            output,
        } => {
            anyhow::ensure!(max_expansions > 0, "--max-expansions must be positive");
            let waypoint_limits = ge_core::navigation::WaypointLimits {
                max_shortcut_tests: max_shortcut_tests.unwrap_or(100_000),
                max_fan_steps: max_fan_steps.unwrap_or(1_000_000),
            };
            anyhow::ensure!(
                waypoint_limits.max_shortcut_tests > 0 && waypoint_limits.max_fan_steps > 0,
                "waypoint work limits must be positive"
            );
            let translation: [f32; 3] = translation
                .try_into()
                .map_err(|_| anyhow::anyhow!("translation requires three coordinates"))?;
            let start: [f32; 3] = start
                .try_into()
                .map_err(|_| anyhow::anyhow!("start requires three coordinates"))?;
            let goal: [f32; 3] = goal
                .try_into()
                .map_err(|_| anyhow::anyhow!("goal requires three coordinates"))?;
            let request = ge_core::navigation::WorldPathRequest {
                frame: ge_core::navigation::NavigationFrame { translation },
                start,
                goal,
                max_expansions,
            };
            let probe = probe_position
                .map(|p| -> Result<([f32; 3], f32)> {
                    let position: [f32; 3] = p.try_into().map_err(|_| {
                        anyhow::anyhow!("probe position requires three coordinates")
                    })?;
                    let radius = corridor_radius.context("probe requires corridor radius")?;
                    anyhow::ensure!(
                        position.into_iter().all(f32::is_finite),
                        "probe position must be finite"
                    );
                    anyhow::ensure!(
                        radius.is_finite() && (radius * radius).is_finite(),
                        "corridor radius square must be finite"
                    );
                    Ok((position, radius))
                })
                .transpose()?;
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let data = path::PathData::new(&graph)?;
            let selected = data.world_query(request)?;
            let generated = if waypoints {
                selected
                    .path_search
                    .as_ref()
                    .and_then(|s| s.path.as_ref())
                    .map(|p| data.waypoints(request, p, waypoint_limits))
                    .transpose()?
            } else {
                None
            };
            let waypoint_probe = match (probe, generated.as_ref()) {
                (Some((position, radius)), Some(points)) => {
                    let route = ge_core::navigation::WaypointRoute::new(&points.world_points)?;
                    Some(
                        serde_json::json!({"position":position,"radius":radius,"max_segments":corridor_segments,
                        "arrived":route.arrived(position)?,"corridor_segment":route.corridor_segment(position,radius,corridor_segments)?}),
                    )
                }
                _ => None,
            };
            let result = serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"request":request,"result":selected,
                "waypoints_requested":waypoints,"waypoint_limits":waypoints.then_some(waypoint_limits),"waypoints":generated,"reference_compared":false,
                "waypoint_probe_request":probe.map(|(position,radius)|serde_json::json!({"position":position,"radius":radius,"max_segments":corridor_segments})),"waypoint_probe":waypoint_probe,
                "scope":"Translation-only raised downward endpoint binding and default face-chain search; optional centroid-seeded greedy waypoints use original unraised request positions, face flags and vertex-fan clearance. Read-only world route target/length and optional arrival/corridor probes do not advance points or move actors. Frame is explicit; no placement/rotation inference, actor snapping/movement or original-runtime parity. Wii arithmetic, fourth-component state and actor overrides remain unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavPath {
            disc,
            resource,
            hash,
            index,
            start_triangle,
            goal_triangle,
            max_expansions,
            output,
        } => {
            anyhow::ensure!(max_expansions > 0, "--max-expansions must be positive");
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let request = ge_core::navigation::PathSearch {
                start_triangle,
                goal_triangle,
                max_expansions,
            };
            let selected = path::PathData::new(&graph)?.query(request)?;
            let result = serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"request":request,"result":selected,"reference_compared":false,
                "scope":"Triangle chain from static SJBE52 r0 default search contract. Equal-g replacement, FIFO score ties and logical 256-slot pool reuse; not movement waypoints or original-runtime parity. Wii estimate arithmetic, whole-block heap-address reuse, self-edge expansion and actor overrides remain unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::ScanPaths { disc, output } => {
            let result = path::scan(&disc)?;
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
            anyhow::ensure!(
                result["errors"].as_array().is_some_and(Vec::is_empty),
                "path audit found errors; inspect JSON"
            );
        }
        Command::NavRay {
            disc,
            resource,
            hash,
            index,
            origin,
            direction,
            max_t,
            group,
            selection,
            output,
        } => {
            let origin: [f32; 3] = origin
                .try_into()
                .map_err(|_| anyhow::anyhow!("origin requires three coordinates"))?;
            let direction: [f32; 3] = direction
                .try_into()
                .map_err(|_| anyhow::anyhow!("direction requires three coordinates"))?;
            let ray = ge_core::navigation::SurfaceRay {
                origin,
                direction,
                max_parameter: max_t,
            };
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let data = navigation::SurfaceData::new(&graph)?;
            let hit = data.query(ray, group, selection)?;
            let result = serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"ray":ray,"group":group,"selection":selection,"hit":hit,"reference_compared":false,
                "scope":"Mesh-local surface ray from static SJBE52 r0 contracts. First: storage order, 0<t<maximum. Nearest: least nonnegative t below maximum, storage-order ties. Not pathfinding, world collision or original-runtime parity. Wii estimate precision remains unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavSegment {
            disc,
            resource,
            hash,
            index,
            triangle,
            origin,
            displacement,
            output,
        } => {
            let origin: [f32; 3] = origin
                .try_into()
                .map_err(|_| anyhow::anyhow!("origin requires three coordinates"))?;
            let displacement: [f32; 3] = displacement
                .try_into()
                .map_err(|_| anyhow::anyhow!("displacement requires three coordinates"))?;
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let hit =
                navigation::SurfaceData::new(&graph)?.segment(origin, displacement, triangle)?;
            let result = serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"origin":origin,"displacement":displacement,"triangle":triangle,"hit":hit,"reference_compared":false,
                "scope":"Mesh-local selected-triangle segment query, inclusive 0<=t<=1. Displacement is the full vector, not an endpoint. Static SJBE52 r0 contract; Wii estimate precision and original execution remain unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::NavConnectedSegment {
            disc,
            resource,
            hash,
            index,
            start_triangle,
            previous_triangle,
            depth,
            face_limit,
            origin,
            displacement,
            output,
        } => {
            anyhow::ensure!(face_limit > 0, "--face-limit must be positive");
            let origin: [f32; 3] = origin
                .try_into()
                .map_err(|_| anyhow::anyhow!("origin requires three coordinates"))?;
            let displacement: [f32; 3] = displacement
                .try_into()
                .map_err(|_| anyhow::anyhow!("displacement requires three coordinates"))?;
            let request = ge_core::navigation::SegmentSearch {
                origin,
                displacement,
                start_triangle,
                previous_triangle,
                max_depth: depth,
                max_face_tests: face_limit,
            };
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let found = navigation::SurfaceData::new(&graph)?.connected_segment(request)?;
            let result = serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,
                "request":request,"result":found,"reference_compared":false,
                "scope":"Mesh-local current-face-first, slot-ordered depth-first segment lookup. Default policy prevents immediate backtracking only; no global visited set. Not nearest selection, a movement path or original-runtime parity. Host budget exhaustion is an error. Wii arithmetic, custom actor policies and world transforms remain unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::ScanNav { disc, output } => {
            let mut assets = GameAssets::open(&disc, "Filelist.bin")?;
            let resources: Vec<String> = assets
                .index
                .entries
                .iter()
                .filter(|e| e.path.to_ascii_lowercase().ends_with(".edb"))
                .map(|e| e.path.clone())
                .collect();
            let mut graphs = 0;
            let mut probed = 0;
            let mut stats = navigation::ProbeStats::default();
            let mut errors = Vec::new();
            for (i, name) in resources.iter().enumerate() {
                if i % 100 == 0 {
                    eprintln!("Navigation audit {i}/{}", resources.len());
                }
                let result = (|| -> Result<()> {
                    let edb = assets.load_edb(name)?;
                    for (index, entity) in edb.entities()?.into_iter().enumerate() {
                        if entity.entity_type != 0x607 {
                            continue;
                        }
                        graphs += 1;
                        let graph = edb.entity_graph_at(index)?;
                        let probe = navigation::SurfaceData::new(&graph)?.probe()?;
                        stats.include(probe);
                        probed += 1;
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    errors.push(serde_json::json!({"resource":name,"error":format!("{error:#}")}));
                }
            }
            let result = serde_json::json!({"disc":assets.disc.identity,"resources":resources.len(),"graphs":graphs,"probed":probed,"samples_per_graph_limit":32,"stats":stats,"errors":errors,"reference_compared":false,
                "scope":"Sampled mesh-local first/nearest rays, selected-face segments and depth-one connected-face queries using the observed default policy, with independent f64 geometry checks. Static contract smoke only; original-runtime parity, Wii estimate precision, custom actor policies, world transforms and gameplay unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
            } else {
                io::stdout().lock().write_all(&json)?;
            }
            anyhow::ensure!(
                errors.is_empty(),
                "navigation probe found {} errors; inspect JSON",
                errors.len()
            );
        }
        Command::ViewMap {
            placements,
            options,
            disc,
            resource,
            map,
            width,
            height,
            eye,
            target,
        } => {
            options.validate()?;
            let mut assets = GameAssets::open(&disc, "Filelist.bin")?;
            let edb = assets.load_edb(&resource)?;
            let scene = edb.map_scene_with_placements(map, placements.policy())?;
            let materials = scene_materials(&edb, &scene)?;
            let camera = inspection_camera(&scene, eye, target)?;
            eprintln!(
                "Native asset inspection; gameplay not implemented. Left-drag orbit, right/middle-drag pan, wheel zoom, Home reset, Esc close."
            );
            let viewed = viewer::run(&scene, &materials, camera, [width, height], &options)?;
            if let (Some(path), Some(image)) = (&options.capture, &viewed.image) {
                write_new(path, &encode_png(image)?)?;
            }
            let result = serde_json::json!({"window":viewed.evidence,"disc":assets.disc.identity,"resource_hash":edb.header.hash,"map_hash":map,
                "placements_requested":placements.include_placements,"allow_incomplete":placements.allow_incomplete,"placements_loaded":scene.placements_loaded,"render_placements":scene.render_placements,"navigation_placements":scene.navigation_placements,"nav_meshes":scene.nav_meshes.len(),"placements_not_loaded":scene.placements_not_loaded});
            io::stdout()
                .lock()
                .write_all(&serde_json::to_vec_pretty(&result)?)?;
        }
        Command::EntityGraph {
            disc,
            resource,
            hash,
            index,
            output,
        } => {
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let graph = if let Some(index) = index {
                edb.entity_graph_at(index)?
            } else {
                edb.entity_graph(hash.context("entity hash required")?)?
            };
            let hash = graph.entity.hash;
            let result = serde_json::json!({"resource_hash":edb.header.hash,"graph":graph,
                "scope":"Observed 0x607 navigation mesh data only. Class identity is established for SJBE52 revision 0; original traversal, agent clearance, collision and reference parity remain unverified."});
            let json = serde_json::to_vec_pretty(&result)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "{resource} entity {hash:#010x}: {} vertices, {} triangles, {} links, {} components; {}",
                    graph.vertices.len(),
                    graph.triangles.len(),
                    graph.diagnostics.directed_links,
                    graph.diagnostics.connected_components,
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::RenderMap {
            placements,
            disc,
            resource,
            output,
            map,
            width,
            height,
            eye,
            target,
        } => {
            let mut assets = GameAssets::open(&disc, "Filelist.bin")?;
            let edb = assets.load_edb(&resource)?;
            let scene = edb.map_scene_with_placements(map, placements.policy())?;
            let materials = scene_materials(&edb, &scene)?;
            let camera = inspection_camera(&scene, eye, target)?;
            let rendered =
                pollster::block_on(ge_render::render(&scene, &materials, camera, width, height))?;
            write_new(&output, &encode_png(&rendered.image)?)?;
            let result = serde_json::json!({"render":rendered.evidence,"disc":assets.disc.identity,"resource_hash":edb.header.hash,"map_hash":map,"zones":scene.zones.len(),"meshes":scene.geometry.meshes,"placements_requested":placements.include_placements,"allow_incomplete":placements.allow_incomplete,"placements_loaded":scene.placements_loaded,"render_placements":scene.render_placements,"navigation_placements":scene.navigation_placements,"nav_meshes":scene.nav_meshes.len(),"placements_not_loaded":scene.placements_not_loaded,"unsupported_placements":scene.placements.iter().filter(|p|matches!(p.status,Some(ge_assets::scene::PlacementState::Unsupported{..}))).collect::<Vec<_>>(),"placement_transform_convention":ge_assets::transform::PLACEMENT_CONVENTION,"eye":camera.eye,"target":camera.target,"rgba_sha256":format!("{:x}",Sha256::digest(&rendered.image.pixels))});
            io::stdout()
                .lock()
                .write_all(&serde_json::to_vec_pretty(&result)?)?;
        }
        Command::Scene {
            placements,
            disc,
            resource,
            map,
            output,
        } => {
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let scene = edb.map_scene_with_placements(map, placements.policy())?;
            let mut summary = serde_json::to_value(&scene)?;
            summary["vertex_count"] = serde_json::json!(scene.vertex_count());
            summary["triangle_count"] = serde_json::json!(scene.triangle_count());
            summary["scope"] = serde_json::json!(
                "Map inspection geometry; requested placements use a declared transform convention. Visibility/rotation parity, unsupported entities, actors, skies, effects, collision and reference parity remain unverified."
            );
            let json = serde_json::to_vec_pretty(&summary)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "{resource}: {} zones, {} meshes, {} vertices, {} triangles, {} placements not loaded; {}",
                    scene.zones.len(),
                    scene.geometry.meshes,
                    scene.vertex_count(),
                    scene.triangle_count(),
                    scene.placements_not_loaded,
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::ScanEdb {
            disc,
            decode_textures,
            decode_graphs,
            decode_collision,
            output,
        } => {
            let mut assets = GameAssets::open(&disc, "Filelist.bin")?;
            let resources: Vec<String> = assets
                .index
                .entries
                .iter()
                .filter(|e| e.path.to_ascii_lowercase().ends_with(".edb"))
                .map(|e| e.path.clone())
                .collect();
            let mut parsed = 0;
            let mut textures = 0;
            let mut external = 0;
            let mut images = 0;
            let mut decoded_mips = 0;
            let mut errors = Vec::new();
            let mut formats = std::collections::BTreeMap::<u32, usize>::new();
            let mut versions = std::collections::BTreeMap::<u32, usize>::new();
            let mut decoded_digest = Sha256::new();
            let mut graphs = GraphAudit::default();
            let mut collision = collision_audit::Audit::default();
            let mut map_spatial = spatial_audit::Audit::default();
            let mut initial_placements = placement_audit::Audit::default();
            let mut collision_frames = frame_audit::Audit::default();
            let mut initial_bounds = bounds_audit::Audit::default();
            let mut candidate_hierarchies = candidates::Audit::default();
            let mut static_geometry = static_collect::Audit::default();
            let mut static_mesh_contacts = mesh_audit::Audit::default();
            let mut world_box_requests = world_audit::Audit::default();
            let mut world_line_requests = world_line_audit::Audit::default();
            let mut map_navigation = MapNavigationAudit::default();
            let mut grenade_prototype = None;
            let mut entity_types = std::collections::BTreeMap::<String, usize>::new();
            for (index, name) in resources.iter().enumerate() {
                if index % 100 == 0 {
                    eprintln!("EDB audit {index}/{}", resources.len());
                }
                let result = (|| -> Result<()> {
                    let edb = assets.load_edb(name)?;
                    *versions.entry(edb.header.version).or_default() += 1;
                    parsed += 1;
                    if edb.header.hash == ge_core::grenade::GRENADE_RESOURCE {
                        grenade_prototype = Some(
                            edb.animation_script_reference(
                                ge_core::grenade::GRENADE_SCRIPT_REFERENCE,
                            )?
                            .context("missing grenade script prototype reference")?,
                        );
                    }
                    textures += edb.textures.len();
                    if decode_graphs || decode_collision {
                        for (index, entity) in edb.entities()?.into_iter().enumerate() {
                            *entity_types
                                .entry(format!("0x{:x}", entity.entity_type))
                                .or_default() += 1;
                            if decode_collision && entity.entity_type == 0x601 {
                                collision.found += 1;
                                match edb.collision_mesh_at(index) {
                                    Ok(mesh)=>collision.include(&mesh)?,
                                    Err(error)=>errors.push(serde_json::json!({"resource":name,"entity":entity.hash,"collision_error":format!("{error:#}")})),
                                }
                            }
                            if !decode_graphs || entity.entity_type != 0x607 {
                                continue;
                            }
                            graphs.found += 1;
                            match edb.entity_graph_at(index) {
                                Ok(graph) => graphs.include(&graph),
                                Err(error) => errors.push(serde_json::json!({"resource":name,"entity":entity.hash,"error":format!("{error:#}")})),
                            }
                        }
                    }
                    if decode_graphs || decode_collision {
                        for map in edb.map_hashes()? {
                            if decode_collision {
                                let placements = edb.map_placements(map)?;
                                initial_bounds.include(&edb, &placements).with_context(|| {
                                    format!("map {map:08x} initial base bounds")
                                })?;
                                collision_frames.include(&placements).with_context(|| {
                                    format!("map {map:08x} authored collision frames")
                                })?;
                                initial_placements.include(&placements).with_context(|| {
                                    format!("map {map:08x} initial placement matrices")
                                })?;
                                map_spatial.maps += 1;
                                match edb.map_spatial(map) {
                                    Ok(spatial) => {
                                        let portals=edb.map_portals(map).with_context(||format!("map {map:08x} portal inputs"))?;
                                        map_spatial.include(&spatial,&portals).with_context(|| format!("map {map:08x} BSP/portal probes"))?;
                                        candidate_hierarchies.include(&edb,map,&placements,&spatial).with_context(||format!("map {map:08x} initialized candidate hierarchies"))?;
                                        static_geometry.include(&edb,map,&spatial).with_context(||format!("map {map:08x} initialized geometry collection"))?;
                                        static_mesh_contacts.include(&edb,map,&spatial).with_context(||format!("map {map:08x} initialized static mesh contacts"))?;
                                        world_box_requests.include(&edb,map,&spatial).with_context(||format!("map {map:08x} initialized world box requests"))?;
                                        world_line_requests.include(&edb,map,&spatial).with_context(||format!("map {map:08x} initialized world line requests"))?;
                                    },
                                    Err(error) => errors.push(serde_json::json!({"resource":name,"map":map,"spatial_error":format!("{error:#}")})),
                                }
                            }
                            if !decode_graphs {
                                continue;
                            }
                            map_navigation.maps += 1;
                            match edb.map_navigation(map) {
                                Ok(bindings)=>map_navigation.include(&bindings),
                                Err(error)=>errors.push(serde_json::json!({"resource":name,"map":map,"error":format!("{error:#}")})),
                            }
                        }
                    }
                    for texture in &edb.textures {
                        let result = (|| -> Result<()> {
                            let info = edb.texture_info(texture.hash)?;
                            if info.external_file.is_some() {
                                external += 1;
                                return Ok(());
                            }
                            for frame in 0..info.frames.len() {
                                let tpl = edb.texture_tpl(texture.hash, frame)?;
                                images += 1;
                                *formats.entry(tpl.images[0].format).or_default() += 1;
                                if decode_textures {
                                    for mip in 0..=tpl.images[0].max_lod {
                                        let image = tpl.decode(0, mip)?;
                                        decoded_digest.update(edb.header.hash.to_be_bytes());
                                        decoded_digest.update(texture.hash.to_be_bytes());
                                        decoded_digest.update((frame as u32).to_be_bytes());
                                        decoded_digest.update([mip]);
                                        decoded_digest.update(image.width.to_be_bytes());
                                        decoded_digest.update(image.height.to_be_bytes());
                                        decoded_digest.update(&image.pixels);
                                        decoded_mips += 1;
                                    }
                                }
                            }
                            Ok(())
                        })();
                        if let Err(error) = result {
                            errors.push(serde_json::json!({"resource":name,"texture":texture.hash,"error":format!("{error:#}")}));
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    errors.push(serde_json::json!({"resource":name,"error":format!("{error:#}")}));
                }
            }
            let summary = serde_json::json!({"disc":assets.disc.identity,"resources":resources.len(),"parsed":parsed,"textures":textures,"external_references":external,"local_images":images,"decoded_mips":decoded_mips,"decoded_pixels":decode_textures,"gx_formats":formats,"edb_versions":versions,"decoded_rgba_sha256":decode_textures.then(||format!("{:x}",decoded_digest.finalize())),"graphs":decode_graphs.then_some(&graphs),"root_entity_types":decode_graphs.then_some(&entity_types),"errors":errors,
                "map_navigation":decode_graphs.then_some(&map_navigation),
                "grenade_prototype":grenade_prototype,
                "collision":decode_collision.then_some(&collision),
                "map_spatial":decode_collision.then_some(&map_spatial),
                "initial_placements":decode_collision.then_some(&initial_placements),
                "collision_frames":decode_collision.then_some(&collision_frames),
                "initial_bounds":decode_collision.then_some(&initial_bounds),
                "candidate_hierarchies":decode_collision.then_some(&candidate_hierarchies),
                "static_geometry":decode_collision.then_some(&static_geometry),
                "static_mesh_contacts":decode_collision.then_some(&static_mesh_contacts),
                "world_box_requests":decode_collision.then_some(&world_box_requests),
                "world_line_requests":decode_collision.then_some(&world_line_requests),
                "scope":"v262/v263 headers, texture metadata/local TPL ranges, optional root 0x607 graphs, static navigation registrations, root 0x601 collision inputs and map BSP/portal inputs. Initial placement matrices use statically reconstructed Euler order six and T*S*Rz*Rx*Ry. Both authored inverse modes retain separate f64 probes and provisional host reciprocal seeds. Initial base/component/slot-six bounds and seeded hierarchy refresh are reconstructed without changing serialized membership flags. Candidate queries have independent f64 list diagnostics with explicit boundary ambiguity. Separate authored zone-center queries collect recursive static geometry records and prepare consecutive-cache mesh origins. Separate authored unit-box/identity-frame zone-center probes traverse initialized static meshes through batch caches, supported coarse trees and ordered triangle/primitive contacts. Appended contacts have finite-value and source triangle/material provenance checks; these are not independent contact-truth or original-reference comparisons. Additional authored world requests bind paired BSP selection and optional ordered portal expansion to one shared record/cache/contact sequence, with direct single-zone and expanded record-cohort comparisons. The separate grenade ascent command writes the confirmed source request settings and reports contact-count blocking only for that first stage. Separate full-map line probes use clipped unique BSP zones, line-specific candidate gates, transpose placement frames, resident plane/batch caches, nearest/any-hit comparisons and source-provenance checks. F64 raw-plane distances are diagnostics, not reference tolerances. Live membership/mutation, dynamic positions, actor-list line clearance, Wii FP estimates and full gameplay parity remain unverified."});
            let json = serde_json::to_vec_pretty(&summary)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "EDB audit: {parsed}/{} metadata parsed; {textures} textures; {images} local images; {} errors; {}",
                    resources.len(),
                    errors.len(),
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
            anyhow::ensure!(
                errors.is_empty(),
                "EDB audit found {} errors; inspect the JSON report",
                errors.len()
            );
        }
        Command::Edb {
            disc,
            resource,
            output,
        } => {
            let edb = GameAssets::open(&disc, "Filelist.bin")?.load_edb(&resource)?;
            let json = serde_json::to_vec_pretty(&edb)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "{resource}: {} payload bytes, {} base bytes, {} textures; {}",
                    edb.header.payload_bytes,
                    edb.header.base_bytes,
                    edb.textures.len(),
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::Texture {
            disc,
            resource,
            hash,
            output,
            frame,
            mip,
        } => {
            let image = GameAssets::open(&disc, "Filelist.bin")?
                .decode_texture(&resource, hash, frame, mip)?;
            write_new(&output, &encode_png(&image)?)?;
            println!(
                "{resource} texture {hash:#010x} frame {frame} mip {mip}: {}x{}, RGBA SHA-256 {:x}; {}",
                image.width,
                image.height,
                Sha256::digest(&image.pixels),
                output.display()
            );
        }
        Command::Archive {
            disc,
            index,
            output,
        } => {
            let mut disc = GameDisc::open(&disc)?;
            let size = disc
                .files
                .iter()
                .find(|f| f.path.eq_ignore_ascii_case(&index))
                .context("index not found")?
                .bytes;
            let filelist = Filelist::parse(&disc.read(&index, 0, size.try_into()?)?)?;
            filelist.validate_locations(&index, &disc.files)?;
            let json = serde_json::to_vec_pretty(&filelist)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "{index}: {} entries, all locations in bounds; {}",
                    filelist.entries.len(),
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::Inspect { disc, output } => {
            let inventory = GameDisc::open(&disc)?.inventory();
            let json = serde_json::to_vec_pretty(&inventory)?;
            if let Some(path) = output {
                write_new(&path, &json)?;
                println!(
                    "{} {}: {} files ({} bytes); inventory: {}",
                    inventory.disc.game_id,
                    inventory.disc.title,
                    inventory.file_count,
                    inventory.file_bytes,
                    path.display()
                );
            } else {
                io::stdout().lock().write_all(&json)?;
            }
        }
        Command::Probe {
            disc,
            path,
            offset,
            length,
            output,
        } => {
            let data = GameDisc::open(&disc)?.read(&path, offset, length)?;
            println!(
                "{path} at {offset:#x}: {} bytes, SHA-256 {:x}",
                data.len(),
                Sha256::digest(&data)
            );
            if let Some(path) = output {
                write_new(&path, &data)?;
            } else {
                for (row, bytes) in data.chunks(16).enumerate() {
                    let hex = bytes
                        .iter()
                        .map(|b| format!("{b:02x} "))
                        .collect::<String>();
                    let text = bytes
                        .iter()
                        .map(|b| {
                            if b.is_ascii_graphic() || *b == b' ' {
                                *b as char
                            } else {
                                '.'
                            }
                        })
                        .collect::<String>();
                    println!("{:08x}  {hex:48} {text}", offset + (row * 16) as u64);
                }
            }
        }
    }
    Ok(())
}

fn scene_materials(
    edb: &ge_assets::edb::Edb,
    scene: &ge_assets::scene::Scene,
) -> Result<std::collections::BTreeMap<usize, ge_render::MaterialImage>> {
    let mut materials = std::collections::BTreeMap::new();
    let used = scene
        .geometry
        .batches
        .iter()
        .map(|b| b.binding_index)
        .collect::<std::collections::BTreeSet<_>>();
    for binding in used {
        let texture_hash = scene.geometry.texture_bindings[binding];
        let tpl = edb.texture_tpl(texture_hash, 0)?;
        let image = tpl.decode(0, 0)?;
        materials.insert(
            binding,
            ge_render::MaterialImage {
                image,
                wrap_s: tpl.images[0].wrap_s,
                wrap_t: tpl.images[0].wrap_t,
            },
        );
    }
    Ok(materials)
}
fn inspection_camera(
    scene: &ge_assets::scene::Scene,
    eye: Option<Vec<f32>>,
    target: Option<Vec<f32>>,
) -> Result<ge_render::camera::Camera> {
    let mut camera = ge_render::camera::Camera::fit(scene.geometry.bounds)?;
    if let Some(eye) = eye {
        camera.eye = eye
            .try_into()
            .map_err(|_| anyhow::anyhow!("eye requires three coordinates"))?;
    }
    if let Some(target) = target {
        camera.target = target
            .try_into()
            .map_err(|_| anyhow::anyhow!("target requires three coordinates"))?;
    }
    camera.matrix(1.)?;
    Ok(camera)
}

#[derive(Default, serde::Serialize)]
struct GraphAudit {
    found: usize,
    decoded: usize,
    vertices: usize,
    triangles: usize,
    directed_links: usize,
    self_links: usize,
    neighbors_after_boundary: usize,
    boundary_slots: usize,
    connected_components: usize,
    centroid_samples: usize,
    distance_weight_samples: usize,
    flag_samples: usize,
    shortcut_blocked_faces: usize,
    normal_samples: usize,
    low_actor_normal_faces: usize,
    non_reciprocal_links: usize,
    links_without_shared_edge: usize,
    degenerate_triangles: usize,
    max_centroid_error: f64,
    max_distance_weight_error: f64,
}
impl GraphAudit {
    fn include(&mut self, graph: &ge_assets::graph::TriangleGraph) {
        self.decoded += 1;
        self.vertices += graph.vertices.len();
        self.triangles += graph.triangles.len();
        let d = &graph.diagnostics;
        self.directed_links += d.directed_links;
        self.self_links += d.self_links;
        self.neighbors_after_boundary += d.neighbors_after_boundary;
        self.boundary_slots += d.boundary_slots;
        self.connected_components += d.connected_components;
        self.centroid_samples += d.centroid_samples;
        self.distance_weight_samples += d.distance_weight_samples;
        self.flag_samples += d.flag_samples;
        self.shortcut_blocked_faces += d.shortcut_blocked_faces;
        self.normal_samples += d.normal_samples;
        self.low_actor_normal_faces += d.low_actor_normal_faces;
        self.non_reciprocal_links += d.non_reciprocal_links;
        self.links_without_shared_edge += d.links_without_shared_edge;
        self.degenerate_triangles += d.degenerate_triangles;
        self.max_centroid_error = self.max_centroid_error.max(d.max_centroid_error);
        self.max_distance_weight_error = self
            .max_distance_weight_error
            .max(d.max_distance_weight_error);
    }
}

#[derive(Default, serde::Serialize)]
struct MapNavigationAudit {
    maps: usize,
    decoded: usize,
    placements: usize,
    registrations: usize,
    ready: usize,
    unavailable: usize,
    unresolved_entity_references: usize,
    nonzero_translations: usize,
    shared_meshes: usize,
}
impl MapNavigationAudit {
    fn include(&mut self, map: &ge_assets::navigation::MapNavigation) {
        self.decoded += 1;
        self.placements += map.placements;
        self.registrations += map.registrations.len();
        self.unresolved_entity_references += map.unresolved_entity_references;
        let mut meshes = std::collections::BTreeSet::new();
        for r in &map.registrations {
            if let ge_assets::navigation::NavigationBinding::Ready {
                entity_index,
                translation,
                ..
            } = r.binding
            {
                self.ready += 1;
                self.nonzero_translations += usize::from(translation != [0.; 3]);
                meshes.insert(entity_index);
            } else {
                self.unavailable += 1;
            }
        }
        self.shared_meshes += meshes.len();
    }
}

fn parse_hash(text: &str) -> std::result::Result<u32, String> {
    u32::from_str_radix(text.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
}

fn encode_png(image: &ge_assets::tpl::RgbaImage) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    let mut encoder = png::Encoder::new(&mut encoded, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.pixels)?;
    writer.finish()?;
    Ok(encoded)
}

fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating {} (existing files are preserved)", path.display()))?
        .write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/output.rs"]
mod tests;
