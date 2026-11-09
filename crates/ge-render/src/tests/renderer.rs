use super::*;
use ge_assets::scene::{Batch, Geometry, Vertex};

fn fixture() -> (Scene, BTreeMap<usize, MaterialImage>, Camera) {
    let vertices = [0.5f32, 0.]
        .into_iter()
        .flat_map(|z| [[-1., -1., z], [1., -1., z], [0., 1., z]])
        .map(|position| Vertex {
            position,
            uv: [0.5; 2],
            color: [255; 4],
            position_index: 0,
            color_index: 0,
            normal_index: 0,
            packed_normal_raw: 0,
        })
        .collect();
    let batches = (0..2)
        .map(|i| Batch {
            model: ge_assets::transform::IDENTITY,
            placement_index: None,
            entity_address: 0,
            entity_flags: 0,
            index_format_raw: 0x30000203,
            binding_index: i,
            raw_binding_index: i as u16,
            flags: 0,
            transparency: 0,
            declared_triangles: 1,
            unknown_words: [0; 5],
            first_index: i as u32 * 3,
            index_count: 3,
        })
        .collect();
    let scene = Scene {
        map_hash: 0,
        zones: vec![],
        placements_not_loaded: 0,
        placements_loaded: 0,
        render_placements: 0,
        navigation_placements: 0,
        nav_meshes: vec![],
        placements: vec![],
        geometry: Geometry {
            texture_bindings: vec![0, 1],
            meshes: 1,
            splits: 0,
            zone_links: 0,
            shared_visits: 0,
            bounds: [[-1., -1., 0.], [1., 1., 0.5]],
            material_references: vec![],
            batches,
            vertices,
            indices: vec![0, 1, 2, 3, 4, 5],
        },
    };
    let materials = [(0, [255, 0, 0, 255]), (1, [0, 255, 0, 255])]
        .into_iter()
        .map(|(i, pixel)| {
            (
                i,
                MaterialImage {
                    image: RgbaImage {
                        width: 2,
                        height: 2,
                        pixels: pixel.repeat(4),
                    },
                    wrap_s: 0,
                    wrap_t: 0,
                },
            )
        })
        .collect();
    let camera = Camera {
        eye: [0., 0., 3.],
        target: [0.; 3],
        vertical_fov_degrees: 60.,
        near: 0.1,
        far: 10.,
    };
    (scene, materials, camera)
}

#[test]
fn invalid_geometry_and_images_fail_before_gpu_use() {
    let (mut scene, mut materials, _) = fixture();
    assert!(validate(&scene, &materials, 65, 65).is_ok());
    assert!(validate(&scene, &materials, 0, 65).is_err());
    scene.geometry.indices[0] = 99;
    assert!(validate(&scene, &materials, 65, 65).is_err());
    scene.geometry.indices[0] = 0;
    scene.geometry.batches[0].index_count = u32::MAX;
    assert!(validate(&scene, &materials, 65, 65).is_err());
    scene.geometry.batches[0].index_count = 3;
    scene.geometry.batches[0].model[0] = f32::NAN;
    assert!(validate(&scene, &materials, 65, 65).is_err());
    scene.geometry.batches[0].model = ge_assets::transform::IDENTITY;
    materials.get_mut(&0).unwrap().image.pixels.pop();
    assert!(validate(&scene, &materials, 65, 65).is_err());
    assert!(wrap(3).is_err());
}

#[test]
fn preview_shader_parses_and_validates_without_a_gpu() {
    let module = naga::front::wgsl::parse_str(include_str!("../preview.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn native_texture_depth_alpha_and_unaligned_readback() {
    let (scene, mut materials, camera) = fixture();
    let output = pollster::block_on(render(&scene, &materials, camera, 65, 65)).unwrap();
    println!(
        "native adapter: {} ({})",
        output.evidence.adapter, output.evidence.backend
    );
    assert_eq!(output.image.pixels.len(), 65 * 65 * 4);
    let center = (32 * 65 + 32) * 4;
    assert_eq!(&output.image.pixels[center..center + 4], &[255, 0, 0, 255]);
    assert!(output.image.pixels[0] < 25);
    assert_eq!(&output.image.pixels[..4], &BACKGROUND);
    assert!(
        output.evidence.non_background_pixels > 0
            && output.evidence.non_background_pixels < 65 * 65
    );
    materials
        .get_mut(&0)
        .unwrap()
        .image
        .pixels
        .chunks_exact_mut(4)
        .for_each(|p| p[3] = 0);
    let output = pollster::block_on(render(&scene, &materials, camera, 65, 65)).unwrap();
    assert_eq!(&output.image.pixels[center..center + 4], &[0, 255, 0, 255]);
    // Reuse one triangle's indices/vertices with two different model transforms.
    let (mut scene, materials, camera) = fixture();
    scene.geometry.vertices.truncate(3);
    for v in &mut scene.geometry.vertices {
        v.position[2] = 0.;
    }
    scene.geometry.indices = vec![0, 1, 2];
    for (index, batch) in scene.geometry.batches.iter_mut().enumerate() {
        batch.first_index = 0;
        batch.model = ge_assets::transform::placement(
            [if index == 0 { -0.8 } else { 0.8 }, 0., 0.],
            [0.; 3],
            [0.5; 3],
        )
        .unwrap();
    }
    let output = pollster::block_on(render(&scene, &materials, camera, 129, 65)).unwrap();
    assert_eq!(output.evidence.model_groups, 2);
    assert_eq!(output.evidence.vertices, 3);
    assert_eq!(output.evidence.unique_triangles, 1);
    assert_eq!(output.evidence.triangles, 2);
    assert_eq!(
        &output.image.pixels[(32 * 129 + 49) * 4..(32 * 129 + 49) * 4 + 4],
        &[255, 0, 0, 255]
    );
    assert_eq!(
        &output.image.pixels[(32 * 129 + 79) * 4..(32 * 129 + 79) * 4 + 4],
        &[0, 255, 0, 255]
    );
    pollster::block_on(persistent_bgra_scene_survives_camera_and_target_changes());
}

async fn persistent_bgra_scene_survives_camera_and_target_changes() {
    let (scene, materials, camera) = fixture();
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap();
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let gpu = GpuScene::upload(
        &device,
        &queue,
        &scene,
        &materials,
        wgpu::TextureFormat::Bgra8Unorm,
    )
    .unwrap();
    let frame = |camera, w, h| {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Persistent BGRA fixture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = depth_texture(&device, w, h).unwrap();
        gpu.update_camera(&queue, camera, w as f32 / h as f32)
            .unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        gpu.encode(
            &mut encoder,
            &target.create_view(&Default::default()),
            &depth.create_view(&Default::default()),
        );
        queue.submit(Some(encoder.finish()));
        readback(&device, &queue, &target).unwrap()
    };
    let initial = frame(camera, 65, 65);
    assert_eq!(
        &initial.pixels[(32 * 65 + 32) * 4..(32 * 65 + 32) * 4 + 4],
        &[255, 0, 0, 255]
    );
    let shifted = Camera {
        eye: [2., 0., 3.],
        target: [2., 0., 0.],
        ..camera
    };
    let changed = frame(shifted, 129, 65);
    assert_eq!(
        &changed.pixels[(32 * 129 + 64) * 4..(32 * 129 + 64) * 4 + 4],
        &BACKGROUND
    );
    let reset = frame(camera, 65, 65);
    assert_eq!(reset.pixels, initial.pixels);
    assert!(validation.pop().await.is_none());
}
