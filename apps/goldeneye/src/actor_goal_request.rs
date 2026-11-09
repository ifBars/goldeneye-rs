//! Explicit diagnostic inputs for the observed actor path policy.
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
    /// Raw policy origin, not a world point transformed by this command.
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    origin: Vec<f32>,
    /// Raw policy target for candidate distance, squared heuristic and direction gates.
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    point: Vec<f32>,
    #[arg(long, default_value_t = 100_000)]
    max_expansions: usize,
    /// Separate host face-walk budget per goal callback; exhaustion is an error.
    #[arg(long, default_value_t = 100_000)]
    max_goal_steps: usize,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.max_expansions > 0 && options.max_goal_steps > 0,
        "actor search budgets must be positive"
    );
    let origin: [f32; 3] = options
        .origin
        .as_slice()
        .try_into()
        .context("expected three origin coordinates")?;
    let point: [f32; 3] = options
        .point
        .as_slice()
        .try_into()
        .context("expected three point coordinates")?;
    ensure!(
        origin.into_iter().chain(point).all(f32::is_finite),
        "actor policy origin/point must be finite"
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
    let result = crate::path::PathData::new(&graph)?.query_actor_path(
        request,
        origin,
        point,
        options.max_goal_steps,
    )?;
    Ok(
        serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"request":request,"origin":origin,"point":point,"max_goal_steps":options.max_goal_steps,"goal_policy":"observed_actor_proximity_face_walk","edge_policy":"actor_target_direction","heuristic":"observed_squared_3d_distance","result":result,"reference_compared":false,
        "scope":"Observed actor goal, heuristic and edge callbacks over explicit raw origin/point/start-face inputs. Goal tests occur on queue removal. Missing crossings and host budgets report errors. Original actor input selection/storage, scheduling, world binding for this caller, numeric/FP parity and movement remain unverified."}),
    )
}
