//! One explicit target-action snapshot; game data loads lazily only after its angular gate.
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::navigation::{
    ActorTargetActionState, ActorTargetOwner, NavigationFrame, run_actor_target_action,
};
use serde::Deserialize;
use std::{fs::File, io::Read, path::PathBuf};
#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    #[arg(value_parser=crate::parse_hash,required_unless_present="index",conflicts_with="index")]
    hash: Option<u32>,
    #[arg(long)]
    index: Option<usize>,
    /// Authored JSON snapshot with owner/instance translations, yaw and caller records.
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long, default_value_t = 100_000)]
    max_face_tests: usize,
    #[arg(long, default_value_t = 100_000)]
    max_expansions: usize,
    #[arg(long, default_value_t = 100_000)]
    max_goal_steps: usize,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    owner_translation: [f32; 3],
    owner_yaw: f32,
    instance_translation: [f32; 3],
    state: ActorTargetActionState,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    ensure!(
        options.max_face_tests > 0 && options.max_expansions > 0 && options.max_goal_steps > 0,
        "actor action budgets must be positive"
    );
    let mut bytes = Vec::new();
    File::open(&options.snapshot)
        .context("opening actor action snapshot")?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1_048_576,
        "actor action snapshot exceeds 1 MiB"
    );
    let snapshot: Snapshot =
        serde_json::from_slice(&bytes).context("parsing actor action snapshot")?;
    let before = snapshot.state.clone();
    let mut after = snapshot.state;
    let owner = ActorTargetOwner {
        frame: NavigationFrame {
            translation: snapshot.owner_translation,
        },
        yaw: snapshot.owner_yaw,
    };
    let frame = NavigationFrame {
        translation: snapshot.instance_translation,
    };
    let mut query = None;
    let action = run_actor_target_action(&mut after, owner, options.max_face_tests, |request| {
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
        let selected = result.selection()?;
        query = Some(
            serde_json::json!({"resource_hash":edb.header.hash,"entity_hash":graph.entity.hash,"result":result}),
        );
        Ok(selected)
    })?;
    Ok(
        serde_json::json!({"owner":owner,"instance_frame":frame,"state_before":before,"state_after":after,"action":action,"query":query,"reference_compared":false,
        "scope":"Observed target-action angular gate, distinct-frame surface binding, actor policy search and selected-record motion update over an authored snapshot. Gate/miss returns 1 and preserves state; selected goal returns 2, sets raw state 2 and updates only the active motion. Scratch center is added directly to the captured instance frame. Host square root/reciprocal/libm are provisional; zero/overflow motion reports an error. Live record population, action scheduling, subsequent integration/collision, original FP behavior and gameplay parity remain unverified."}),
    )
}
