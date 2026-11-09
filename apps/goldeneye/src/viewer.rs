//! Native asset inspection. These controls do not implement the original game's input rules.
use anyhow::{Context, Result, bail, ensure};
use ge_assets::{scene::Scene, tpl::RgbaImage};
use ge_render::{
    GpuScene, MaterialImage,
    camera::{Camera, OrbitCamera},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[derive(clap::Args, Default)]
pub struct Options {
    /// Exit after this many submitted frames; useful for explicit local graphics checks.
    #[arg(long, value_parser=clap::value_parser!(u32).range(1..=10000))]
    pub frames: Option<u32>,
    /// Create a hidden native window for a bounded graphics check.
    #[arg(long, requires = "frames")]
    pub hidden: bool,
    /// Request a real window resize after the first frame (bounded checks only).
    #[arg(long, requires = "frames")]
    pub smoke_resize: bool,
    /// Save the final bounded frame as a local PNG; existing files are preserved.
    #[arg(long, requires = "frames")]
    pub capture: Option<PathBuf>,
}
impl Options {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.smoke_resize || self.frames.is_some_and(|n| n >= 3),
            "resize smoke needs at least three frames"
        );
        if let Some(path) = &self.capture {
            ensure!(
                !path.exists(),
                "capture output already exists: {}",
                path.display()
            );
        }
        Ok(())
    }
}

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
    pub scene: ge_render::SceneStats,
    pub capture_rgba_sha256: Option<String>,
    pub capture_non_background_pixels: Option<usize>,
    pub scope: &'static str,
}
pub struct Viewed {
    pub evidence: Evidence,
    pub image: Option<RgbaImage>,
}

pub fn run(
    scene: &Scene,
    materials: &BTreeMap<usize, MaterialImage>,
    camera: Camera,
    size: [u32; 2],
    options: &Options,
) -> Result<Viewed> {
    options.validate()?;
    ensure!(
        size.iter().all(|&n| (1..=4096).contains(&n)),
        "window size must be 1..4096 per axis"
    );
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        scene,
        materials,
        orbit: OrbitCamera::new(camera)?,
        size,
        options,
        native: None,
        error: None,
        deadline: Instant::now() + Duration::from_secs(30),
        scene_uploads: 0,
        dragging: None,
        last_cursor: None,
        active: true,
        retry_at: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    let state = app.native.context("native window did not initialize")?;
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
            .filter(|p| *p != [9, 12, 15, 255])
            .count()
    });
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
            scene: state.gpu.stats.clone(),
            capture_rgba_sha256,
            capture_non_background_pixels,
            scope: "Native inspection window and presentation only; hidden smoke is not human visual/input validation. Original camera/visibility, lighting/materials, animation, gameplay and reference parity remain unverified.",
        },
        image: state.capture,
    })
}

struct Native {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::Texture,
    gpu: GpuScene,
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
    async fn new(
        window: Arc<Window>,
        scene: &Scene,
        materials: &BTreeMap<usize, MaterialImage>,
        capture: bool,
    ) -> Result<Self> {
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
                label: Some("GoldenEye native inspection"),
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
        let depth = ge_render::depth_texture(&device, config.width, config.height)?;
        let gpu = GpuScene::upload(&device, &queue, scene, materials, config.format)?;
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
            depth,
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
        let depth = ge_render::depth_texture(&self.device, size.width, size.height)?;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth = depth;
        self.resize_count += 1;
        Ok(())
    }
    fn draw(&mut self, camera: Camera, capture: bool) -> Result<bool> {
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
        self.gpu.update_camera(
            &self.queue,
            camera,
            self.config.width as f32 / self.config.height as f32,
        )?;
        let color = frame.texture.create_view(&Default::default());
        let depth = self.depth.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.gpu.encode(&mut encoder, &color, &depth);
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
    scene: &'a Scene,
    materials: &'a BTreeMap<usize, MaterialImage>,
    orbit: OrbitCamera,
    size: [u32; 2],
    options: &'a Options,
    native: Option<Native>,
    error: Option<anyhow::Error>,
    deadline: Instant,
    scene_uploads: u32,
    dragging: Option<MouseButton>,
    last_cursor: Option<PhysicalPosition<f64>>,
    active: bool,
    retry_at: Option<Instant>,
}
impl App<'_> {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: anyhow::Error) {
        self.error = Some(error);
        event_loop.exit();
    }
    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let Some(state) = self.native.as_mut() else {
            return Ok(());
        };
        if self.options.frames.is_some_and(|n| state.frames >= n) {
            event_loop.exit();
            return Ok(());
        }

        let capture =
            self.options.capture.is_some() && self.options.frames == Some(state.frames + 1);
        let presented = self.active && state.draw(self.orbit.camera(), capture)?;
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
        let Some(state) = self.native.as_mut() else {
            return Ok(());
        };
        if state.window.id() != id {
            return Ok(());
        }
        let mut redraw = true;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size)?,
            WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. } => {
                self.dragging = None;
                self.last_cursor = None;
                redraw = false;
            }
            WindowEvent::MouseInput {
                state: pressed,
                button,
                ..
            } => {
                self.dragging = if pressed == ElementState::Pressed {
                    Some(button)
                } else if self.dragging == Some(button) {
                    None
                } else {
                    self.dragging
                };
                redraw = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(previous) = self.last_cursor {
                    let delta = [
                        (position.x - previous.x).clamp(-4000., 4000.) as f32,
                        (position.y - previous.y).clamp(-4000., 4000.) as f32,
                    ];
                    match self.dragging {
                        Some(MouseButton::Left) => {
                            self.orbit.orbit([-delta[0] * 0.006, delta[1] * 0.006])?
                        }
                        Some(MouseButton::Right | MouseButton::Middle) => {
                            self.orbit.pan(delta, state.config.height as f32)?
                        }
                        _ => redraw = false,
                    }
                } else {
                    redraw = false;
                }
                self.last_cursor = Some(position);
            }
            WindowEvent::MouseWheel { delta, .. } => self.orbit.zoom(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => (p.y / 40.) as f32,
            })?,
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Named(NamedKey::Home) => self.orbit.reset(),
                    _ => redraw = false,
                }
            }
            WindowEvent::RedrawRequested => return self.redraw(event_loop),
            _ => redraw = false,
        }
        if redraw {
            state.window.request_redraw();
        }
        Ok(())
    }
}
impl ApplicationHandler for App<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.active = true;
        if self.native.is_none() {
            let result = (|| -> Result<Native> {
                let title = format!(
                    "GoldenEye Wii | ASSET INSPECTION | {} placements omitted | Drag: orbit / right: pan / wheel: zoom / Home: reset",
                    self.scene.placements_not_loaded
                );
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
                    self.scene,
                    self.materials,
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
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.active = false;
        self.dragging = None;
        self.last_cursor = None;
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Err(error) = self.event(event_loop, id, event) {
            self.fail(event_loop, error);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.options.frames.is_some() {
            if Instant::now() >= self.deadline {
                self.fail(
                    event_loop,
                    anyhow::anyhow!(
                        "bounded native viewer timed out: {:?}",
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
        } else if let Some(when) = self.retry_at {
            if Instant::now() >= when {
                self.retry_at = None;
                if let Some(state) = &self.native {
                    state.window.request_redraw();
                }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(when));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
