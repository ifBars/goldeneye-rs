//! Authored snapshots for the observed caller's distinct owner/instance coordinates.
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::navigation::{ActorTargetRequest, NavigationFrame};
use std::path::PathBuf;
#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    #[arg(value_parser=crate::parse_hash,required_unless_present="index",conflicts_with="index")]
    hash: Option<u32>,
    #[arg(long)]
    index: Option<usize>,
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    owner_translation: Vec<f32>,
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    instance_translation: Vec<f32>,
    /// World target from the caller's +0xa0 record; not snapped to a surface.
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    target: Vec<f32>,
    /// World position from the caller's selected active record.
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    active_position: Vec<f32>,
    #[arg(long, default_value_t = 100_000)]
    max_face_tests: usize,
    #[arg(long, default_value_t = 100_000)]
    max_expansions: usize,
    #[arg(long, default_value_t = 100_000)]
    max_goal_steps: usize,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
fn position(values: &[f32]) -> Result<[f32; 3]> {
    let point: [f32; 3] = values.try_into().context("expected three coordinates")?;
    ensure!(
        point.into_iter().all(f32::is_finite),
        "actor target coordinates must be finite"
    );
    Ok(point)
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.max_expansions > 0 && options.max_goal_steps > 0 && options.max_face_tests > 0,
        "actor target budgets must be positive"
    );
    let frame = NavigationFrame {
        translation: position(&options.instance_translation)?,
    };
    let request = ActorTargetRequest {
        owner_frame: NavigationFrame {
            translation: position(&options.owner_translation)?,
        },
        target: position(&options.target)?,
        active_position: position(&options.active_position)?,
        max_face_tests: options.max_face_tests,
    };
    let edb = GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(&options.resource)?;
    let graph = if let Some(index) = options.index {
        edb.entity_graph_at(index)?
    } else {
        edb.entity_graph(options.hash.context("entity hash required")?)?
    };
    let result = crate::path::PathData::new(&graph)?.query_actor_target(
        request,
        || Ok(Some(frame)),
        options.max_expansions,
        options.max_goal_steps,
    )?;
    Ok(
        serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"request":request,"instance_frame":frame,"max_expansions":options.max_expansions,"max_goal_steps":options.max_goal_steps,"result":result,"reference_compared":false,
        "scope":"Caller binding after its external action gate: owner-local storage-first 0<t<1.25 ray; original target height gate; later binding-frame lookup; instance-local raised active segment with depth five and manager flags; unraised explicit actor goal/edge/heuristic inputs. Snapshots/frame availability are authored. Initial mesh is retained across frame resolution. External action gate, live actor storage, later mutation, scheduling, steering, numeric parity and gameplay remain unverified."}),
    )
}
