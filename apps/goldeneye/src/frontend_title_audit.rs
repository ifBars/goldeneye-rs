//! Deterministic, metadata-only title runtime audit. No pixels, asset bytes or
//! original-runtime parity are emitted. Layer visibility is a geometric/color
//! diagnostic, not GPU occlusion, texture sampling or a reference comparison.
use crate::frontend_title::{Evidence, Program, ROOT, Scene};
use anyhow::{Context, Result, ensure};
use ge_assets::edb::Edb;
use ge_core::frontend_segments::{Phase, Segment};
use ge_render::frontend::{Blend, Layer, Mesh, RgbScale};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const HZ: u32 = 60;
const HOLD_SECONDS: u32 = 40;
const MAX_OUTRO_STEPS: u32 = 600;
const MORPH_COLOR_METRIC: &str = "per_mesh_rgb_patterns_no_rgb_tracks";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Signature {
    texture: u32,
    vertices: usize,
    additive: bool,
}
impl Signature {
    fn of(mesh: &Mesh) -> Self {
        Self {
            texture: mesh.texture,
            vertices: mesh.vertices.len(),
            additive: mesh.blend == Blend::Additive,
        }
    }
}

#[derive(Serialize)]
pub struct Window {
    pub start_seconds: u32,
    pub end_seconds: u32,
    pub samples: usize,
    pub hold_samples: usize,
    pub morph_samples: usize,
    pub layer_changes: usize,
    pub morph_color_changes: usize,
    pub distinct_morph_color_hashes: usize,
}

#[derive(Serialize)]
pub struct Report {
    pub resource_hash: u32,
    pub root_script: u32,
    pub sample_rate_hz: u32,
    pub scheduled_seconds: u32,
    pub samples: usize,
    pub hold_samples: usize,
    pub hold_morph_samples: usize,
    pub first_hold_seconds: f64,
    pub intro: Segment,
    pub outro: Segment,
    pub source_frames_per_second: f32,
    pub camera_aspect: ge_core::frontend_projection::Aspect,
    pub camera_fov_degrees: f32,
    pub minimum_hold_logo_triangles: usize,
    pub maximum_meshes: usize,
    pub maximum_draw_triangles: usize,
    pub maximum_color_frame_meshes: usize,
    pub hold_layer_changes: usize,
    pub hold_morph_color_changes: usize,
    pub distinct_hold_layer_hashes: usize,
    pub distinct_hold_morph_color_hashes: usize,
    /// Per-mesh RGB patterns, excluding alpha and mesh order/multiplicity.
    pub morph_color_metric: &'static str,
    pub external_commands_recorded: usize,
    pub windows: Vec<Window>,
    /// Ordered tick u32 LE, phase byte, frame f32 LE and ordered layer digest.
    pub scheduled_sequence_sha256: String,
    pub outro_sequence_sha256: String,
    pub confirmed_seconds: f64,
    pub outro_samples: usize,
    pub completed_seconds: f64,
    pub completed_frame: f32,
    pub deterministic_replay_samples: usize,
    pub deterministic_replay_verified: bool,
    pub reference_compared: bool,
    pub scope: &'static str,
}

struct Sample {
    layers: [u8; 32],
    colors: BTreeSet<[u8; 32]>,
    meshes: usize,
    triangles: usize,
    morph_meshes: usize,
    logo_triangles: usize,
}

#[derive(Default)]
struct Changes {
    previous: Option<[u8; 32]>,
    changes: usize,
    distinct: BTreeSet<[u8; 32]>,
}
impl Changes {
    fn include(&mut self, hash: [u8; 32]) {
        if self.previous.is_some_and(|previous| previous != hash) {
            self.changes += 1;
        }
        self.previous = Some(hash);
        self.distinct.insert(hash);
    }
}

/// A sample counts as a color change only when it introduces an unseen RGB
/// pattern after the first morph-present sample. Reordering, duplicating or
/// removing meshes cannot introduce a pattern. This metric is used only after
/// checking that the reachable title graph has no independent RGB tracks.
#[derive(Default)]
struct ColorChanges {
    initialized: bool,
    changes: usize,
    distinct: BTreeSet<[u8; 32]>,
}
impl ColorChanges {
    fn include(&mut self, patterns: &BTreeSet<[u8; 32]>) {
        if patterns.is_empty() {
            return;
        }
        if self.initialized && !patterns.is_subset(&self.distinct) {
            self.changes += 1;
        }
        self.distinct.extend(patterns);
        self.initialized = true;
    }
    fn animated(&self) -> bool {
        self.changes > 0 && self.distinct.len() >= 2
    }
}

