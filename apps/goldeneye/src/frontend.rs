//! First visible game checkpoint: a reconstructed main-menu layout using local assets.
use crate::menu::view::sprite;
use anyhow::{Context, Result, ensure};
use ge_assets::{frontend_font::FrontendFont, resource::GameAssets, tpl::RgbaImage};
use ge_core::frontend::State;
use ge_core::frontend_sequence::Stage;
use ge_render::frontend::{Layer, Quad};
use std::sync::Arc;
use std::{collections::BTreeMap, path::PathBuf};

pub const WHITE: [f32; 4] = [0.97, 0.91, 0.82, 1.];
pub const BLUE: [f32; 4] = [0.39, 0.65, 0.78, 1.];

#[derive(clap::Args, Default)]
pub struct PlayOptions {
    /// User-owned SJBE52 revision-zero image. Auto-detected beside the executable or in the current directory when omitted.
    pub disc: Option<PathBuf>,
    /// Developer shortcut: open the menu directly instead of playing startup screens.
    #[arg(long)]
    pub skip_startup: bool,
    /// Disable native audio output explicitly.
    #[arg(long)]
    pub mute: bool,
    /// Begin at a named stage for an explicit bounded media check.
    #[arg(long, value_enum, requires = "frames", conflicts_with = "skip_startup")]
    pub start_at: Option<StartupStage>,
    /// Bounded developer probe: confirm the title when its source intro finishes.
    #[arg(long, requires_all = ["frames", "start_at"])]
    pub smoke_confirm_title: bool,
    /// Bounded developer inputs through the normal menu handler, six presented frames apart.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        requires = "frames",
        conflicts_with = "smoke_confirm_title"
    )]
    pub menu_inputs: Vec<crate::menu::session::ProbeInput>,
    /// Windows preferences file; independent of Wii saves. Defaults under LOCALAPPDATA.
    #[arg(long)]
    pub settings_file: Option<PathBuf>,
    /// Original title projection setting; independent of the host capture canvas.
    #[arg(long, value_enum, default_value = "wide")]
    pub title_aspect: TitleAspect,
    #[command(flatten)]
    pub window: crate::viewer::Options,
}

