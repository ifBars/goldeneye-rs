//! Explicit authored goal policy for the alternate mesh-local selector; no actor behavior is inferred.
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::navigation::GoalPathSearch;
use std::path::PathBuf;
#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    #[arg(value_parser=crate::parse_hash,required_unless_present="index",conflicts_with="index")]
    hash: Option<u32>,
    #[arg(long)]
    index: Option<usize>,
    #[arg(long)]
    start_triangle: usize,
    /// Authored acceptable face IDs. Repeat to admit several faces; this is not an original actor policy.
    #[arg(long = "accept-triangle", required = true)]
    accepted: Vec<usize>,
    /// Explicit mesh-local point for the observed squared 3D actor heuristic.
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    heuristic_point: Vec<f32>,
    #[arg(long, default_value_t = 100_000)]
    max_expansions: usize,
    /// Apply the observed actor target-directed edge predicate using the explicit heuristic point.
    #[arg(long)]
    actor_edges: bool,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.max_expansions > 0,
        "--max-expansions must be positive"
    );
    let point: [f32; 3] = options
        .heuristic_point
        .as_slice()
        .try_into()
        .context("expected three heuristic point coordinates")?;
    ensure!(
        point.into_iter().all(f32::is_finite),
        "heuristic point must be finite"
    );
    let edb = GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(&options.resource)?;
    let graph = if let Some(index) = options.index {
        edb.entity_graph_at(index)?
    } else {
        edb.entity_graph(options.hash.context("entity hash required")?)?
    };
    let request = GoalPathSearch {
        start_triangle: options.start_triangle,
        max_expansions: options.max_expansions,
    };
    let result = crate::path::PathData::new(&graph)?.query_goal(
        request,
        &options.accepted,
        point,
        options.actor_edges,
    )?;
    Ok(
        serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"request":request,"goal_policy":{"kind":"authored_face_set","accepted":options.accepted},"heuristic_point":point,"heuristic":"observed_squared_3d_distance","edge_policy":if options.actor_edges {"actor_target_direction"}else{"default_immediate_backtrack"},"edge_cost":"stored_slot_weight_or_center_distance","result":result,"reference_compared":false,
        "scope":"Alternate mesh-local search reconstructed from static evidence. Goals are tested on queue removal; heuristic has no destination face. Accepted faces/point are authored inputs. Optional actor edges apply observed flag/stored-normal/direction gates; default edges reject immediate backtracking only. FIFO ties, equal-g replacement/reopening and logical pool reuse are shared with explicit-face search. Full actor goals, actual point sources, action selection, Wii normalization/FP state and gameplay remain unverified."}),
    )
}
