//! First-ascent box contacts from explicit initialized world snapshots.
//! This command does not run the later world/actor line phase or advance action state.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets};
use ge_core::grenade_arc::prepare_grenade_arc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

const SCOPE: &str = "First grenade ascent only, with authored start/target and ordered world-manager state over initialized static maps. Original selection origin/apex radius, one type-three box, filter, portal flag and capacities are retained. No WorldLine, ActorLine, live-world clearance, action-state advance, body spawn or original-runtime/FP parity is established.";

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
    pub start: [f32; 3],
    pub target: [f32; 3],
    pub angle_index: u32,
    pub small_lift_raw: u8,
}
pub fn run(options: &Options) -> Result<Value> {
    let snapshot: Snapshot = crate::world_query::read_snapshot(&options.snapshot)?;
    query_loaded(&snapshot, |resource| {
        GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(resource)
    })
}

/// Ordinary analytic rejection does not address world state or load an image.
/// Explicitly empty world lists are different from unavailable world resources.
pub fn query_loaded(
    snapshot: &Snapshot,
    mut load: impl FnMut(&str) -> Result<Edb>,
) -> Result<Value> {
    let candidate = prepare_grenade_arc(
        snapshot.start,
        snapshot.target,
        snapshot.angle_index,
        snapshot.small_lift_raw,
    )?;
    let Some(arc) = candidate.arc.as_ref() else {
        return Ok(
            json!({"candidate":candidate,"volume":null,"typed_request":null,"world_query":null,"blocked":null,
            "reference_compared":false,"scope":SCOPE}),
        );
    };
    let volume = arc.clearance_volume()?;
    let typed_request = volume.world_box_request()?;
    let query = crate::mesh_query::Snapshot {
        world_origin: typed_request.world_origin,
        radius: typed_request.radius,
        world_from_query: typed_request.world_from_query,
        boxes: typed_request.boxes.to_vec(),
        filter_slot: typed_request.filter_slot,
        filter_flags: typed_request.filter_flags,
        record_capacity: typed_request.record_capacity,
        contact_capacity: typed_request.contact_capacity,
    };
    let world_query = crate::world_query::query_worlds(
        &snapshot.worlds,
        typed_request.request_flags_raw,
        &query,
        &mut load,
    )?;
    let blocked = if world_query["selected_world"].is_null() {
        ensure!(
            snapshot.worlds.is_empty() && world_query["query"].is_null(),
            "nonempty world request has no established selected-world query"
        );
        false
    } else {
        let contacts = world_query["query"]["contacts"]
            .as_array()
            .context("selected-world query has no established contact list")?;
        !contacts.is_empty()
    };
    Ok(
        json!({"candidate":candidate,"volume":volume,"typed_request":typed_request,"world_query":world_query,"blocked":blocked,
        "reference_compared":false,"scope":SCOPE}),
    )
}

#[cfg(test)]
#[path = "tests/grenade_ascent.rs"]
mod tests;