#[derive(Clone, Copy, Default, clap::ValueEnum)]
pub enum TitleAspect {
    #[default]
    Wide,
    Standard,
}
impl From<TitleAspect> for ge_core::frontend_projection::Aspect {
    fn from(value: TitleAspect) -> Self {
        match value {
            TitleAspect::Wide => Self::Wide,
            TitleAspect::Standard => Self::Standard,
        }
    }
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub enum StartupStage {
    Strap,
    StrapFade,
    Zapper,
    ZapperFade,
    Activision,
    Eurocom,
    Mgm,
    Legal,
    Title,
    MainMenu,
}

pub struct Assets {
    pub images: BTreeMap<u32, RgbaImage>,
    pub font: FrontendFont,
    pub font_texture: u32,
    pub title: Arc<crate::frontend_title::Program>,
    pub credits: crate::menu::credits::Credits,
    pub credits_font: FrontendFont,
    pub credits_texture: u32,
}
impl Assets {
    pub fn load(assets: &mut GameAssets) -> Result<Self> {
        let menu = assets.load_edb("fe_main.edb")?;
        let fonts = assets.load_edb("o_bondfont.edb")?;
        let boot = assets.load_edb("fe_boot.edb")?;
        let title = crate::frontend_title::Program::load(&menu)?;
        let font = fonts
            .frontend_font(0x0700002f)?
            .context("main menu font missing")?;
        let font_texture = fonts
            .textures
            .get(font.body.texture_selector_raw as usize)
            .context("font texture selector outside directory")?
            .hash;
        let credits = crate::menu::credits::Credits::load(assets)?;
        let credits_font = fonts
            .frontend_font(0x0700002e)?
            .context("credits font missing")?;
        let credits_texture = fonts
            .textures
            .get(credits_font.body.texture_selector_raw as usize)
            .context("credits font texture selector outside directory")?
            .hash;
        let mut images = BTreeMap::new();
        for hash in [
            0x86000020, 0x86000025, 0x86000029, 0x8600002b, 0x8600002d, 0x8600002e,
        ] {
            images.insert(hash, menu.decode_texture(hash, 0, 0)?);
        }
        for hash in title.textures() {
            images.insert(hash, menu.decode_texture(hash, 0, 0)?);
        }
        for index in [7, 15, 16, 17, 18, 19, 20] {
            images.insert(
                0x1000 + index,
                boot.decode_texture(0x86000000 + index, 0, 0)?,
            );
        }
        images.insert(font_texture, fonts.decode_texture(font_texture, 0, 0)?);
        images.insert(
            credits_texture,
            fonts.decode_texture(credits_texture, 0, 0)?,
        );
        images.insert(
            0,
            RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![255; 4],
            },
        );
        // One persistent movie target; asynchronous decoding supplies its frames.
        images.insert(
            1,
            RgbaImage {
                width: 640,
                height: 448,
                pixels: vec![0; 640 * 448 * 4],
            },
        );
        Ok(Self {
            images,
            font,
            font_texture,
            title,
            credits,
            credits_font,
            credits_texture,
        })
    }
    pub(crate) fn text(
        &self,
        out: &mut Vec<Layer>,
        text: &str,
        mut x: f32,
        y: f32,
        height: f32,
        color: [f32; 4],
    ) {
        for code in text.encode_utf16() {
            if let Some(g) = self.font.body.glyph(code) {
                let scale = height / f32::from(g.height.max(1));
                let width = f32::from(g.width) * scale;
                out.push(
                    Quad {
                        texture: self.font_texture,
                        rect: [x, y, width, height],
                        uv: [g.uv_min[0], g.uv_min[1], g.uv_max[0], g.uv_max[1]],
                        color,
                    }
                    .into(),
                );
                x += width;
            }
        }
    }
    pub fn layers(
        &self,
        state: &State,
        stage: Stage,
        elapsed: f64,
        title: &mut crate::frontend_title::Scene,
        menu_elapsed: f64,
    ) -> Result<Vec<Layer>> {
        let mut q = Vec::new();
        match stage {
            Stage::Strap | Stage::Zapper => {
                let left = if stage == Stage::Zapper {
                    19
                } else if elapsed * 30. < 170. || (325. ..480.).contains(&(elapsed * 30.)) {
                    15
                } else {
                    17
                };
                // Two checked source tiles form the warning image; layout refinement
                // remains separate from the confirmed boot state/timing contract.
                sprite(&mut q, 0x1000 + left, [-83.5, -29.3, 403.4, 538.6], [1.; 4]);
                q.push(
                    Quad {
                        texture: 0x1000 + left + 1,
                        rect: [319.9, -29.3, 403.4, 538.6],
                        uv: [0., 0., 1., 1.],
                        color: [1.; 4],
                    }
                    .into(),
                );
                return Ok(q);
            }
            Stage::Activision | Stage::Eurocom | Stage::Mgm => {
                sprite(&mut q, 1, [0., 0., 640., 480.], [1.; 4]);
                return Ok(q);
            }
            Stage::Legal => {
                sprite(&mut q, 0x1007, [0., 0., 640., 480.], [1.; 4]);
                return Ok(q);
            }
            Stage::Title => {
                sprite(&mut q, 1, [0., 0., 640., 480.], [1.; 4]);
                q.extend(title.layers(elapsed)?);
                self.text(
                    &mut q,
                    "PRESS ANY BUTTON",
                    175.,
                    349.,
                    19.,
                    [WHITE[0], WHITE[1], WHITE[2], title.evidence.prompt_alpha],
                );
                return Ok(q);
            }
            Stage::MainMenu => {}
        }
        q.extend(crate::menu::view::layers(self, state, menu_elapsed));
        Ok(q)
    }
}
pub fn disc_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let mut roots = vec![std::env::current_dir()?];
    if let Some(parent) = std::env::current_exe()?.parent() {
        roots.push(parent.to_path_buf());
    }
    roots.sort();
    roots.dedup();
    let mut candidates = Vec::new();
    for root in roots {
        for entry in std::fs::read_dir(root)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("rvz") || e.eq_ignore_ascii_case("iso"))
            {
                candidates.push(path);
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    ensure!(
        candidates.len() == 1,
        "Place one user-owned SJBE52 revision-zero RVZ/ISO beside the executable, or launch: goldeneye.exe play <disc path>"
    );
    Ok(candidates.remove(0))
}
pub fn run(options: PlayOptions) -> Result<()> {
    options.window.validate()?;
    ensure!(
        !options.smoke_confirm_title || matches!(options.start_at, Some(StartupStage::Title)),
        "--smoke-confirm-title requires --start-at title"
    );
    ensure!(
        options.menu_inputs.is_empty()
            || options.skip_startup
            || matches!(options.start_at, Some(StartupStage::MainMenu)),
        "--menu-inputs requires --skip-startup or --start-at main-menu"
    );
    ensure!(
        options.menu_inputs.len() <= 256
            && (options.menu_inputs.is_empty()
                || options
                    .window
                    .frames
                    .is_some_and(|n| u64::from(n) > options.menu_inputs.len() as u64 * 6)),
        "--frames must exceed six times the menu input count (maximum 256 inputs)"
    );
    let session = crate::menu::session::Session::open(options.settings_file, options.menu_inputs)?;
    let path = disc_path(options.disc)?;
    let mut source = GameAssets::open(&path, "Filelist.bin")?;
    ensure!(
        source.disc.identity.game_id == "SJBE52" && source.disc.identity.revision == 0,
        "The main-menu checkpoint currently supports SJBE52 revision zero only"
    );
    let assets = Assets::load(&mut source)?;
    let catalog = super::frontend_media::Catalog::load(&mut source)?;
    drop(source);
    let mut playback = super::frontend_playback::Playback::new(
        path.clone(),
        catalog,
        options.skip_startup,
        options.mute,
        options.start_at,
    )?;
    playback.set_settings(*session.state.settings())?;
    let result = super::frontend_window::run(
        &assets,
        &options.window,
        playback,
        options.smoke_confirm_title,
        options.title_aspect.into(),
        session,
        crate::game::Controller::new(path),
    )?;
    if let (Some(path), Some(image)) = (&options.window.capture, &result.image) {
        super::write_new(path, &super::encode_png(image)?)?;
    }
    println!("{}", serde_json::to_string_pretty(&result.evidence)?);
    Ok(())
}
