//! Ordered 2D frontend layers. Layout/input live in the application/core.
use anyhow::{Result, ensure};
use ge_assets::tpl::RgbaImage;
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

/// Coordinates use a 640x480 reference canvas; the native viewport letterboxes it.
pub struct Quad {
    pub texture: u32,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}
/// A projected point on the reference canvas, with unmodified texture UVs.
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    /// Positive clip-space W retained after the caller projects canvas XY.
    /// Texture and color interpolation use its reciprocal perspective weights.
    pub clip_w: f32,
}
/// Explicit triangle-list vertices. Source strip expansion/projection belongs
/// to the caller; the renderer neither repairs winding nor fills a rectangle.
pub struct Mesh {
    pub texture: u32,
    pub vertices: Vec<Vertex>,
    pub blend: Blend,
    pub rgb_scale: RgbScale,
}
/// Material modulation scale, applied after texture/raster multiplication and
/// clamped before framebuffer blending. Alpha is not scaled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbScale {
    One,
    Two,
}
impl RgbScale {
    fn value(self) -> f32 {
        match self {
            Self::One => 1.,
            Self::Two => 2.,
        }
    }
}
/// RGB blend equations selected by the caller's decoded material metadata.
/// Frame alpha keeps the existing opaque-window composition convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    Additive,
}
pub enum Layer {
    Quad(Quad),
    Mesh(Mesh),
}
impl From<Quad> for Layer {
    fn from(value: Quad) -> Self {
        Self::Quad(value)
    }
}
impl From<Mesh> for Layer {
    fn from(value: Mesh) -> Self {
        Self::Mesh(value)
    }
}
#[derive(Clone, Copy)]
enum LayerRef<'a> {
    Quad(&'a Quad),
    Mesh(&'a Mesh),
}

const MAX_LAYERS: usize = 16384;
const MAX_VERTICES: usize = 1_048_576;
const VERTEX_BYTES: usize = 40;

fn append_vertex(bytes: &mut Vec<u8>, vertex: Vertex, scale: RgbScale) -> Result<()> {
    ensure!(
        vertex.clip_w.is_finite() && vertex.clip_w > 0.,
        "frontend clip W must be finite and positive"
    );
    let [x, y] = vertex.position;
    let position = [x / 320. - 1., 1. - y / 240.];
    ensure!(
        position
            .iter()
            .chain(&vertex.uv)
            .chain(&vertex.color)
            .all(|v| v.is_finite()),
        "non-finite frontend layer"
    );
    ensure!(
        position
            .iter()
            .all(|value| (value * vertex.clip_w).is_finite()),
        "non-finite frontend clip position"
    );
    for value in position.into_iter().chain(vertex.uv).chain(vertex.color) {
        bytes.extend(value.to_ne_bytes());
    }
    bytes.extend(scale.value().to_ne_bytes());
    bytes.extend(vertex.clip_w.to_ne_bytes());
    Ok(())
}
pub struct Frontend {
    pipeline: wgpu::RenderPipeline,
    additive_pipeline: wgpu::RenderPipeline,
    textures: BTreeMap<u32, wgpu::BindGroup>,
    resident_images: BTreeMap<u32, wgpu::Texture>,
}
impl Frontend {
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &BTreeMap<u32, RgbaImage>,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Frontend layers"),
            source: wgpu::ShaderSource::Wgsl(include_str!("frontend.wgsl").into()),
        });
        // Both pipelines must use the SAME explicit layout. Automatically
        // inferred layouts are pipeline-exclusive and cannot share bind groups.
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shared frontend images"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shared frontend pipeline layout"),
            bind_group_layouts: &[Some(&image_layout)],
            immediate_size: 0,
        });
        let make_pipeline = |blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Ordered alpha frontend"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_BYTES as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x4,3=>Float32,4=>Float32],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
        };
        let pipeline = make_pipeline(wgpu::BlendState::ALPHA_BLENDING);
        let additive_pipeline = make_pipeline(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendState::ALPHA_BLENDING.alpha,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Frontend linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut textures = BTreeMap::new();
        let mut resident_images = BTreeMap::new();
        for (&id, image) in images {
            ensure!(
                image.width > 0 && image.height > 0 && image.width <= 4096 && image.height <= 4096,
                "frontend image dimensions out of range"
            );
            ensure!(
                image.pixels.len() == image.width as usize * image.height as usize * 4,
                "frontend image byte count differs"
            );
            let extent = wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Frontend local asset"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &image.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width * 4),
                    rows_per_image: Some(image.height),
                },
                extent,
            );
            let view = texture.create_view(&Default::default());
            textures.insert(
                id,
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Frontend image"),
                    layout: &image_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                }),
            );
            resident_images.insert(id, texture);
        }
        Ok(Self {
            pipeline,
            additive_pipeline,
            textures,
            resident_images,
        })
    }
    /// Replace pixels while preserving the sampled texture and bind group.
    pub fn update_image(&self, queue: &wgpu::Queue, id: u32, image: &RgbaImage) -> Result<()> {
        let target = self
            .resident_images
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("missing frontend image {id:08x}"))?;
        ensure!(
            image.width == target.width() && image.height == target.height(),
            "frontend update dimensions differ"
        );
        ensure!(
            image.pixels.len() == image.width as usize * image.height as usize * 4,
            "frontend update byte count differs"
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            target.size(),
        );
        Ok(())
    }
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        quads: &[Quad],
    ) -> Result<()> {
        self.encode_iter(
            device,
            encoder,
            target,
            size,
            quads.iter().map(LayerRef::Quad),
        )
    }
    /// Submit quads and triangle meshes in precisely their supplied layer order.
    pub fn encode_layers(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        layers: &[Layer],
    ) -> Result<()> {
        self.encode_iter(
            device,
            encoder,
            target,
            size,
            layers.iter().map(|layer| match layer {
                Layer::Quad(quad) => LayerRef::Quad(quad),
                Layer::Mesh(mesh) => LayerRef::Mesh(mesh),
            }),
        )
    }
    fn encode_iter<'a>(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        layers: impl IntoIterator<Item = LayerRef<'a>>,
    ) -> Result<()> {
        ensure!(size[0] > 0 && size[1] > 0, "empty frontend target");
        let mut bytes = Vec::new();
        let mut draws = Vec::new();
        for layer in layers {
            ensure!(draws.len() < MAX_LAYERS, "frontend layer budget exceeded");
            let (texture, count, blend) = match layer {
                LayerRef::Quad(quad) => (quad.texture, 6, Blend::Alpha),
                LayerRef::Mesh(mesh) => {
                    ensure!(
                        mesh.vertices.len().is_multiple_of(3),
                        "frontend mesh requires complete triangles"
                    );
                    (mesh.texture, mesh.vertices.len(), mesh.blend)
                }
            };
            ensure!(
                self.textures.contains_key(&texture),
                "missing frontend texture {:08x}",
                texture
            );
            let first = bytes.len() / VERTEX_BYTES;
            ensure!(
                count <= MAX_VERTICES.saturating_sub(first),
                "frontend vertex budget exceeded"
            );
            match layer {
                LayerRef::Mesh(mesh) => {
                    for &vertex in &mesh.vertices {
                        append_vertex(&mut bytes, vertex, mesh.rgb_scale)?;
                    }
                }
                LayerRef::Quad(quad) => {
                    ensure!(
                        quad.rect
                            .iter()
                            .chain(&quad.uv)
                            .chain(&quad.color)
                            .all(|v| v.is_finite()),
                        "non-finite frontend layer"
                    );
                    let [x, y, w, h] = quad.rect;
                    let [u, v, s, t] = quad.uv;
                    for (px, py, tu, tv) in [
                        (x, y, u, v),
                        (x + w, y, s, v),
                        (x, y + h, u, t),
                        (x, y + h, u, t),
                        (x + w, y, s, v),
                        (x + w, y + h, s, t),
                    ] {
                        append_vertex(
                            &mut bytes,
                            Vertex {
                                position: [px, py],
                                uv: [tu, tv],
                                color: quad.color,
                                clip_w: 1.,
                            },
                            RgbScale::One,
                        )?;
                    }
                }
            }
            draws.push((texture, first as u32..(first + count) as u32, blend));
        }
        // wgpu does not permit zero-sized bound vertex buffers.
        if bytes.is_empty() {
            bytes.resize(VERTEX_BYTES, 0);
        }
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Frontend frame vertices"),
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Frontend"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let scale = (size[0] as f32 / 640.).min(size[1] as f32 / 480.);
        let (w, h) = (640. * scale, 480. * scale);
        pass.set_viewport(
            (size[0] as f32 - w) * 0.5,
            (size[1] as f32 - h) * 0.5,
            w,
            h,
            0.,
            1.,
        );
        pass.set_vertex_buffer(0, vertices.slice(..));
        for (texture, range, blend) in draws {
            pass.set_pipeline(match blend {
                Blend::Alpha => &self.pipeline,
                Blend::Additive => &self.additive_pipeline,
            });
            pass.set_bind_group(0, &self.textures[&texture], &[]);
            pass.draw(range, 0..1);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/frontend.rs"]
mod tests;
