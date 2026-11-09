//! Native menu window. Window lifecycle stays separate from frontend assets and pure state.
use crate::menu::{
    session::Session,
    view::{Hit, hit},
};
use crate::{frontend::Assets, frontend_playback::Playback};
use anyhow::{Context, Result, bail, ensure};
use ge_assets::tpl::RgbaImage;
use ge_core::frontend::{Command, Effect, Input};
use ge_core::frontend_sequence::Stage;
use ge_render::frontend::{Frontend, Layer, Quad};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

use crate::viewer::Options;

#[derive(Serialize)]
pub struct Evidence {
    pub adapter: String,
    pub backend: String,
    pub surface_format: String,
    pub hidden: bool,
    pub frames_presented: u32,
    pub resize_count: u32,
    pub scene_uploads: u32,
    pub acquisition_attempts: u32,
    pub acquisition_skips: BTreeMap<String, u32>,
    pub final_size: [u32; 2],
    pub menu: ge_core::frontend::State,
    pub menu_session: crate::menu::session::Evidence,
    pub game_session: crate::game::Evidence,
    pub game_session_shutdown_complete: bool,
    pub credits_lines: usize,
    pub menu_elapsed_seconds: f64,
    pub playback: super::frontend_playback::Evidence,
    pub title: super::frontend_title::Evidence,
    pub title_confirmation_accepted: bool,
    pub textures: usize,
    pub capture_rgba_sha256: Option<String>,
    pub capture_non_background_pixels: Option<usize>,
    pub scope: &'static str,
}
pub struct Viewed {
    pub evidence: Evidence,
    pub image: Option<RgbaImage>,
}

pub fn run(
    assets: &Assets,
    options: &Options,
    playback: Playback,
    smoke_confirm_title: bool,
    title_aspect: ge_core::frontend_projection::Aspect,
    menu: Session,
    game: crate::game::Controller,
) -> Result<Viewed> {
    options.validate()?;
    let size = [960, 720];
    ensure!(
        size.iter().all(|&n| (1..=4096).contains(&n)),
        "window size must be 1..4096 per axis"
    );
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        assets,
        menu,
        game,
        playback,
        title: super::frontend_title::Scene::with_aspect(assets.title.clone(), title_aspect)?,
        smoke_confirm_title,
        title_confirmation_accepted: false,
        last_tick: Instant::now(),
        size,
        options,
        native: None,
        error: None,
        deadline: Instant::now()
            + Duration::from_secs(30).max(Duration::from_millis(
                u64::from(options.frames.unwrap_or(0)) * 50,
            )),
        scene_uploads: 0,
        last_cursor: None,
        active: true,
        retry_at: None,
    };
    let window_result = event_loop.run_app(&mut app);
    let result = (|| -> Result<Viewed> {
        window_result?;
        app.menu.flush()?;
        if let Some(error) = app.error.take() {
            return Err(error);
        }
        let state = app
            .native
            .take()
            .context("native window did not initialize")?;
        if let Some(frames) = options.frames {
            ensure!(
                state.frames >= frames,
                "window closed before bounded frame check finished"
            );
        }
        state.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })?;
        let capture_rgba_sha256 = state
            .capture
            .as_ref()
            .map(|i| format!("{:x}", Sha256::digest(&i.pixels)));
        let capture_non_background_pixels = state.capture.as_ref().map(|i| {
            i.pixels
                .chunks_exact(4)
                .filter(|p| *p != [0, 0, 0, 255])
                .count()
        });
        let menu = app.menu.state.clone();
        let menu_elapsed_seconds = app.menu.elapsed();
        let menu_session = app.menu.evidence();
        let game_session = app.game.evidence();
        Ok(Viewed {
            evidence: Evidence {
                adapter: state.adapter,
                backend: state.backend,
                surface_format: format!("{:?}", state.config.format),
                hidden: options.hidden,
                frames_presented: state.frames,
                resize_count: state.resize_count,
                scene_uploads: app.scene_uploads,
                acquisition_attempts: state.acquisition_attempts,
                acquisition_skips: state.acquisition_skips,
                final_size: [state.config.width, state.config.height],
                menu,
                menu_session,
                game_session,
                game_session_shutdown_complete: true,
                credits_lines: assets.credits.lines().len(),
                menu_elapsed_seconds,
                playback: app.playback.evidence()?,
                title: app.title.evidence,
                title_confirmation_accepted: app.title_confirmation_accepted,
                textures: assets.images.len(),
                capture_rgba_sha256,
                capture_non_background_pixels,
                scope: "Native startup media and animated frontend with local textures/fonts and PCM output. Host keyboard/mouse/probe inputs share one command boundary. Source difficulty selection can admit a host-local one-user session and retain mission resources in its CPU worker, with world/player/camera readiness separate. Session music suspension is a host transition; accepted effect tails remain audible. Visible session evidence precedes unconditional post-window worker cleanup. Saves, controller configuration, cheat activation, gameplay and original reference synchronization remain incomplete.",
            },
            image: state.capture,
        })
    })();
    let shutdown = app.game.shutdown();
    match (result, shutdown) {
        (Ok(viewed), Ok(())) => Ok(viewed),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("session shutdown also failed: {cleanup:#}")))
        }
    }
}

