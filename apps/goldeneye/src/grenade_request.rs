//! Asset-backed grenade launch preparation; no runtime instance or physics body is spawned.
use anyhow::{Context, Result, ensure};
use ge_assets::resource::GameAssets;
use ge_core::{
    grenade::{self, GrenadeSource, GrenadeSpin, GrenadeTime},
    navigation::ActorTargetRecord,
};
use serde::Deserialize;
use serde_json::json;
use std::{fs::File, io::Read, path::PathBuf};

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    /// Authored source, world records, RNG word, body flags and optional angular-only steps.
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    source: GrenadeSource,
    records: Vec<ActorTargetRecord>,
    rng_word: u32,
    body_flags_raw: u32,
    #[serde(default)]
    steps: Vec<GrenadeTime>,
}
pub fn run(options: &Options) -> Result<serde_json::Value> {
    let mut bytes = Vec::new();
    File::open(&options.snapshot)
        .context("opening grenade snapshot")?
        .take(1_048_577)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1_048_576, "grenade snapshot exceeds 1 MiB");
    let snapshot: Snapshot = serde_json::from_slice(&bytes).context("parsing grenade snapshot")?;
    ensure!(
        snapshot.steps.len() <= 256,
        "grenade snapshot exceeds 256 spin steps"
    );
    grenade::grenade_record(snapshot.source)?;
    let mut assets = GameAssets::open(&options.disc, "Filelist.bin")?;
    let edb = assets.load_edb(&format!("0x{:08x}", grenade::GRENADE_RESOURCE))?;
    let prototype = edb
        .animation_script_reference(grenade::GRENADE_SCRIPT_REFERENCE)?
        .context("missing grenade script prototype reference")?;
    let mut rng = snapshot.rng_word;
    let launch = grenade::prepare_grenade_launch(snapshot.source, &snapshot.records, &mut rng)?;
    let mut spin = GrenadeSpin {
        flags_raw: snapshot.body_flags_raw,
        angular_velocity: launch.angular_velocity,
    };
    let mut steps = Vec::new();
    for time in snapshot.steps {
        let before = spin.clone();
        let step = spin.damp(launch.angular_damping, time)?;
        steps.push(json!({"time":time,"before":before,"after":spin,"step":step}));
    }
    Ok(
        json!({"resource_hash":edb.header.hash,"script_reference":grenade::GRENADE_SCRIPT_REFERENCE,"prototype":prototype,
        "source":snapshot.source,"rng_before":snapshot.rng_word,"rng_after":rng,"launch":launch,"spin_steps":steps,"spin_after":spin,
        "reference_compared":false,"spawned":false,
        "scope":"Resource/entity metadata resolution and grenade launch packet preparation over authored world records. Optional steps apply angular damping only. A descriptor does not establish runtime XGrenade/component allocation. No body activation/registration, translation integration, collision, lifetime or live record setup; engine trigonometry and Wii FP precision remain unverified."}),
    )
}