#[derive(Default)]
struct WindowState {
    samples: usize,
    hold_samples: usize,
    morph_samples: usize,
    layers: Changes,
    colors: ColorChanges,
}

/// Run the actual asset-bound title program twice on identical deterministic
/// clocks. The caller owns disc identity/output paths and receipt integration.
pub fn audit(edb: &Edb) -> Result<Report> {
    ensure!(
        edb.header.hash == 0x01080004,
        "title audit requires fe_main resource"
    );
    let (fps, intro, outro) = segments(edb)?;
    require_no_rgb_tracks(edb)?;
    let logo = signatures(edb, 0x82000064)?;
    let morph = signatures(edb, 0x82000065)?;
    ensure!(!logo.is_empty(), "title audit primary logo geometry absent");
    ensure!(!morph.is_empty(), "title audit color-frame geometry absent");
    let program = Program::load(edb)?;
    let mut scene = Scene::new(program.clone())?;
    let mut replay = Scene::new(program)?;
    let mut windows: [WindowState; 4] = std::array::from_fn(|_| WindowState::default());
    let mut sequence = Sha256::new();
    let mut hold_layers = Changes::default();
    let mut hold_colors = ColorChanges::default();
    let mut hold_samples = 0;
    let mut hold_morph_samples = 0;
    let mut first_hold = None;
    let mut minimum_logo = usize::MAX;
    let mut max_meshes = 0;
    let mut max_triangles = 0;
    let mut max_morphs = 0;
    let mut previous_external = 0;
    let mut replay_samples = 0;
    let last_tick = HZ * HOLD_SECONDS;
    for tick in 0..=last_tick {
        let seconds = f64::from(tick) / f64::from(HZ);
        let sample = observe_pair(&mut scene, &mut replay, seconds, &logo, &morph)?;
        replay_samples += 1;
        let evidence = &scene.evidence;
        ensure!(
            matches!(evidence.phase, Phase::Intro | Phase::Hold),
            "title audit exited before confirmation"
        );
        ensure!(
            evidence.external_commands >= previous_external,
            "title audit external counter rewound"
        );
        previous_external = evidence.external_commands;
        max_meshes = max_meshes.max(sample.meshes);
        max_triangles = max_triangles.max(sample.triangles);
        max_morphs = max_morphs.max(evidence.color_frame_meshes);
        sequence_sample(&mut sequence, tick, evidence, sample.layers);
        let window = &mut windows[((tick / (HZ * 10)) as usize).min(3)];
        window.samples += 1;
        if evidence.phase == Phase::Hold {
            ensure!(
                evidence.frame == intro.end,
                "title audit hold escaped source endpoint"
            );
            ensure!(
                sample.logo_triangles > 0,
                "title audit logo disappeared during hold at {seconds}s"
            );
            ensure!(
                evidence.entities > 0 && evidence.draw_triangles > 0,
                "title audit held title has no entities/triangles"
            );
            first_hold.get_or_insert(seconds);
            minimum_logo = minimum_logo.min(sample.logo_triangles);
            hold_samples += 1;
            hold_layers.include(sample.layers);
            window.hold_samples += 1;
            window.layers.include(sample.layers);
            // Glint instances are deliberately absent between their timed
            // bursts. Collect independent RGB patterns only while a selected
            // morph exists. Alpha/pose changes and population changes cannot
            // masquerade as color-frame animation.
            if evidence.color_frame_meshes > 0 {
                ensure!(
                    sample.morph_meshes > 0,
                    "title audit selected color-frame geometry absent"
                );
                hold_morph_samples += 1;
                hold_colors.include(&sample.colors);
                window.morph_samples += 1;
                window.colors.include(&sample.colors);
            }
        }
    }
    let first_hold = first_hold.context("title audit never entered source intro hold")?;
    ensure!(
        max_morphs > 0,
        "title audit never selected a color-frame mesh"
    );
    ensure!(
        hold_layers.changes > 0,
        "title audit held title layers never animate"
    );
    ensure!(
        hold_colors.animated(),
        "title audit color-frame animation never changes"
    );
    for (index, window) in windows.iter().enumerate() {
        ensure!(
            window.hold_samples > 1 && window.colors.animated(),
            "title audit color-frame animation froze in window {index}"
        );
    }
    let confirmed_seconds = f64::from(HOLD_SECONDS);
    ensure!(
        scene.request_outro(confirmed_seconds)?,
        "title audit confirmation rejected after hold"
    );
    ensure!(
        replay.request_outro(confirmed_seconds)?,
        "title audit replay confirmation rejected"
    );
    let mut outro_sequence = Sha256::new();
    let mut completion = None;
    let mut outro_samples = 0;
    for tick in 0..=MAX_OUTRO_STEPS {
        let seconds = confirmed_seconds + f64::from(tick) / f64::from(HZ);
        let sample = observe_pair(&mut scene, &mut replay, seconds, &logo, &morph)?;
        replay_samples += 1;
        outro_samples += 1;
        ensure!(
            matches!(scene.evidence.phase, Phase::Outro | Phase::Complete),
            "title audit confirmation did not start outro"
        );
        ensure!(
            scene.evidence.frame >= outro.start && scene.evidence.frame <= outro.end,
            "title audit outro escaped source segment"
        );
        if tick == 0 {
            ensure!(
                scene.evidence.frame == outro.start,
                "title audit outro did not start at marker"
            );
        }
        sequence_sample(&mut outro_sequence, tick, &scene.evidence, sample.layers);
        ensure!(
            scene.complete() == (scene.evidence.phase == Phase::Complete),
            "title audit completion state differs"
        );
        if scene.complete() {
            ensure!(
                scene.evidence.frame == outro.end,
                "title audit completed at wrong frame"
            );
            completion = Some(seconds);
            break;
        }
    }
    let completed_seconds =
        completion.context("title audit outro failed to complete within work budget")?;
    Ok(Report {
        resource_hash: edb.header.hash,
        root_script: ROOT,
        sample_rate_hz: HZ,
        scheduled_seconds: HOLD_SECONDS,
        samples: last_tick as usize + 1,
        hold_samples,
        hold_morph_samples,
        first_hold_seconds: first_hold,
        intro,
        outro,
        source_frames_per_second: fps,
        camera_aspect: scene.evidence.camera_aspect,
        camera_fov_degrees: scene.evidence.camera_fov_degrees,
        minimum_hold_logo_triangles: minimum_logo,
        maximum_meshes: max_meshes,
        maximum_draw_triangles: max_triangles,
        maximum_color_frame_meshes: max_morphs,
        hold_layer_changes: hold_layers.changes,
        hold_morph_color_changes: hold_colors.changes,
        distinct_hold_layer_hashes: hold_layers.distinct.len(),
        distinct_hold_morph_color_hashes: hold_colors.distinct.len(),
        morph_color_metric: MORPH_COLOR_METRIC,
        external_commands_recorded: scene.evidence.external_commands,
        windows: windows
            .into_iter()
            .enumerate()
            .map(|(index, window)| Window {
                start_seconds: index as u32 * 10,
                end_seconds: (index as u32 + 1) * 10,
                samples: window.samples,
                hold_samples: window.hold_samples,
                morph_samples: window.morph_samples,
                layer_changes: window.layers.changes,
                morph_color_changes: window.colors.changes,
                distinct_morph_color_hashes: window.colors.distinct.len(),
            })
            .collect(),
        scheduled_sequence_sha256: format!("{:x}", sequence.finalize()),
        outro_sequence_sha256: format!("{:x}", outro_sequence.finalize()),
        confirmed_seconds,
        outro_samples,
        completed_seconds,
        completed_frame: scene.evidence.frame,
        deterministic_replay_samples: replay_samples,
        deterministic_replay_verified: true,
        reference_compared: false,
        scope: "Actual source title program at authored 60Hz, repeated named-child/script loops, indexed color-frame RGB patterns, persistent geometric logo and source intro/hold/outro boundaries. Color metrics exclude alpha, pose and mesh multiplicity/order, and the reachable source graph is checked to contain no independent RGB tracks. Two identical-clock executions agree. Metadata-only CPU layers; no GPU visibility, original rendering/input/audio synchronization or reference parity claim.",
    })
}