struct Native {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    gpu: Frontend,
    adapter: String,
    backend: String,
    frames: u32,
    resize_count: u32,
    capture: Option<RgbaImage>,
    zero_size: bool,
    acquisition_attempts: u32,
    acquisition_skips: BTreeMap<String, u32>,
}
impl Native {
    async fn new(window: Arc<Window>, assets: &Assets, capture: bool) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .context("no surface-compatible native graphics adapter")?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("GoldenEye main menu"),
                ..Default::default()
            })
            .await?;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("native surface unsupported")?;
        let caps = surface.get_capabilities(&adapter);
        // Preserve the current inspection shader's linear UNORM output where supported.
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        if capture {
            ensure!(
                caps.usages.contains(wgpu::TextureUsages::COPY_SRC),
                "this surface cannot capture frames"
            );
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let gpu = Frontend::upload(&device, &queue, &assets.images, config.format)?;
        surface.configure(&device, &config);
        if let Some(error) = validation.pop().await {
            bail!("native initialization validation: {error}");
        }
        Ok(Self {
            instance,
            surface,
            window,
            device,
            queue,
            config,
            gpu,
            adapter: info.name,
            backend: format!("{:?}", info.backend),
            frames: 0,
            resize_count: 0,
            capture: None,
            zero_size: false,
            acquisition_attempts: 0,
            acquisition_skips: BTreeMap::new(),
        })
    }
    fn resize(&mut self, size: PhysicalSize<u32>) -> Result<()> {
        self.zero_size = size.width == 0 || size.height == 0;
        if self.zero_size {
            return Ok(());
        }
        if self.config.width == size.width && self.config.height == size.height {
            return Ok(());
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.resize_count += 1;
        Ok(())
    }
    fn draw(
        &mut self,
        assets: &Assets,
        menu: &Session,
        playback: &mut Playback,
        capture: bool,
        title: &mut super::frontend_title::Scene,
        game: Option<&crate::game::Snapshot>,
    ) -> Result<bool> {
        if self.zero_size {
            return Ok(false);
        }
        self.acquisition_attempts += 1;
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                drop(frame);
                self.surface.configure(&self.device, &self.config);
                return self.skip("suboptimal");
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.window.inner_size())?;
                self.surface.configure(&self.device, &self.config);
                return self.skip("outdated");
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.surface.configure(&self.device, &self.config);
                return self.skip("lost");
            }
            wgpu::CurrentSurfaceTexture::Timeout => return self.skip("timeout"),
            wgpu::CurrentSurfaceTexture::Occluded => return self.skip("occluded"),
            wgpu::CurrentSurfaceTexture::Validation => {
                bail!("native surface acquisition validation failed")
            }
        };
        let validation = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let color = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut layers = if let Some(snapshot) = game {
            crate::game::view::layers(assets, snapshot)
        } else {
            assets.layers(
                &menu.state,
                playback.stage(),
                playback.elapsed(),
                title,
                menu.elapsed(),
            )?
        };
        for cue in title.take_cues() {
            playback.play_intro_cue(&cue)?;
        }
        crate::menu::view::brightness(&mut layers, menu.state.settings().brightness());
        let rgba = playback.sequence.overlay_rgba();
        if rgba[3] != 0 {
            layers.push(Layer::Quad(Quad {
                texture: 0,
                rect: [0., 0., 640., 480.],
                uv: [0., 0., 1., 1.],
                color: rgba.map(|byte| f32::from(byte) / 255.),
            }));
        }
        self.gpu.encode_layers(
            &self.device,
            &mut encoder,
            &color,
            [self.config.width, self.config.height],
            &layers,
        )?;
        self.queue.submit(Some(encoder.finish()));
        if capture {
            self.capture = Some(ge_render::readback(
                &self.device,
                &self.queue,
                &frame.texture,
            )?);
        }
        self.window.pre_present_notify();
        self.queue.present(frame);
        if let Some(error) = pollster::block_on(validation.pop()) {
            bail!("native frame validation: {error}");
        }
        self.frames += 1;
        Ok(true)
    }
    fn skip(&mut self, status: &str) -> Result<bool> {
        *self.acquisition_skips.entry(status.into()).or_default() += 1;
        Ok(false)
    }
}

