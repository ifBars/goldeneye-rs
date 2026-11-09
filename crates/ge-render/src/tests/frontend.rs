use super::*;

const RED: u32 = 7;
const GREEN: u32 = 19;
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn images() -> BTreeMap<u32, RgbaImage> {
    BTreeMap::from([
        (
            RED,
            RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![255, 0, 0, 255],
            },
        ),
        (
            GREEN,
            RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![0, 255, 0, 128],
            },
        ),
    ])
}

fn quad(texture: u32, rect: [f32; 4], alpha: f32) -> Quad {
    Quad {
        texture,
        rect,
        uv: [0., 0., 1., 1.],
        color: [1., 1., 1., alpha],
    }
}

async fn device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();
    println!("frontend test adapter: {:?}", adapter.get_info());
    adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap()
}

fn target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    [width, height]: [u32; 2],
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Authored frontend readback target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu: &Frontend,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    quads: &[Quad],
) -> RgbaImage {
    let target = target(device, format, size);
    let mut encoder = device.create_command_encoder(&Default::default());
    gpu.encode(
        device,
        &mut encoder,
        &target.create_view(&Default::default()),
        size,
        quads,
    )
    .unwrap();
    queue.submit(Some(encoder.finish()));
    crate::readback(device, queue, &target).unwrap()
}

fn layer_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gpu: &Frontend,
    format: wgpu::TextureFormat,
    size: [u32; 2],
    layers: &[Layer],
) -> RgbaImage {
    let target = target(device, format, size);
    let mut encoder = device.create_command_encoder(&Default::default());
    gpu.encode_layers(
        device,
        &mut encoder,
        &target.create_view(&Default::default()),
        size,
        layers,
    )
    .unwrap();
    queue.submit(Some(encoder.finish()));
    crate::readback(device, queue, &target).unwrap()
}

// Six authored strip corners form a sloped hexagon. Expanding the raw strip
// deliberately retains both windings; the frontend must not cull half of it.
fn six_corner_mesh(texture: u32, alpha: f32) -> Mesh {
    let strip = [
        [80., 80.],
        [80., 400.],
        [240., 80.],
        [240., 280.],
        [400., 160.],
        [400., 280.],
    ];
    let mut vertices = Vec::new();
    for first in 0..4 {
        for position in &strip[first..first + 3] {
            vertices.push(Vertex {
                position: *position,
                uv: [(position[0] - 80.) / 320., (position[1] - 80.) / 320.],
                color: [1., 1., 1., alpha],
                clip_w: 1.,
            });
        }
    }
    Mesh {
        texture,
        vertices,
        blend: Blend::Alpha,
        rgb_scale: RgbScale::One,
    }
}

fn pixel(image: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width + x) * 4) as usize;
    image.pixels[at..at + 4].try_into().unwrap()
}