/// In this title subset every instance starts with white RGB. Without an RGB
/// track anywhere in its ancestry, a changing per-corner RGB pattern comes from
/// the selected color frame rather than a separate pose/color animation. Check
/// the complete reachable graph, not unrelated scripts stored in the resource.
fn require_no_rgb_tracks(edb: &Edb) -> Result<()> {
    let mut pending = vec![ROOT];
    let mut seen = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        if !seen.insert(reference) {
            continue;
        }
        ensure!(
            seen.len() <= 256,
            "title RGB-track audit graph budget exhausted"
        );
        let view = edb
            .frontend_script(reference)?
            .context("title RGB-track audit script absent")?;
        for command in view.commands(4096)? {
            let Some(instance) = command.instance()? else {
                continue;
            };
            ensure!(
                instance.file_reference_raw == u32::MAX,
                "title RGB-track audit external file is not bound"
            );
            ensure!(
                view.track_bundle(instance.track_selector)?.channel_mask & 0x40 == 0,
                "title RGB-pattern audit cannot distinguish independent RGB tracks"
            );
            if instance.kind == ge_assets::frontend_script::InstanceKind::Script {
                pending.push(instance.reference_raw);
            }
        }
    }
    Ok(())
}

fn segments(edb: &Edb) -> Result<(f32, Segment, Segment)> {
    let view = edb
        .frontend_script(ROOT)?
        .context("title audit root script absent")?;
    let fps = view.header()?.frames_per_second;
    let mut intro = None;
    let mut outro = None;
    for command in view.commands(4096)? {
        if command.opcode != 0x0b {
            continue;
        }
        let tag = u32::from_be_bytes(
            command
                .bytes_raw
                .get(12..16)
                .context("short title audit UI marker")?
                .try_into()?,
        );
        if !matches!(tag, 0x160001a6 | 0x160001a7) {
            continue;
        }
        ensure!(
            command.start >= 0 && command.duration > 0,
            "invalid title audit segment"
        );
        let segment = Segment {
            start: f32::from(command.start),
            end: f32::from(command.start) + f32::from(command.duration) - 1.,
        };
        let destination = if tag == 0x160001a6 {
            &mut intro
        } else {
            &mut outro
        };
        ensure!(
            destination.replace(segment).is_none(),
            "ambiguous title audit UI segment"
        );
    }
    Ok((
        fps,
        intro.context("title audit intro marker absent")?,
        outro.context("title audit outro marker absent")?,
    ))
}