struct App<'a> {
    assets: &'a Assets,
    menu: Session,
    game: crate::game::Controller,
    playback: Playback,
    title: super::frontend_title::Scene,
    smoke_confirm_title: bool,
    title_confirmation_accepted: bool,
    last_tick: Instant,
    size: [u32; 2],
    options: &'a Options,
    native: Option<Native>,
    error: Option<anyhow::Error>,
    deadline: Instant,
    scene_uploads: u32,
    last_cursor: Option<PhysicalPosition<f64>>,
    active: bool,
    retry_at: Option<Instant>,
}
impl App<'_> {
    /// All keyboard, pointer and bounded-probe commands cross this boundary.
    fn input(&mut self, input: Input) -> Result<Effect> {
        if self.game.is_active() {
            return if input == Input::Back {
                self.game.cancel();
                self.playback.set_session_suspended(false)?;
                self.menu.input(input, &mut self.playback)
            } else {
                Ok(Effect::None)
            };
        }
        let effect = self.menu.input(input, &mut self.playback)?;
        if let Some(Command::NewGame(request)) = self.menu.state.take_command() {
            self.game.start(request)?;
            self.playback.set_session_suspended(true)?;
        }
        Ok(effect)
    }
    fn confirm_title(&mut self) -> Result<()> {
        self.title_confirmation_accepted |= self.title.request_outro(self.playback.elapsed())?;
        Ok(())
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        self.error = Some(error);
        event_loop.exit();
    }
    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        // A queued OS paint/about-to-wait callback can arrive after the final
        // presentation. Do not advance diagnostics beyond the captured frame.
        if self.native.as_ref().is_some_and(|state| {
            self.options
                .frames
                .is_some_and(|limit| state.frames >= limit)
        }) {
            event_loop.exit();
            return Ok(());
        }
        let now = Instant::now();
        self.game.poll()?;
        let elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;
        self.game.advance_frame_clock(elapsed)?;
        let image = self.playback.tick(elapsed)?;
        if self.smoke_confirm_title
            && !self.title_confirmation_accepted
            && self.playback.stage() == Stage::Title
        {
            self.confirm_title()?;
        }
        let frames = self.native.as_ref().map_or(0, |s| s.frames);
        if let Some(input) = self.menu.tick(frames)? {
            let effect = self.input(input.into())?;
            self.menu.record_probe(frames, input, effect);
        }
        let Some(state) = self.native.as_mut() else {
            return Ok(());
        };
        if let Some(image) = image {
            state.gpu.update_image(&state.queue, 1, &image)?;
        }
        if !self.playback.ready() {
            return Ok(());
        }
        let capture =
            self.options.capture.is_some() && self.options.frames == Some(state.frames + 1);
        // A bounded capture waits for the actual load result at its last frame;
        // disc I/O never runs on this callback and the existing deadline bounds it.
        if self.options.frames == Some(state.frames + 1)
            && self.game.snapshot().is_some_and(|snapshot| {
                matches!(
                    snapshot.stage,
                    crate::game::SessionStage::Admitting | crate::game::SessionStage::Loading
                )
            })
        {
            return Ok(());
        }
        let presented = self.active
            && state.draw(
                self.assets,
                &self.menu,
                &mut self.playback,
                capture,
                &mut self.title,
                self.game.snapshot(),
            )?;
        if self.playback.stage() == Stage::Title && self.title.complete() {
            self.playback.confirm()?;
        }
        self.retry_at = None;
        if presented {
            if state.frames == 1 && self.options.smoke_resize {
                let size =
                    PhysicalSize::new((self.size[0] * 3 / 4).max(1), (self.size[1] * 3 / 4).max(1));
                if let Some(actual) = state.window.request_inner_size(size) {
                    state.resize(actual)?;
                }
            }
            if self.options.frames.is_some_and(|n| state.frames >= n) {
                event_loop.exit();
            }
        } else if self.active && !state.zero_size {
            // Occluded/outdated surfaces need a later retry even in draw-on-demand mode.
            self.retry_at = Some(Instant::now() + Duration::from_millis(100));
        }
        Ok(())
    }
    fn event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) -> Result<()> {
        let Some(state) = self.native.as_ref() else {
            return Ok(());
        };
        if state.window.id() != id {
            return Ok(());
        }
        let size = [state.config.width, state.config.height];
        let mut redraw = true;
        let mut title_confirmation = false;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self
                .native
                .as_mut()
                .expect("checked native window")
                .resize(size)?,
            WindowEvent::Focused(focused) => {
                if !self.options.hidden {
                    self.active = focused;
                    self.playback.set_paused(!focused)?;
                    self.last_tick = Instant::now();
                }
                self.last_cursor = None;
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_cursor = None;
                redraw = false;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.playback.stage() == Stage::Title {
                    title_confirmation = true;
                } else if self.playback.stage() != Stage::MainMenu {
                    self.playback.confirm()?;
                } else if self.game.is_active() {
                    if self.last_cursor.is_some_and(|position| {
                        crate::game::view::hit_back([position.x, position.y], size)
                    }) {
                        self.input(Input::Back)?;
                    }
                } else if let Some(position) = self.last_cursor {
                    let target = hit([position.x, position.y], size, &self.menu.state);
                    match target {
                        Some(Hit::Row(index) | Hit::Decrease(index) | Hit::Increase(index)) => {
                            if self.menu.select(index, &mut self.playback)? {
                                let input = match target {
                                    Some(Hit::Decrease(_)) => Input::Left,
                                    Some(Hit::Increase(_)) => Input::Right,
                                    _ => Input::Confirm,
                                };
                                self.input(input)?;
                            }
                        }
                        Some(Hit::Back) => {
                            self.input(Input::Back)?;
                        }
                        Some(Hit::Reset) => {
                            self.input(Input::Reset)?;
                        }
                        Some(Hit::Confirm) => {
                            self.input(Input::Confirm)?;
                        }
                        None => {}
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_cursor = Some(position);
                if self.playback.stage() == Stage::MainMenu
                    && !self.game.is_active()
                    && let Some(Hit::Row(index) | Hit::Decrease(index) | Hit::Increase(index)) =
                        hit([position.x, position.y], size, &self.menu.state)
                {
                    self.menu.select(index, &mut self.playback)?;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.playback.stage() == Stage::Title {
                    title_confirmation = true;
                } else if self.playback.stage() != Stage::MainMenu {
                    self.playback.confirm()?;
                } else if let Some(input) = key_input(&event.logical_key) {
                    if !event.repeat
                        || matches!(input, Input::Up | Input::Down | Input::Left | Input::Right)
                    {
                        self.input(input)?;
                    }
                } else {
                    redraw = false;
                }
            }
            WindowEvent::RedrawRequested => return self.redraw(event_loop),
            _ => redraw = false,
        }
        if redraw {
            self.native
                .as_ref()
                .expect("checked native window")
                .window
                .request_redraw();
        }
        if title_confirmation {
            self.confirm_title()?;
        }
        Ok(())
    }
}
impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.active = true;
        self.last_tick = Instant::now();
        if let Err(error) = self.playback.set_paused(false) {
            self.fail(event_loop, error);
            return;
        }
        if self.native.is_none() {
            let result = (|| -> Result<Native> {
                let title = "GoldenEye 007";
                let window = Arc::new(
                    event_loop.create_window(
                        Window::default_attributes()
                            .with_title(title)
                            .with_inner_size(PhysicalSize::new(self.size[0], self.size[1]))
                            .with_visible(false),
                    )?,
                );
                let state = pollster::block_on(Native::new(
                    window,
                    self.assets,
                    self.options.capture.is_some(),
                ))?;
                if !self.options.hidden {
                    state.window.set_visible(true);
                }
                state.window.request_redraw();
                Ok(state)
            })();
            match result {
                Ok(state) => {
                    self.native = Some(state);
                    self.scene_uploads += 1;
                }
                Err(error) => self.fail(event_loop, error),
            }
        } else if let Some(state) = &self.native {
            state.window.request_redraw();
        }
    }
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.playback.set_paused(true) {
            self.fail(event_loop, error);
        }
        self.active = false;
        self.last_cursor = None;
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Err(error) = self.event(event_loop, id, event) {
            self.fail(event_loop, error);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.active && self.options.frames.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        if self.options.frames.is_some() {
            if Instant::now() >= self.deadline {
                self.fail(
                    event_loop,
                    anyhow::anyhow!(
                        "bounded native menu timed out: {:?}",
                        self.native.as_ref().map(|n| (
                            n.frames,
                            n.acquisition_attempts,
                            &n.acquisition_skips
                        ))
                    ),
                );
                return;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
            if self.options.hidden {
                // Hidden windows may receive no OS paint events. Drive only this app's
                // bounded frame operation; do not synthesize desktop input or OS events.
                if let Err(error) = self.redraw(event_loop) {
                    self.fail(event_loop, error);
                }
            } else if let Some(state) = &self.native {
                state.window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(8),
            ));
            if let Some(state) = &self.native {
                state.window.request_redraw();
            }
        }
    }
}

fn key_input(key: &Key) -> Option<Input> {
    match key {
        Key::Named(NamedKey::ArrowUp) => Some(Input::Up),
        Key::Named(NamedKey::ArrowDown) => Some(Input::Down),
        Key::Named(NamedKey::ArrowLeft) => Some(Input::Left),
        Key::Named(NamedKey::ArrowRight) => Some(Input::Right),
        Key::Named(NamedKey::Enter | NamedKey::Space) => Some(Input::Confirm),
        Key::Named(NamedKey::Escape | NamedKey::Backspace) => Some(Input::Back),
        Key::Character(c) if c.eq_ignore_ascii_case("w") => Some(Input::Up),
        Key::Character(c) if c.eq_ignore_ascii_case("s") => Some(Input::Down),
        Key::Character(c) if c.eq_ignore_ascii_case("a") => Some(Input::Left),
        Key::Character(c) if c.eq_ignore_ascii_case("d") => Some(Input::Right),
        Key::Character(c) if c.eq_ignore_ascii_case("r") => Some(Input::Reset),
        _ => None,
    }
}
#[cfg(test)]
#[path = "tests/frontend_window.rs"]
mod tests;
