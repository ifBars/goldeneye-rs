//! Explicit local mission-context inspection through the runtime's retained loader.
use super::MissionContext;
use anyhow::Result;
use ge_assets::resource::GameAssets;
use ge_core::mission::Difficulty;
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(clap::Args)]
pub struct Options {
    pub disc: PathBuf,
    /// Source level resource ID; New Game selects the tutorial.
    #[arg(long, default_value = "0181016b", value_parser = crate::parse_hash)]
    pub level: u32,
    /// Source difficulty: 0 Operative, 1 Agent, 2 007, 3 007 Classic.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    pub difficulty: u8,
    /// Requested StartPoint ID; 1 matches Tutorial New Game. Other requests are explicit inspection inputs.
    #[arg(long, default_value_t = 1)]
    pub start_point_id: u32,
    /// Explicit FP mode request; does not infer initial weapon/action state.
    #[arg(long, value_parser = crate::parse_hash, requires_all = ["animation_previous", "animation_fallback"])]
    pub animation_request: Option<u32>,
    /// Explicit previous mode reference; zero is a coded directory index.
    #[arg(long, value_parser = crate::parse_hash, requires_all = ["animation_request", "animation_fallback"])]
    pub animation_previous: Option<u32>,
    /// Explicit fallback mode reference; no default animation history is invented.
    #[arg(long, value_parser = crate::parse_hash, requires_all = ["animation_request", "animation_previous"])]
    pub animation_fallback: Option<u32>,
    /// Construct owned fresh FP animation layers from supplied weapon data. Does not tick or sample a pose.
    #[arg(long, conflicts_with_all=["animation_request","animation_previous","animation_fallback"])]
    pub initialize_animation_layers: bool,
    /// Decode the selected constructor clip's raw frame. Does not commit a player pose or camera.
    #[arg(long, requires = "initialize_animation_layers")]
    pub decode_initial_frame: bool,
    /// Execute the source reset's fixed 0.2-second FP prewarm and commit local bones.
    #[arg(
        long,
        requires = "initialize_animation_layers",
        conflicts_with = "decode_initial_frame"
    )]
    pub evaluate_prewarm_pose: bool,
    /// Evaluate tagged-bone FK with an explicitly disabled cache inspection context.
    #[arg(
        long,
        requires = "evaluate_prewarm_pose",
        conflicts_with = "evaluate_cached_camera_bone"
    )]
    pub evaluate_uncached_camera_bone: bool,
    /// Construct the FP actor-bone cache and evaluate the tagged camera bone.
    #[arg(
        long,
        requires = "evaluate_prewarm_pose",
        conflicts_with = "evaluate_uncached_camera_bone"
    )]
    pub evaluate_cached_camera_bone: bool,
    /// Create a metadata-only receipt; existing files are never overwritten.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn run(options: Options) -> Result<()> {
    let difficulty = Difficulty::try_from(options.difficulty)?;
    let mut assets = GameAssets::open(&options.disc, "Filelist.bin")?;
    let mut context = if options.start_point_id == 1 {
        MissionContext::load(&mut assets, options.level, difficulty)?
    } else {
        MissionContext::load_with_start_point(
            &mut assets,
            options.level,
            difficulty,
            options.start_point_id,
        )?
    };
    if let (Some(requested), Some(previous), Some(fallback)) = (
        options.animation_request,
        options.animation_previous,
        options.animation_fallback,
    ) {
        context.prepare_animation(requested, previous, fallback)?;
    }
    if options.initialize_animation_layers {
        context.initialize_animation_layers(&mut assets)?;
    }
    if options.decode_initial_frame {
        context.decode_initial_frame()?;
    }
    if options.evaluate_prewarm_pose {
        context.evaluate_prewarm_pose()?;
    }
    if options.evaluate_uncached_camera_bone {
        context.evaluate_uncached_camera_bone()?;
    }
    if options.evaluate_cached_camera_bone {
        context.evaluate_cached_camera_bone()?;
    }
    let json = serde_json::to_vec_pretty(&context.evidence())?;
    if let Some(path) = options.output {
        crate::write_new(&path, &json)?;
    }
    io::stdout().lock().write_all(&json)?;
    Ok(())
}