fn near(actual: [u8; 4], expected: [u8; 4]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 1,
            "channel {actual} differs from {expected}"
        );
    }
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn movie_pixels_update_without_rebuilding_texture_bindings() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let gpu =
            Frontend::upload(&device, &queue, &images(), wgpu::TextureFormat::Rgba8Unorm).unwrap();
        let layers = [quad(RED, [0., 0., 640., 480.], 1.)];
        let first = frame(
            &device,
            &queue,
            &gpu,
            wgpu::TextureFormat::Rgba8Unorm,
            [80, 60],
            &layers,
        );
        assert_eq!(pixel(&first, 40, 30), [255, 0, 0, 255]);
        gpu.update_image(
            &queue,
            RED,
            &RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![0, 0, 255, 255],
            },
        )
        .unwrap();
        let second = frame(
            &device,
            &queue,
            &gpu,
            wgpu::TextureFormat::Rgba8Unorm,
            [80, 60],
            &layers,
        );
        assert_eq!(pixel(&second, 40, 30), [0, 0, 255, 255]);
        assert!(
            gpu.update_image(
                &queue,
                RED,
                &RgbaImage {
                    width: 2,
                    height: 1,
                    pixels: vec![0; 8]
                }
            )
            .is_err()
        );
        assert!(
            gpu.update_image(
                &queue,
                999,
                &RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![0; 4]
                }
            )
            .is_err()
        );
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn rgba_and_bgra_preserve_texture_channels_vertex_alpha_and_ordered_layers() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
            let mut quads = vec![
                quad(RED, [0., 0., 640., 480.], 1.),
                quad(GREEN, [160., 120., 320., 240.], 0.5),
                quad(RED, [240., 180., 160., 120.], 0.5),
            ];
            let image = frame(&device, &queue, &gpu, format, [128, 96], &quads);
            assert_eq!(pixel(&image, 10, 10), [255, 0, 0, 255]);
            // Texture alpha128/255 is multiplied by vertex alpha0.5. Straight
            // source-over then gives one-quarter green over opaque red.
            near(pixel(&image, 36, 30), [191, 64, 0, 255]);
            near(pixel(&image, 64, 48), [223, 32, 0, 255]);
            assert_eq!(pixel(&image, 110, 80), [255, 0, 0, 255]);
            // Reordering only the two translucent layers changes the center;
            // depth, opaque blending or texture-ID sorting would hide this.
            quads.swap(1, 2);
            let reversed = frame(&device, &queue, &gpu, format, [128, 96], &quads);
            near(pixel(&reversed, 64, 48), [191, 64, 0, 255]);
            assert_ne!(pixel(&image, 64, 48), pixel(&reversed, 64, 48));
            let opaque_vertex = frame(
                &device,
                &queue,
                &gpu,
                format,
                [128, 96],
                &[
                    quad(RED, [0., 0., 640., 480.], 1.),
                    quad(GREEN, [160., 120., 320., 240.], 1.),
                ],
            );
            near(pixel(&opaque_vertex, 64, 48), [127, 128, 0, 255]);
        }
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn four_three_canvas_letterboxes_wide_and_tall_targets_and_empty_frames_clear() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let format = wgpu::TextureFormat::Bgra8Unorm;
        let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
        let quads = [quad(RED, [0., 0., 640., 480.], 1.)];
        // Both widths require padded copy rows; sampled rows also catch padding
        // accidentally retained in the owned readback image.
        let wide = frame(&device, &queue, &gpu, format, [160, 90], &quads);
        assert_eq!(
            (wide.width, wide.height, wide.pixels.len()),
            (160, 90, 160 * 90 * 4)
        );
        for (x, y) in [(0, 0), (19, 45), (140, 45), (159, 89)] {
            assert_eq!(pixel(&wide, x, y), BLACK);
        }
        for (x, y) in [(21, 1), (80, 45), (138, 88)] {
            assert_eq!(pixel(&wide, x, y), [255, 0, 0, 255]);
        }
        let tall = frame(&device, &queue, &gpu, format, [96, 128], &quads);
        assert_eq!(
            (tall.width, tall.height, tall.pixels.len()),
            (96, 128, 96 * 128 * 4)
        );
        for (x, y) in [(0, 0), (48, 27), (48, 100), (95, 127)] {
            assert_eq!(pixel(&tall, x, y), BLACK);
        }
        for (x, y) in [(1, 29), (48, 64), (94, 98)] {
            assert_eq!(pixel(&tall, x, y), [255, 0, 0, 255]);
        }
        let empty = frame(&device, &queue, &gpu, format, [96, 128], &[]);
        assert!(empty.pixels.chunks_exact(4).all(|pixel| pixel == BLACK));
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn invalid_layers_images_and_target_sizes_error_without_gpu_validation_failures() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
        let target = target(&device, format, [16, 12]);
        let view = target.create_view(&Default::default());
        let invalid = |size, quads: &[Quad]| {
            let mut encoder = device.create_command_encoder(&Default::default());
            gpu.encode(&device, &mut encoder, &view, size, quads)
                .unwrap_err()
                .to_string()
        };
        assert!(
            invalid([16, 12], &[quad(999, [0., 0., 20., 20.], 1.)])
                .contains("missing frontend texture")
        );
        assert!(invalid([0, 12], &[]).contains("empty frontend target"));
        assert!(invalid([16, 0], &[]).contains("empty frontend target"));
        for field in 0..3 {
            let mut layer = quad(RED, [0., 0., 20., 20.], 1.);
            match field {
                0 => layer.rect[0] = f32::NAN,
                1 => layer.uv[1] = f32::INFINITY,
                _ => layer.color[3] = f32::NEG_INFINITY,
            }
            assert!(invalid([16, 12], &[layer]).contains("non-finite frontend layer"));
        }
        let too_many = (0..16385)
            .map(|_| quad(RED, [0., 0., 1., 1.], 1.))
            .collect::<Vec<_>>();
        assert!(invalid([16, 12], &too_many).contains("frontend layer budget"));
        for bad in [
            RgbaImage {
                width: 0,
                height: 1,
                pixels: vec![],
            },
            RgbaImage {
                width: 4097,
                height: 1,
                pixels: vec![],
            },
            RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![255, 0, 0],
            },
        ] {
            assert!(
                Frontend::upload(&device, &queue, &BTreeMap::from([(RED, bad)]), format).is_err()
            );
        }
        // Failure is host-side validation, so a following valid encode succeeds.
        let good = frame(
            &device,
            &queue,
            &gpu,
            format,
            [16, 12],
            &[quad(RED, [0., 0., 640., 480.], 1.)],
        );
        assert_eq!(pixel(&good, 8, 6), [255, 0, 0, 255]);
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn six_corner_strip_preserves_nonrectangular_coverage_and_texture_uv_orientation() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let source = BTreeMap::from([(
                29,
                RgbaImage {
                    width: 2,
                    height: 2,
                    pixels: vec![
                        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
                    ],
                },
            )]);
            let gpu = Frontend::upload(&device, &queue, &source, format).unwrap();
            let image = layer_frame(
                &device,
                &queue,
                &gpu,
                format,
                [160, 120],
                &[six_corner_mesh(29, 1.).into()],
            );
            assert_eq!(pixel(&image, 30, 30), [255, 0, 0, 255]);
            assert_eq!(pixel(&image, 90, 38), [0, 255, 0, 255]);
            assert_eq!(pixel(&image, 30, 90), [0, 0, 255, 255]);
            // These lie inside the mesh's bounding rectangle but outside its
            // authored strip, catching rectangle substitution and triangle fans.
            assert_eq!(pixel(&image, 90, 25), BLACK);
            assert_eq!(pixel(&image, 90, 90), BLACK);
            assert_eq!(pixel(&image, 65, 85), BLACK);
            let white = BTreeMap::from([(
                41,
                RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![255; 4],
                },
            )]);
            let colored_gpu = Frontend::upload(&device, &queue, &white, format).unwrap();
            let mut colored_mesh = six_corner_mesh(41, 1.);
            for vertex in &mut colored_mesh.vertices {
                let blue = (vertex.position[0] - 80.) / 320.;
                vertex.color = [1. - blue, 0., blue, 1.];
            }
            let colored = layer_frame(
                &device,
                &queue,
                &colored_gpu,
                format,
                [160, 120],
                &[colored_mesh.into()],
            );
            // Pixel center x242 is 162/320 across the shape: vertex color
            // interpolates before texture multiplication, rather than flattening
            // each triangle to one corner's tint.
            near(pixel(&colored, 60, 45), [126, 0, 129, 255]);
        }
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn meshes_and_quads_keep_shared_texture_updates_vertex_alpha_and_interleaved_order() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
            let mut layers = vec![
                quad(RED, [0., 0., 640., 480.], 1.).into(),
                six_corner_mesh(GREEN, 0.5).into(),
                quad(RED, [160., 120., 160., 120.], 0.5).into(),
            ];
            let image = layer_frame(&device, &queue, &gpu, format, [160, 120], &layers);
            near(pixel(&image, 60, 45), [223, 32, 0, 255]);
            near(pixel(&image, 30, 30), [191, 64, 0, 255]);
            assert_eq!(pixel(&image, 90, 90), [255, 0, 0, 255]);
            layers.swap(1, 2);
            let reverse = layer_frame(&device, &queue, &gpu, format, [160, 120], &layers);
            near(pixel(&reverse, 60, 45), [191, 64, 0, 255]);
            // Meshes use the same persistent image bind group as old quads.
            gpu.update_image(
                &queue,
                GREEN,
                &RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![0, 0, 255, 128],
                },
            )
            .unwrap();
            let updated = layer_frame(&device, &queue, &gpu, format, [160, 120], &layers);
            near(pixel(&updated, 60, 45), [191, 0, 64, 255]);
        }
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn malformed_meshes_and_vertex_budget_fail_before_gpu_encoding() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
        let target = target(&device, format, [16, 12]);
        let view = target.create_view(&Default::default());
        let invalid = |mesh: Mesh| {
            let mut encoder = device.create_command_encoder(&Default::default());
            gpu.encode_layers(&device, &mut encoder, &view, [16, 12], &[mesh.into()])
                .unwrap_err()
                .to_string()
        };
        let mut partial = six_corner_mesh(RED, 1.);
        partial.vertices.pop();
        assert!(invalid(partial).contains("complete triangles"));
        for field in 0..3 {
            let mut mesh = six_corner_mesh(RED, 1.);
            match field {
                0 => mesh.vertices[0].position[0] = f32::INFINITY,
                1 => mesh.vertices[0].uv[1] = f32::NAN,
                _ => mesh.vertices[0].color[3] = f32::NEG_INFINITY,
            };
            assert!(invalid(mesh).contains("non-finite frontend layer"));
        }
        for clip_w in [0., -0., -1., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut mesh = six_corner_mesh(RED, 1.);
            mesh.vertices[0].clip_w = clip_w;
            assert!(invalid(mesh).contains("clip W must be finite and positive"));
        }
        let mut overflowing_clip = six_corner_mesh(RED, 1.);
        overflowing_clip.vertices[0].position[0] = f32::MAX;
        overflowing_clip.vertices[0].clip_w = 1e20;
        assert!(invalid(overflowing_clip).contains("non-finite frontend clip position"));
        assert!(invalid(six_corner_mesh(999, 1.)).contains("missing frontend texture"));
        let vertex = Vertex {
            position: [0., 0.],
            uv: [0., 0.],
            color: [1.; 4],
            clip_w: 1.,
        };
        let too_many = Mesh {
            texture: RED,
            vertices: vec![vertex; 1_048_578],
            blend: Blend::Alpha,
            rgb_scale: RgbScale::One,
        };
        assert!(invalid(too_many).contains("vertex budget"));
        let empty = layer_frame(
            &device,
            &queue,
            &gpu,
            format,
            [16, 12],
            &[Mesh {
                texture: RED,
                vertices: vec![],
                blend: Blend::Alpha,
                rgb_scale: RgbScale::One,
            }
            .into()],
        );
        assert!(empty.pixels.chunks_exact(4).all(|pixel| pixel == BLACK));
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn additive_meshes_use_source_alpha_plus_destination_and_keep_surrounding_quad_order() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let gpu = Frontend::upload(&device, &queue, &images(), format).unwrap();
            let mesh = |blend| {
                let mut mesh = six_corner_mesh(GREEN, 0.5);
                mesh.blend = blend;
                Layer::Mesh(mesh)
            };
            let alpha = layer_frame(
                &device,
                &queue,
                &gpu,
                format,
                [160, 120],
                &[
                    quad(RED, [0., 0., 640., 480.], 1.).into(),
                    mesh(Blend::Alpha),
                ],
            );
            let additive = layer_frame(
                &device,
                &queue,
                &gpu,
                format,
                [160, 120],
                &[
                    quad(RED, [0., 0., 640., 480.], 1.).into(),
                    mesh(Blend::Additive),
                ],
            );
            // Identical source color/UV/alpha differs only in destination factor.
            near(pixel(&alpha, 60, 45), [191, 64, 0, 255]);
            near(pixel(&additive, 60, 45), [255, 64, 0, 255]);
            let mut mixed = vec![
                quad(RED, [0., 0., 640., 480.], 1.).into(),
                mesh(Blend::Additive),
                quad(RED, [160., 120., 160., 120.], 0.5).into(),
            ];
            let overlay = layer_frame(&device, &queue, &gpu, format, [160, 120], &mixed);
            near(pixel(&overlay, 60, 45), [255, 32, 0, 255]);
            mixed.swap(1, 2);
            let reverse = layer_frame(&device, &queue, &gpu, format, [160, 120], &mixed);
            near(pixel(&reverse, 60, 45), [255, 64, 0, 255]);
            assert_eq!(pixel(&reverse, 90, 90), [255, 0, 0, 255]);
            gpu.update_image(
                &queue,
                GREEN,
                &RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![0, 0, 255, 128],
                },
            )
            .unwrap();
            let updated = layer_frame(&device, &queue, &gpu, format, [160, 120], &mixed);
            near(pixel(&updated, 60, 45), [255, 0, 64, 255]);
        }
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn rgb_material_scale_doubles_neutral_color_and_clamps_before_blending_without_scaling_alpha() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let mut source = images();
            source.insert(
                77,
                RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![0, 0, 255, 255],
                },
            );
            source.insert(
                78,
                RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![128, 0, 0, 255],
                },
            );
            let gpu = Frontend::upload(&device, &queue, &source, format).unwrap();
            let render = |texture, tint, alpha, rgb_scale, blend| {
                let mut mesh = six_corner_mesh(texture, alpha);
                mesh.rgb_scale = rgb_scale;
                mesh.blend = blend;
                for vertex in &mut mesh.vertices {
                    vertex.color = [tint, tint, tint, alpha];
                }
                layer_frame(
                    &device,
                    &queue,
                    &gpu,
                    format,
                    [160, 120],
                    &[quad(77, [0., 0., 640., 480.], 1.).into(), mesh.into()],
                )
            };
            let one = render(RED, 127. / 255., 1., RgbScale::One, Blend::Alpha);
            let two = render(RED, 127. / 255., 1., RgbScale::Two, Blend::Alpha);
            near(pixel(&one, 60, 45), [127, 0, 0, 255]);
            near(pixel(&two, 60, 45), [254, 0, 0, 255]);
            // .75*2 exceeds one. It must clamp BEFORE source-alpha blending,
            // yielding .5 red over blue rather than .75 red over blue.
            let saturated = render(RED, 0.75, 0.5, RgbScale::Two, Blend::Alpha);
            near(pixel(&saturated, 60, 45), [128, 0, 127, 255]);
            let quarter_alpha = render(RED, 0.75, 0.25, RgbScale::Two, Blend::Alpha);
            near(pixel(&quarter_alpha, 60, 45), [64, 0, 191, 255]);
            let additive = render(RED, 0.75, 0.5, RgbScale::Two, Blend::Additive);
            near(pixel(&additive, 60, 45), [128, 0, 255, 255]);
            // Scaling/clamping the vertex tint before texture multiplication
            // would yield128. The source material scales the product instead.
            let dark_texture = render(78, 0.75, 1., RgbScale::Two, Blend::Alpha);
            near(pixel(&dark_texture, 60, 45), [192, 0, 0, 255]);
        }
        assert!(validation.pop().await.is_none());
    });
}