fn signatures(edb: &Edb, reference: u32) -> Result<BTreeSet<Signature>> {
    let entity = edb
        .entity_reference(reference)?
        .context("title audit geometry reference absent")?;
    let geometry = edb.entity_geometry_at(entity.directory_index)?;
    geometry
        .batches
        .iter()
        .map(|batch| {
            let mode = batch.transparency >> 8;
            ensure!(mode <= 1, "unsupported title audit blend mode");
            Ok(Signature {
                texture: *geometry
                    .texture_bindings
                    .get(batch.binding_index)
                    .context("title audit texture binding absent")?,
                vertices: batch.index_count as usize,
                additive: mode == 1,
            })
        })
        .collect()
}

fn observe_pair(
    scene: &mut Scene,
    replay: &mut Scene,
    seconds: f64,
    logo: &BTreeSet<Signature>,
    morph: &BTreeSet<Signature>,
) -> Result<Sample> {
    let layers = scene.layers(seconds)?;
    let sample = inspect(&layers, logo, morph)?;
    validate_evidence(&scene.evidence, &sample)?;
    let repeated = inspect(&replay.layers(seconds)?, logo, morph)?;
    validate_evidence(&replay.evidence, &repeated)?;
    ensure!(
        sample.layers == repeated.layers && sample.colors == repeated.colors,
        "title audit deterministic layer replay differs at {seconds}s"
    );
    ensure!(
        serde_json::to_vec(&scene.evidence)? == serde_json::to_vec(&replay.evidence)?,
        "title audit deterministic state replay differs at {seconds}s"
    );
    Ok(sample)
}

