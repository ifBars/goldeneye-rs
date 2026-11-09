//! Native GPU map inspection. This shader is an unlit preview, not GX/TEV parity.
pub mod camera;
pub mod frontend;
mod gpu;
use anyhow::{Context, Result, ensure};
use camera::Camera;
use ge_assets::{scene::Scene, tpl::RgbaImage};
pub use gpu::{GpuScene, SceneStats, depth_texture};
use serde::Serialize;
use std::{collections::BTreeMap, sync::mpsc, time::Duration};
use wgpu::util::DeviceExt;
const BACKGROUND: [u8; 4] = [9, 12, 15, 255];

pub struct MaterialImage {
    pub image: RgbaImage,
    pub wrap_s: u32,
    pub wrap_t: u32,
}

#[derive(Debug, Serialize)]
pub struct RenderEvidence {
    pub adapter: String,
    pub backend: String,
    pub device_type: String,
    pub width: u32,
    pub height: u32,
    pub vertices: usize,
    pub triangles: usize,
    pub draws: usize,
    pub textures: usize,
    pub model_groups: usize,
    pub unique_triangles: usize,
    pub non_background_pixels: usize,
    pub scope: &'static str,
}
pub struct Rendered {
    pub image: RgbaImage,
    pub evidence: RenderEvidence,
}

pub async fn render(
    scene: &Scene,
    materials: &BTreeMap<usize, MaterialImage>,
    camera: Camera,
    width: u32,
    height: u32,
) -> Result<Rendered> {
    validate(scene, materials, width, height)?;
    camera.matrix(width as f32 / height as f32)?;
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .context("no native graphics adapter")?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("GoldenEye static map preview"),
            ..Default::default()
        })
        .await?;
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let gpu = GpuScene::upload(&device, &queue, scene, materials, format)?;
    gpu.update_camera(&queue, camera, width as f32 / height as f32)?;
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Preview color"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let depth = depth_texture(&device, width, height)?;
    let color_view = target.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    gpu.encode(&mut encoder, &color_view, &depth_view);
    queue.submit(Some(encoder.finish()));
    let image = readback(&device, &queue, &target)?;
    if let Some(error) = validation.pop().await {
        anyhow::bail!("GPU validation: {error}");
    }
    let evidence = RenderEvidence {
        adapter: info.name,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        width,
        height,
        vertices: scene.geometry.vertices.len(),
        triangles: scene.triangle_count(),
        draws: scene.geometry.batches.len(),
        textures: materials.len(),
        model_groups: gpu.stats.model_groups,
        unique_triangles: scene.geometry.indices.len() / 3,
        non_background_pixels: image
            .pixels
            .chunks_exact(4)
            .filter(|p| *p != BACKGROUND)
            .count(),
        scope: "Map inspection with shared geometry and model transforms; unlit texture*vertex-color preview with depth. Placement selection/rotation parity, unsupported entities, actors, sky, lighting/TEV, material-state parity and original-reference parity remain unverified.",
    };
    Ok(Rendered { image, evidence })
}

/// Copy a submitted RGBA/BGRA render target into an owned RGBA image.
/// The caller keeps the target alive (and unpresented, for surfaces) until this returns.
pub fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
) -> Result<RgbaImage> {
    let width = target.width();
    let height = target.height();
    let bgra = match target.format() {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        _ => anyhow::bail!("unsupported readback target format"),
    };
    ensure!(
        target.usage().contains(wgpu::TextureUsages::COPY_SRC),
        "readback target lacks COPY_SRC"
    );
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let stride = (width * 4).div_ceil(256) * 256;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Map readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    let submission = queue.submit(Some(encoder.finish()));
    let (sender, receiver) = mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(30)),
    })?;
    receiver.recv_timeout(Duration::from_secs(5))??;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    {
        let data = staging.slice(..).get_mapped_range()?;
        for row in data.chunks_exact(stride as usize) {
            pixels.extend_from_slice(&row[..width as usize * 4]);
        }
    }
    staging.unmap();

    if bgra {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    Ok(RgbaImage {
        width,
        height,
        pixels,
    })
}

fn wrap(value: u32) -> Result<wgpu::AddressMode> {
    Ok(match value {
        0 => wgpu::AddressMode::ClampToEdge,
        1 => wgpu::AddressMode::Repeat,
        2 => wgpu::AddressMode::MirrorRepeat,
        _ => anyhow::bail!("unsupported texture wrap mode {value}"),
    })
}

fn validate(
    scene: &Scene,
    materials: &BTreeMap<usize, MaterialImage>,
    width: u32,
    height: u32,
) -> Result<()> {
    ensure!(
        width > 0 && height > 0 && width <= 4096 && height <= 4096,
        "render size must be 1..4096 per axis"
    );
    ensure!(
        !scene.geometry.vertices.is_empty() && !scene.geometry.indices.is_empty(),
        "empty render geometry"
    );
    ensure!(
        scene
            .geometry
            .indices
            .iter()
            .all(|&i| (i as usize) < scene.geometry.vertices.len()),
        "render index out of bounds"
    );
    ensure!(
        scene.geometry.vertices.iter().all(|v| v
            .position
            .iter()
            .chain(&v.uv)
            .all(|f| f.is_finite())),
        "non-finite render vertex"
    );
    for batch in &scene.geometry.batches {
        ensure!(
            batch.model.iter().all(|v| v.is_finite())
                && [
                    batch.model[3],
                    batch.model[7],
                    batch.model[11],
                    batch.model[15]
                ] == [0., 0., 0., 1.],
            "invalid affine model transform"
        );
        ensure!(
            u64::from(batch.first_index) + u64::from(batch.index_count)
                <= scene.geometry.indices.len() as u64,
            "render batch range out of bounds"
        );
        ensure!(
            materials.contains_key(&batch.binding_index),
            "missing material image"
        );
    }
    for material in materials.values() {
        let i = &material.image;
        ensure!(
            i.width > 0
                && i.height > 0
                && i.width <= 8192
                && i.height <= 8192
                && i.pixels.len() as u64 == u64::from(i.width) * u64::from(i.height) * 4,
            "invalid material image"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/renderer.rs"]
mod tests;
