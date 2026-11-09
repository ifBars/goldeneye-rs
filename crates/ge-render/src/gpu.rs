//! Persistent GPU resources shared by offscreen capture and native presentation.
use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct SceneStats {
    pub vertices: usize,
    pub triangles: usize,
    pub draws: usize,
    pub textures: usize,
    pub model_groups: usize,
    pub unique_triangles: usize,
}
struct Draw {
    indices: std::ops::Range<u32>,
    material: usize,
    model: usize,
}

pub struct GpuScene {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    model_groups: Vec<wgpu::BindGroup>,
    materials: BTreeMap<usize, wgpu::BindGroup>,
    draws: Vec<Draw>,
    pub stats: SceneStats,
}
impl GpuScene {
    pub fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        materials: &BTreeMap<usize, MaterialImage>,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        validate(scene, materials, 1, 1)?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Independent unlit preview"),
            source: wgpu::ShaderSource::Wgsl(include_str!("preview.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Static map textured preview"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x2,2=>Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let mut vertex_bytes = Vec::with_capacity(scene.geometry.vertices.len() * 36);
        for v in &scene.geometry.vertices {
            for f in v
                .position
                .into_iter()
                .chain(v.uv)
                .chain(v.color.map(|c| c as f32 / 255.0))
            {
                vertex_bytes.extend(f.to_ne_bytes());
            }
        }
        let index_bytes = scene
            .geometry
            .indices
            .iter()
            .flat_map(|i| i.to_ne_bytes())
            .collect::<Vec<_>>();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Map vertices"),
            contents: &vertex_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Map triangle indices"),
            contents: &index_bytes,
            usage: wgpu::BufferUsages::INDEX,
        });
        let mut model_lookup = BTreeMap::new();
        let mut model_groups = Vec::new();
        let mut batch_models = Vec::new();
        for batch in &scene.geometry.batches {
            let key = batch.model.map(f32::to_bits);
            let group_index = if let Some(&index) = model_lookup.get(&key) {
                index
            } else {
                let bytes = batch
                    .model
                    .into_iter()
                    .flat_map(f32::to_ne_bytes)
                    .collect::<Vec<_>>();
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Model matrix"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Model transform"),
                    layout: &pipeline.get_bind_group_layout(2),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                });
                let index = model_groups.len();
                model_groups.push(group);
                model_lookup.insert(key, index);
                index
            };
            batch_models.push(group_index);
        }
        let mut groups = BTreeMap::new();
        for (&binding, material) in materials {
            let image = &material.image;
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("User-supplied decoded TPL"),
                size: wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Decoded asset bytes are RGBA regardless of the presentation format.
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
                texture.size(),
            );
            let view = texture.create_view(&Default::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wrap(material.wrap_s)?,
                address_mode_v: wrap(material.wrap_t)?,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Map material"),
                layout: &pipeline.get_bind_group_layout(1),
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
            });
            groups.insert(binding, group);
        }

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera view projection"),
            contents: &[0; 64],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let draws = scene
            .geometry
            .batches
            .iter()
            .enumerate()
            .map(|(i, b)| Draw {
                indices: b.first_index..b.first_index + b.index_count,
                material: b.binding_index,
                model: batch_models[i],
            })
            .collect();
        let stats = SceneStats {
            vertices: scene.geometry.vertices.len(),
            triangles: scene.triangle_count(),
            draws: scene.geometry.batches.len(),
            textures: materials.len(),
            model_groups: model_groups.len(),
            unique_triangles: scene.geometry.indices.len() / 3,
        };
        Ok(Self {
            pipeline,
            vertices,
            indices,
            camera_buffer,
            camera_group,
            model_groups,
            materials: groups,
            draws,
            stats,
        })
    }
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: Camera, aspect: f32) -> Result<()> {
        let bytes = camera
            .matrix(aspect)?
            .into_iter()
            .flat_map(f32::to_ne_bytes)
            .collect::<Vec<_>>();
        queue.write_buffer(&self.camera_buffer, 0, &bytes);
        Ok(())
    }
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Map inspection"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: BACKGROUND[0] as f64 / 255.,
                        g: BACKGROUND[1] as f64 / 255.,
                        b: BACKGROUND[2] as f64 / 255.,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(0, &self.camera_group, &[]);
        for draw in &self.draws {
            pass.set_bind_group(1, &self.materials[&draw.material], &[]);
            pass.set_bind_group(2, &self.model_groups[draw.model], &[]);
            pass.draw_indexed(draw.indices.clone(), 0, 0..1);
        }
    }
}

pub fn depth_texture(device: &wgpu::Device, width: u32, height: u32) -> Result<wgpu::Texture> {
    ensure!(
        width > 0
            && height > 0
            && width <= device.limits().max_texture_dimension_2d
            && height <= device.limits().max_texture_dimension_2d,
        "invalid depth extent"
    );
    Ok(device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Map depth"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    }))
}