fn validate_evidence(evidence: &Evidence, sample: &Sample) -> Result<()> {
    ensure!(
        evidence.frame.is_finite() && evidence.prompt_alpha.is_finite(),
        "nonfinite title audit evidence"
    );
    ensure!(evidence.scripts > 0, "title audit loaded no scripts");
    ensure!(
        evidence.sorted_entities == evidence.entities,
        "title audit bypassed entity sort"
    );
    ensure!(
        evidence.draw_triangles == sample.triangles,
        "title audit triangle accounting differs"
    );
    ensure!(
        evidence.color_frame_meshes <= sample.meshes,
        "title audit color-frame mesh accounting differs"
    );
    Ok(())
}

fn inspect(
    layers: &[Layer],
    logo: &BTreeSet<Signature>,
    morph: &BTreeSet<Signature>,
) -> Result<Sample> {
    let mut hash = Sha256::new();
    let mut sample = Sample {
        layers: [0; 32],
        colors: BTreeSet::new(),
        meshes: 0,
        triangles: 0,
        morph_meshes: 0,
        logo_triangles: 0,
    };
    for layer in layers {
        let Layer::Mesh(mesh) = layer else {
            anyhow::bail!("title audit source program emitted a non-mesh layer");
        };
        ensure!(
            mesh.vertices.len().is_multiple_of(3),
            "title audit mesh is not a triangle list"
        );
        let signature = Signature::of(mesh);
        hash.update(mesh.texture.to_le_bytes());
        hash.update([u8::from(signature.additive)]);
        hash.update([match mesh.rgb_scale {
            RgbScale::One => 1,
            RgbScale::Two => 2,
        }]);
        hash.update((mesh.vertices.len() as u64).to_le_bytes());
        let is_morph = morph.contains(&signature);
        let mut colors = Sha256::new();
        if is_morph {
            colors.update(mesh.texture.to_le_bytes());
            colors.update((mesh.vertices.len() as u64).to_le_bytes());
            sample.morph_meshes += 1;
        }
        for vertex in &mesh.vertices {
            ensure!(
                vertex
                    .position
                    .iter()
                    .chain(&vertex.uv)
                    .chain(&vertex.color)
                    .chain(std::iter::once(&vertex.clip_w))
                    .all(|v| v.is_finite()),
                "nonfinite title audit vertex"
            );
            for value in vertex
                .position
                .iter()
                .chain(&vertex.uv)
                .chain(&vertex.color)
                .chain(std::iter::once(&vertex.clip_w))
            {
                hash.update(value.to_bits().to_le_bytes());
            }
            if is_morph {
                for value in &vertex.color[..3] {
                    colors.update(value.to_bits().to_le_bytes());
                }
            }
        }
        if is_morph {
            sample.colors.insert(colors.finalize().into());
        }
        sample.meshes += 1;
        sample.triangles += mesh.vertices.len() / 3;
        if logo.contains(&signature) {
            sample.logo_triangles += mesh
                .vertices
                .chunks_exact(3)
                .filter(|triangle| visible_triangle(triangle))
                .count();
        }
    }
    sample.layers = hash.finalize().into();
    Ok(sample)
}

fn visible_triangle(triangle: &[ge_render::frontend::Vertex]) -> bool {
    let [a, b, c] = [
        triangle[0].position,
        triangle[1].position,
        triangle[2].position,
    ];
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    area != 0.
        && triangle
            .iter()
            .any(|v| v.color[3] > 0. && v.color[..3].iter().any(|&c| c > 0.))
        && triangle
            .iter()
            .map(|v| v.position[0])
            .fold(f32::INFINITY, f32::min)
            < 640.
        && triangle
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max)
            > 0.
        && triangle
            .iter()
            .map(|v| v.position[1])
            .fold(f32::INFINITY, f32::min)
            < 480.
        && triangle
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max)
            > 0.
}

fn sequence_sample(hash: &mut Sha256, tick: u32, evidence: &Evidence, layers: [u8; 32]) {
    hash.update(tick.to_le_bytes());
    hash.update([match evidence.phase {
        Phase::Intro => 0,
        Phase::Hold => 1,
        Phase::Outro => 2,
        Phase::Complete => 3,
    }]);
    hash.update(evidence.frame.to_bits().to_le_bytes());
    hash.update(layers);
}

#[cfg(test)]
#[path = "tests/frontend_title_audit.rs"]
mod tests;
