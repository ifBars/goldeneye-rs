//! Grenade caller binding for the initialized world portion of the line phase.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets};
use ge_core::grenade_arc::GrenadeWorldLineRequest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

const SCOPE: &str = "World portion of the grenade line phase only, using explicit cached apex/target and ordered world-manager snapshots over initialized static maps. Constructor displacement/bounds precede the caller's Y lift. A world miss still requires the separate actor pass. No full clearance, action-state advance, readiness, body spawn, live-world state or original-runtime/FP parity is established.";

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub worlds: Vec<crate::world_query::World>,
    pub cached_apex: [f32; 3],
    pub target: [f32; 3],
    pub small_lift_raw: u8,
}

pub fn run(options: &Options) -> Result<Value> {
    let snapshot: Snapshot = crate::world_query::read_snapshot(&options.snapshot)?;
    query_loaded(&snapshot, |resource| {
        GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(resource)
    })
}

fn world_blocked(world_count: usize, report: &Value) -> Result<bool> {
    let selected = report
        .get("selected_world")
        .context("world-line report has no selected-world field")?;
    let query = report
        .get("query")
        .context("world-line report has no query field")?;
    if selected.is_null() {
        ensure!(
            world_count == 0 && query.is_null(),
            "nonempty world-line request has no established selected-world query"
        );
        return Ok(false);
    }
    let index = selected
        .as_u64()
        .context("world-line report has an invalid selected-world index")?;
    ensure!(
        index < world_count as u64,
        "world-line selection outside supplied worlds"
    );
    let hit = query
        .get("line")
        .and_then(|line| line.get("hit"))
        .context("selected-world line query has no established hit payload")?;
    ensure!(
        hit.is_null() || hit.is_object(),
        "selected-world line query has an invalid hit payload"
    );
    Ok(!hit.is_null())
}

/// A world miss is only this phase's result. The actor pass remains required;
/// this adapter never supplies a fabricated actor miss or advances action state.
pub fn query_loaded(snapshot: &Snapshot, load: impl FnMut(&str) -> Result<Edb>) -> Result<Value> {
    let typed_request = GrenadeWorldLineRequest::from_target(
        snapshot.cached_apex,
        snapshot.target,
        snapshot.small_lift_raw,
    )?;
    let line = crate::world_line::LineSnapshot {
        origin: typed_request.origin,
        delta: typed_request.delta,
        constructor_delta: Some(typed_request.constructor_delta),
        maximum_parameter: typed_request.maximum_parameter,
        filter_slot: typed_request.filter_slot,
        filter_flags: typed_request.filter_flags,
        required_any: typed_request.required_any,
        excluded_any: typed_request.excluded_any,
        stop_first_raw: typed_request.stop_first_raw,
    };
    let world_query = crate::world_line::query_worlds(&snapshot.worlds, &line, load)?;
    let blocked = world_blocked(snapshot.worlds.len(), &world_query)?;
    Ok(json!({
        "typed_request":typed_request,"world_query":world_query,"blocked":blocked,
        "reference_compared":false,"scope":SCOPE
    }))
}

#[cfg(test)]
#[path = "tests/grenade_line.rs"]
mod tests;