#[test]
#[ignore = "requires a native graphics adapter; run explicitly on a local graphics host"]
fn projected_mesh_w_preserves_screen_coverage_and_perspective_correct_uv_and_color() {
    pollster::block_on(async {
        let (device, queue) = device().await;
        let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let triangle = |texture, weights: [f32; 3], colored| Mesh {
            texture,
            vertices: [[0., 0.], [640., 0.], [0., 480.]]
                .into_iter()
                .enumerate()
                .map(|(i, position)| Vertex {
                    position,
                    // Pixel centers of a two-texel blue-to-red ramp. Linear
                    // filtering then makes sampled red equal the second
                    // vertex's interpolated barycentric weight.
                    uv: [if i == 1 { 0.75 } else { 0.25 }, 0.5],
                    color: if colored {
                        [[0., 0., 1., 1.], [1., 0., 0., 1.], [0., 1., 0., 1.]][i]
                    } else {
                        [1.; 4]
                    },
                    clip_w: weights[i],
                })
                .collect(),
            blend: Blend::Alpha,
            rgb_scale: RgbScale::One,
        };
        let barycentric = |x, y, weights: [f32; 3]| {
            let bx = (f64::from(x) + 0.5) / 640.;
            let by = (f64::from(y) + 0.5) / 480.;
            let raw = [1. - bx - by, bx, by];
            let reciprocal: [f64; 3] = std::array::from_fn(|i| raw[i] / f64::from(weights[i]));
            let sum: f64 = reciprocal.iter().sum();
            reciprocal.map(|weight| weight / sum)
        };
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8Unorm,
        ] {
            let source = BTreeMap::from([
                (
                    29,
                    RgbaImage {
                        width: 2,
                        height: 1,
                        pixels: vec![0, 0, 255, 255, 255, 0, 0, 255],
                    },
                ),
                (
                    41,
                    RgbaImage {
                        width: 1,
                        height: 1,
                        pixels: vec![255; 4],
                    },
                ),
            ]);
            let gpu = Frontend::upload(&device, &queue, &source, format).unwrap();
            for colored in [false, true] {
                let texture = if colored { 41 } else { 29 };
                let perspective = layer_frame(
                    &device,
                    &queue,
                    &gpu,
                    format,
                    [640, 480],
                    &[triangle(texture, [1., 2., 4.], colored).into()],
                );
                let affine = layer_frame(
                    &device,
                    &queue,
                    &gpu,
                    format,
                    [640, 480],
                    &[triangle(texture, [1.; 3], colored).into()],
                );
                for (x, y) in [(159, 119), (319, 59)] {
                    let expected = |weights| {
                        let [a, b, c] = barycentric(x, y, weights);
                        let rgb = if colored { [b, c, a] } else { [b, 0., 1. - b] };
                        let [r, g, b] = rgb.map(|value| (value * 255.).round() as u8);
                        [r, g, b, 255]
                    };
                    near(pixel(&perspective, x, y), expected([1., 2., 4.]));
                    near(pixel(&affine, x, y), expected([1.; 3]));
                    assert!(pixel(&perspective, x, y)[0].abs_diff(pixel(&affine, x, y)[0]) > 10);
                }
                // Multiplying projected XY by W must preserve the supplied
                // triangle's screen silhouette after perspective division.
                assert_eq!(pixel(&perspective, 500, 400), BLACK);
                assert_eq!(pixel(&affine, 500, 400), BLACK);
            }
        }
        assert!(validation.pop().await.is_none());
    });
}
