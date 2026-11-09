//! Checked asset-to-animation binding for the source title visual tree.
use anyhow::{Context, Result, bail, ensure};
use ge_assets::{edb::Edb, frontend_script as asset, scene::Geometry};
use ge_core::{
    frontend_animation as core,
    frontend_projection::{Aspect, Camera},
    frontend_segments::{Phase, Segment, Timeline},
    frontend_sort as sort,
};
use ge_render::frontend::{Blend, Layer, Mesh, RgbScale, Vertex};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub const ROOT: u32 = 0x04001bb6;
const BUDGET: core::Budget = core::Budget {
    steps: 50_000,
    depth: 32,
};
type Colors = BTreeMap<usize, Vec<BTreeMap<u16, [u8; 4]>>>;
struct DrawSource {
    bounds: [[f32; 3]; 2],
    sort_field: i32,
}
pub struct Program {
    scripts: BTreeMap<u32, core::Script>,
    tracks: BTreeMap<(u32, u16), core::Tracks>,
    durations: BTreeMap<u32, f32>,
    geometry: BTreeMap<u32, Geometry>,
    colors: BTreeMap<u32, Colors>,
    draws: BTreeMap<u32, DrawSource>,
    intro: Option<Segment>,
    outro: Option<Segment>,
}
impl Program {
    pub fn load(edb: &Edb) -> Result<Arc<Self>> {
        let mut program = Self {
            scripts: BTreeMap::new(),
            tracks: BTreeMap::new(),
            durations: BTreeMap::new(),
            geometry: BTreeMap::new(),
            colors: BTreeMap::new(),
            draws: BTreeMap::new(),
            intro: None,
            outro: None,
        };
        program.read_script(edb, ROOT, 0)?;
        Ok(Arc::new(program))
    }
    fn read_script(&mut self, edb: &Edb, reference: u32, depth: usize) -> Result<()> {
        if self.scripts.contains_key(&reference) {
            return Ok(());
        }
        ensure!(
            depth < 32 && self.scripts.len() < 256,
            "title graph budget exhausted"
        );
        let view = edb
            .frontend_script(reference)?
            .context("title script reference absent")?;
        let header = view.header()?;
        let mut commands = Vec::new();
        let mut nested = Vec::new();
        for raw in view.commands(4096)? {
            if reference == ROOT && raw.opcode == 0x0b {
                let marker = u32::from_be_bytes(
                    raw.bytes_raw
                        .get(12..16)
                        .context("short title segment marker")?
                        .try_into()?,
                );
                if matches!(marker, 0x160001a6 | 0x160001a7) {
                    ensure!(
                        raw.start >= 0 && raw.duration > 0,
                        "invalid title UI segment"
                    );
                    let segment = Segment {
                        start: f32::from(raw.start),
                        end: f32::from(raw.start) + f32::from(raw.duration) - 1.,
                    };
                    if marker == 0x160001a6 {
                        ensure!(
                            self.intro.replace(segment).is_none(),
                            "ambiguous title intro segment"
                        );
                    } else {
                        ensure!(
                            self.outro.replace(segment).is_none(),
                            "ambiguous title outro segment"
                        );
                    }
                }
            }
            let kind = if let Some(instance) = raw.instance()? {
                ensure!(
                    instance.file_reference_raw == u32::MAX,
                    "external title script files not bound"
                );
                let tracks = decode_tracks(view, instance.track_selector)?;
                self.tracks
                    .insert((reference, instance.track_selector), tracks);
                let kind = match instance.kind {
                    asset::InstanceKind::Entity => core::InstanceKind::Entity,
                    asset::InstanceKind::Script => core::InstanceKind::Script,
                };
                if kind == core::InstanceKind::Script {
                    nested.push(instance.reference_raw);
                } else if !self.geometry.contains_key(&instance.reference_raw) {
                    let entity = edb
                        .entity_reference(instance.reference_raw)?
                        .context("title entity reference absent")?;
                    let geometry = edb.entity_geometry_at(entity.directory_index)?;
                    let spatial = edb.entity_spatial_at(entity.directory_index)?;
                    ensure!(
                        geometry
                            .batches
                            .iter()
                            .all(|batch| batch.entity_address == spatial.address),
                        "title draw bounds do not cover an inline child mesh"
                    );
                    self.draws.insert(
                        instance.reference_raw,
                        DrawSource {
                            bounds: spatial.bounds,
                            sort_field: spatial.sort_field_raw,
                        },
                    );
                    let mut frames = Colors::new();
                    for batch in &geometry.batches {
                        if frames.contains_key(&batch.entity_address) {
                            continue;
                        }
                        if let Some(view) = edb.frontend_color_frames(batch.entity_address)? {
                            ensure!(
                                view.frame_count() <= 256,
                                "title color-frame budget exhausted"
                            );
                            let end = batch.first_index as usize + batch.index_count as usize;
                            let indices = geometry
                                .indices
                                .get(batch.first_index as usize..end)
                                .context("title batch indices outside geometry")?;
                            let colors: BTreeSet<_> = indices
                                .iter()
                                .map(|&i| geometry.vertices[i as usize].color_index)
                                .collect();
                            let mut decoded = Vec::new();
                            for frame in 0..view.frame_count() {
                                let mut values = BTreeMap::new();
                                for &index in &colors {
                                    values.insert(index, view.color(frame, index)?);
                                }
                                decoded.push(values);
                            }
                            frames.insert(batch.entity_address, decoded);
                        }
                    }
                    self.colors.insert(instance.reference_raw, frames);
                    self.geometry.insert(instance.reference_raw, geometry);
                }
                core::CommandKind::Instance(core::Instance {
                    kind,
                    slot: instance.slot,
                    parent_slot: (instance.parent_slot_raw != 255)
                        .then_some(instance.parent_slot_raw),
                    selector: instance.track_selector,
                    reference: instance.reference_raw,
                    word_0c_raw: instance.word_0c_raw,
                    file_reference: instance.file_reference_raw,
                })
            } else {
                match raw.opcode {
                    0x11 => {
                        let count =
                            usize::from(*raw.bytes_raw.get(8).context("short title removal")?);
                        core::CommandKind::Remove(
                            raw.bytes_raw
                                .get(9..9 + count)
                                .context("short title removal slots")?
                                .to_vec(),
                        )
                    }
                    0x12 => core::CommandKind::Terminal,
                    0x10 if raw.bytes_raw.get(12) == Some(&1) => {
                        let variable =
                            *raw.bytes_raw.get(13).context("short title loop variable")?;
                        ensure!(
                            variable < header.variables,
                            "title loop variable outside script variables"
                        );
                        core::CommandKind::Loop {
                            variable,
                            count: i32::from_be_bytes(
                                raw.bytes_raw
                                    .get(16..20)
                                    .context("short title loop count")?
                                    .try_into()?,
                            ),
                            target_frame: u32::from_be_bytes(
                                raw.bytes_raw
                                    .get(20..24)
                                    .context("short title loop target")?
                                    .try_into()?,
                            ),
                        }
                    }
                    0x05 => {
                        let cue = CueRequest::decode(&raw.bytes_raw)?;
                        ensure!(reference == ROOT, "nested title sound actors are not bound");
                        ensure!(
                            view.track_bundle(cue.selector)?.channel_mask == 0,
                            "tracked title sound actors are not bound"
                        );
                        core::CommandKind::External {
                            opcode: raw.opcode,
                            payload: raw.bytes_raw,
                        }
                    }
                    0x0b => core::CommandKind::External {
                        opcode: raw.opcode,
                        payload: raw.bytes_raw,
                    },
                    opcode => core::CommandKind::External {
                        opcode,
                        payload: raw.bytes_raw,
                    },
                }
            };
            commands.push(core::Command {
                start: raw.start,
                duration: raw.duration as u16,
                kind,
            });
        }
        ensure!(header.frames_per_second > 0., "invalid title script rate");
        self.durations.insert(
            reference,
            header.duration_raw as f32 / header.frames_per_second,
        );
        self.scripts.insert(
            reference,
            core::Script {
                fps: header.frames_per_second,
                slots: header.slots,
                commands,
            },
        );
        for reference in nested {
            self.read_script(edb, reference, depth + 1)?;
        }
        Ok(())
    }
    pub fn textures(&self) -> BTreeSet<u32> {
        self.geometry
            .values()
            .flat_map(|g| {
                g.batches
                    .iter()
                    .map(|b| g.texture_bindings[b.binding_index])
            })
            .collect()
    }
}
fn keys<const N: usize>(values: Vec<asset::Key<N>>) -> Vec<core::Key<N>> {
    values
        .into_iter()
        .map(|k| core::Key {
            frame: k.time,
            value: k.value,
        })
        .collect()
}
fn decode_tracks(view: asset::Script<'_>, selector: u16) -> Result<core::Tracks> {
    let bundle = view.track_bundle(selector)?;
    ensure!(
        bundle.channel_mask & !(4 | 8 | 16 | 64 | 128 | 512) == 0,
        "unsupported title track mask {:#x}",
        bundle.channel_mask
    );
    let mut tracks = core::Tracks::default();
    for channel in [4, 8, 16, 64, 128, 512] {
        let Some(values) = bundle.linear_keys(channel, 4096)? else {
            continue;
        };
        match (channel, values) {
            (4, asset::LinearKeys::Vector(v)) => tracks.translation = Some(keys(v)),
            (8, asset::LinearKeys::Quaternion(v)) => tracks.rotation = Some(keys(v)),
            (16, asset::LinearKeys::Vector(v)) => tracks.scale = Some(keys(v)),
            (64, asset::LinearKeys::Vector(v)) => tracks.rgb = Some(keys(v)),
            (128, asset::LinearKeys::Scalar(v)) => tracks.alpha = Some(keys(v)),
            (512, asset::LinearKeys::Scalar(v)) => {
                let extension = bundle
                    .frame_selector_extension()?
                    .context("frame selector extension absent")?;
                let indices = bundle
                    .frame_selector_indices(127)?
                    .context("frame selector indices absent")?;
                tracks.frame_selector = Some(core::FrameSelector {
                    keys: keys(v),
                    indices,
                    playback_mode: extension.playback_mode_raw,
                });
            }
            _ => bail!("title track width does not match channel"),
        }
    }
    Ok(tracks)
}
struct Source(Arc<Program>);
#[derive(Debug, Clone, Serialize)]
pub struct CueRequest {
    pub sound_hash: u32,
    pub gain: f32,
    pub start_frame: i16,
    pub slot: u8,
    selector: u16,
}
impl CueRequest {
    fn decode(payload: &[u8]) -> Result<Self> {
        ensure!(
            payload.len() == 36 && payload[0] == 5,
            "unsupported title sound command size"
        );
        let word = |at| -> Result<u32> { Ok(u32::from_be_bytes(payload[at..at + 4].try_into()?)) };
        ensure!(
            payload[11] == 255
                && word(16)? == 0
                && payload[20] == 1
                && payload[22] == 0
                && payload[23] == 0
                && f32::from_bits(word(24)?) == 0.
                && f32::from_bits(word(28)?) == 0.,
            "unsupported title sound parent/flags/pitch/pan/range"
        );
        Ok(Self {
            sound_hash: word(32)?,
            gain: f32::from(payload[21]) * 0.01,
            start_frame: i16::from_be_bytes(payload[4..6].try_into()?),
            selector: u16::from_be_bytes(payload[8..10].try_into()?),
            slot: payload[10],
        })
    }
}
impl core::Source for Source {
    fn script(&mut self, reference: u32) -> Result<core::Script> {
        self.0
            .scripts
            .get(&reference)
            .cloned()
            .context("title script not loaded")
    }
    fn tracks(&mut self, script: u32, selector: u16) -> Result<core::Tracks> {
        self.0
            .tracks
            .get(&(script, selector))
            .cloned()
            .context("title tracks not loaded")
    }
    fn initial_pose(&mut self, _: u32, _: &core::Instance) -> Result<core::Pose> {
        Ok(core::Pose::default())
    }
}
#[derive(Serialize)]
pub struct Evidence {
    pub frame: f32,
    pub scripts: usize,
    pub entities: usize,
    pub color_frame_meshes: usize,
    pub draw_triangles: usize,
    pub external_commands: usize,
    pub terminal: bool,
    pub phase: Phase,
    pub prompt_alpha: f32,
    pub sorted_entities: usize,
    pub camera_aspect: Aspect,
    pub camera_fov_degrees: f32,
    pub cue_requests: usize,
    pub scope: &'static str,
}
pub struct Scene {
    source: Source,
    runtime: core::Runtime,
    seconds: f32,
    clocks: [f32; 16],
    external_commands: usize,
    pub evidence: Evidence,
    timeline: Timeline,
    initialized: bool,
    camera: Camera,
    cue_requests: Vec<CueRequest>,
    cue_count: usize,
}
impl Scene {
    pub fn new(program: Arc<Program>) -> Result<Self> {
        Self::with_aspect(program, Aspect::Wide)
    }
    pub fn with_aspect(program: Arc<Program>, aspect: Aspect) -> Result<Self> {
        let mut source = Source(program);
        let runtime = core::Runtime::new(ROOT, &mut source)?;
        let timeline = make_timeline(&source.0)?;
        let camera = Camera::new(aspect)?;
        Ok(Self {
            source,
            runtime,
            seconds: 0.,
            clocks: [0.; 16],
            external_commands: 0,
            evidence: Evidence {
                frame: 0.,
                scripts: 0,
                entities: 0,
                color_frame_meshes: 0,
                draw_triangles: 0,
                external_commands: 0,
                terminal: false,
                phase: Phase::Intro,
                prompt_alpha: 0.,
                sorted_entities: 0,
                camera_aspect: camera.aspect(),
                camera_fov_degrees: camera.fov_degrees(),
                cue_requests: 0,
                scope: "Title visual program not presented yet",
            },
            timeline,
            initialized: false,
            camera,
            cue_requests: Vec::new(),
            cue_count: 0,
        })
    }
    pub fn request_outro(&mut self, seconds: f64) -> Result<bool> {
        ensure!(
            seconds.is_finite() && seconds >= 0.,
            "invalid title confirmation time"
        );
        self.timeline.advance_to(seconds as f32)?;
        Ok(self.timeline.confirm())
    }
    pub fn complete(&self) -> bool {
        self.timeline.phase() == Phase::Complete
    }
    pub fn take_cues(&mut self) -> Vec<CueRequest> {
        std::mem::take(&mut self.cue_requests)
    }
    pub fn layers(&mut self, seconds: f64) -> Result<Vec<Layer>> {
        ensure!(
            seconds.is_finite() && seconds >= 0.,
            "invalid title animation time"
        );
        let seconds = seconds as f32;
        let reconstructing = seconds < self.seconds;
        if reconstructing {
            self.runtime = core::Runtime::new(ROOT, &mut self.source)?;
            self.seconds = 0.;
            self.clocks = [0.; 16];
            self.timeline = make_timeline(&self.source.0)?;
            self.initialized = false;
            self.cue_requests.clear();
        }
        self.timeline.advance_to(seconds)?;
        let root_frame = self.timeline.frame();
        let program = self.source.0.clone();
        let mut cues = Vec::new();
        let mut external = 0;
        let mut callback = |event: &core::Event| -> Result<core::Control> {
            if let core::EventKind::External { opcode, .. } = &event.kind {
                ensure!(
                    matches!(*opcode, 0x05 | 0x0b),
                    "unsupported reached title control opcode {opcode:#x}"
                );
                external += 1;
                if *opcode == 5 {
                    let core::EventKind::External { payload, .. } = &event.kind else {
                        unreachable!()
                    };
                    ensure!(
                        event.script == ROOT && event.path.is_empty(),
                        "nested title cue not bound"
                    );
                    let cue = CueRequest::decode(payload)?;
                    let removed = program.scripts[&ROOT].commands.iter().any(|command| {
                        f32::from(command.start) >= f32::from(cue.start_frame)
                            && f32::from(command.start) <= root_frame
                            && matches!(&command.kind, core::CommandKind::Remove(slots) if slots.contains(&cue.slot))
                    });
                    // Native actors start after command walking. A seek/rebuild
                    // suppresses creation; removal before evaluation suppresses
                    // a newly created voice. Owner detach never stops this cue.
                    if !reconstructing && !removed {
                        cues.push(cue);
                    }
                }
            }
            Ok(core::Control::Continue)
        };
        if !self.initialized || root_frame != self.runtime.frame() {
            self.runtime
                .advance_to(root_frame, &mut self.source, &mut callback, BUDGET)?;
            self.initialized = true;
        }
        // Native wrapper independently seeks named children, even after the root
        // terminal. Its wrap discards excess time rather than applying modulo.
        let instances = self.runtime.instances()?;
        for i in 0..16 {
            let tag = 0x1500006e + i as u32;
            if let Some(instance) = instances.iter().find(|s| {
                s.kind == core::InstanceKind::Script
                    && s.parent_key.is_none()
                    && s.word_0c_raw == tag
            }) {
                self.clocks[i] += seconds - self.seconds;
                if self.clocks[i] >= self.source.0.durations[&instance.reference] {
                    self.clocks[i] = 0.;
                }
                self.runtime.seek_tag_seconds(
                    tag,
                    self.clocks[i],
                    &mut self.source,
                    &mut callback,
                    BUDGET,
                )?;
            }
        }
        self.seconds = seconds;
        self.external_commands += external;
        self.cue_count += cues.len();
        self.cue_requests.extend(cues);
        let snapshots = self.runtime.instances()?;
        let mut groups = Vec::new();
        let mut records = Vec::new();
        let mut triangles = 0;
        let mut entities = 0;
        let mut morphs = 0;
        for instance in snapshots
            .iter()
            .filter(|s| s.kind == core::InstanceKind::Entity)
        {
            entities += 1;
            let geometry = &self.source.0.geometry[&instance.reference];
            let draw = &self.source.0.draws[&instance.reference];
            let matrix = &instance.world_matrix;
            // The native frontend installs root translation (0,-10,0) before
            // script drawing. Its queued camera looks from (0,18,0) down Y.
            let depth = if draw.sort_field == 0 {
                sort::Depth::Computed {
                    center: std::array::from_fn(|i| (draw.bounds[0][i] + draw.bounds[1][i]) * 0.5),
                    view_row: self.camera.depth_row(*matrix)?,
                }
            } else {
                sort::Depth::Explicit(sort::override_depth(draw.sort_field)?)
            };
            records.push(sort::Entity {
                payload: groups.len(),
                depth,
            });
            let mut layers = Vec::new();
            for batch in &geometry.batches {
                let colors = self.source.0.colors[&instance.reference].get(&batch.entity_address);
                let selected =
                    if let (Some(frames), Some(pair)) = (colors, instance.frame_selection) {
                        morphs += 1;
                        Some((
                            frames
                                .get(usize::try_from(pair.current)?)
                                .context("title current color frame absent")?,
                            frames
                                .get(usize::try_from(pair.next)?)
                                .context("title next color frame absent")?,
                            pair.blend_raw,
                        ))
                    } else {
                        None
                    };
                let mut vertices = Vec::new();
                for &index in &geometry.indices
                    [batch.first_index as usize..(batch.first_index + batch.index_count) as usize]
                {
                    let vertex = &geometry.vertices[index as usize];
                    let color = if let Some((a, b, blend)) = selected {
                        core::blend_color_frames(
                            *a.get(&vertex.color_index)
                                .context("title frame color index absent")?,
                            *b.get(&vertex.color_index)
                                .context("title frame color index absent")?,
                            blend,
                        )
                    } else {
                        vertex.color
                    };
                    let point =
                        ge_assets::transform::point(&instance.world_matrix, vertex.position)?;
                    let projected = self.camera.project(point)?;
                    vertices.push(Vertex {
                        position: projected.position,
                        clip_w: projected.clip_w,
                        uv: vertex.uv,
                        color: std::array::from_fn(|i| {
                            f32::from(color[i]) / 255. * instance.world_color[i]
                        }),
                    });
                }
                let blend = match batch.transparency >> 8 {
                    0 => Blend::Alpha,
                    1 => Blend::Additive,
                    mode => bail!("unsupported title source blend mode {mode}"),
                };
                triangles += vertices.len() / 3;
                layers.push(
                    Mesh {
                        blend,
                        rgb_scale: RgbScale::Two,
                        texture: geometry.texture_bindings[batch.binding_index],
                        vertices,
                    }
                    .into(),
                );
            }
            groups.push(layers);
        }
        let ordered = sort::order_entities(&records, 4096)?;
        let mut layers = Vec::new();
        for record in &ordered {
            layers.append(&mut groups[record.payload]);
        }
        self.evidence = Evidence {
            frame: self.runtime.frame(),
            scripts: self.source.0.scripts.len(),
            entities,
            color_frame_meshes: morphs,
            draw_triangles: triangles,
            external_commands: self.external_commands,
            terminal: self.runtime.terminal(),
            phase: self.timeline.phase(),
            prompt_alpha: snapshots
                .iter()
                .find(|s| s.word_0c_raw == 0x15000080)
                .map_or(0., |s| s.world_color[3]),
            sorted_entities: ordered.len(),
            camera_aspect: self.camera.aspect(),
            camera_fov_degrees: self.camera.fov_degrees(),
            cue_requests: self.cue_count,
            scope: "Source title geometry, positive animation tracks, color frames, perspective/root transform, type-two ordering and independent intro-sound requests. Host aspect/canvas/text, resampling and mixing remain adaptations. Other external controls, Wii numeric/filter/depth behavior and original display/input/synchronization parity remain unverified.",
        };
        Ok(layers)
    }
}

fn make_timeline(program: &Program) -> Result<Timeline> {
    Timeline::new(
        program
            .scripts
            .get(&ROOT)
            .context("title root unavailable")?
            .fps,
        program.intro.context("title intro marker unavailable")?,
        program.outro.context("title outro marker unavailable")?,
    )
}

#[cfg(test)]
#[path = "tests/frontend_title.rs"]
mod tests;
