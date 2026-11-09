//! Source-shaped frontend animation: forward command scheduling and positive
//! linear/quaternion tracks. Asset addressing and external command effects stay
//! with the caller. Cubic curves, script jumps and original FP estimate accuracy
//! are not implemented by this deliberately bounded evaluator.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key<const N: usize> {
    pub frame: f32,
    pub value: [f32; N],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Pose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 4],
    pub color: [f32; 4],
    pub color_flags_raw: u16,
}

impl Default for Pose {
    /// Native base-component constructor defaults, not an asset binding fallback.
    fn default() -> Self {
        Self {
            translation: [0.; 3],
            rotation: [0.; 4],
            scale: [1.; 4],
            color: [1.; 4],
            color_flags_raw: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tracks {
    /// Bundle mask0x04 writes native translation, not scale.
    pub translation: Option<Vec<Key<3>>>,
    pub rotation: Option<Vec<Key<4>>>,
    /// Bundle mask0x10 writes scale XYZ and zero to its fourth lane.
    pub scale: Option<Vec<Key<3>>>,
    pub rgb: Option<Vec<Key<3>>>,
    pub alpha: Option<Vec<Key<1>>>,
    pub frame_selector: Option<FrameSelector>,
}

#[derive(Debug, Clone)]
pub struct FrameSelector {
    pub keys: Vec<Key<1>>,
    pub indices: Vec<i16>,
    /// Source extension byte3:0 clamp,1 repeat,2 ping-pong.
    pub playback_mode: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FrameSelection {
    pub current: i16,
    pub next: i16,
    pub blend_raw: u8,
}

/// Component-five color-frame blend. Native weights are byte/256 and
/// 1-byte/256; each contribution is truncated separately before addition.
/// The source builds these tables by repeated f32 accumulation. Every increment
/// and sum in its 256 entries is exactly representable because the weights are
/// dyadic and inputs are bytes, so this integer expression has the same result.
pub fn blend_color_frames(first: [u8; 4], second: [u8; 4], blend: u8) -> [u8; 4] {
    let blend = u32::from(blend);
    std::array::from_fn(|i| {
        ((((256 - blend) * u32::from(first[i])) >> 8) + ((blend * u32::from(second[i])) >> 8)) as u8
    })
}

fn select_frame(selector: &FrameSelector, value: f32) -> Result<FrameSelection> {
    ensure!(
        !selector.indices.is_empty() && selector.indices.len() <= 127,
        "unsupported frontend frame selector count"
    );
    ensure!(
        selector.playback_mode <= 2,
        "unsupported frontend frame selector playback mode"
    );
    let last = selector.indices.len() - 1;
    let range = last as f32;
    let frame = if selector.playback_mode == 0 {
        value.min(range)
    } else if range <= 0. {
        0.
    } else {
        let quotient = (value / range).trunc();
        ensure!(
            quotient >= i32::MIN as f32 && quotient < -(i32::MIN as f32),
            "frontend frame selector quotient overflow"
        );
        let remainder = value - range * quotient;
        if selector.playback_mode == 2 && quotient as i32 & 1 != 0 {
            range - remainder
        } else {
            remainder
        }
    };
    ensure!(
        frame >= 0. && frame.is_finite(),
        "frontend frame selector index outside mapping"
    );
    let index = frame.trunc() as usize;
    let current = *selector
        .indices
        .get(index)
        .context("frontend frame selector index outside mapping")?;
    let next = selector.indices[(index + 1).min(last)];
    let blend = (256. * (frame - index as f32)).trunc();
    finite(&[blend])?;
    Ok(FrameSelection {
        current,
        next,
        blend_raw: (blend as i32) as u8,
    })
}

fn finite(values: &[f32]) -> Result<()> {
    ensure!(
        values.iter().all(|x| x.is_finite()),
        "nonfinite frontend animation input or arithmetic"
    );
    Ok(())
}

fn window<const N: usize>(
    keys: &[Key<N>],
    frame: f32,
    cursor: &mut usize,
) -> Result<(Key<N>, Key<N>, f32, bool)> {
    finite(&[frame])?;
    ensure!(!keys.is_empty(), "frontend animation track has no keys");
    ensure!(
        *cursor < keys.len(),
        "frontend animation cursor is out of range"
    );
    while *cursor + 1 < keys.len() && keys[*cursor + 1].frame <= frame {
        *cursor += 1;
    }
    let current = keys[*cursor];
    let final_key = *cursor + 1 == keys.len();
    let next = if final_key {
        current
    } else {
        keys[*cursor + 1]
    };
    finite(&current.value)?;
    finite(&next.value)?;
    finite(&[current.frame, next.frame])?;
    let remaining = if final_key {
        1.
    } else {
        (next.frame - frame) / (next.frame - current.frame)
    };
    finite(&[remaining])?;
    Ok((current, next, remaining, final_key))
}

/// Cached forward sampling. Before the first key, native interpolation
/// extrapolates; the final key is held. Callers must reset the cursor on rewind.
pub fn sample_vector(keys: &[Key<3>], frame: f32, cursor: &mut usize) -> Result<([f32; 3], bool)> {
    let (a, b, remaining, done) = window(keys, frame, cursor)?;
    let value = std::array::from_fn(|i| remaining.mul_add(a.value[i] - b.value[i], b.value[i]));
    finite(&value)?;
    Ok((value, done))
}

pub fn sample_scalar(keys: &[Key<1>], frame: f32, cursor: &mut usize) -> Result<(f32, bool)> {
    let (a, b, remaining, done) = window(keys, frame, cursor)?;
    // The scalar source uses separate subtract/multiply/add, unlike vec3 FMA.
    let product = (a.value[0] - b.value[0]) * remaining;
    let value = b.value[0] + product;
    finite(&[value])?;
    Ok((value, done))
}

fn reciprocal_sqrt(value: f32) -> f32 {
    // Host seed; the Wii rsqrte estimate and rounding remain unverified.
    let seed = 1. / value.sqrt();
    let square = seed * seed;
    let half = seed * 0.5;
    (-(value.mul_add(square, -1.))).mul_add(half, seed)
}

/// Native shortest-path polynomial SLERP, without host trigonometric functions.
/// It retains non-unit inputs and does not normalize the final quaternion.
pub fn quaternion_slerp(a: [f32; 4], b: [f32; 4], amount: f32) -> Result<[f32; 4]> {
    finite(&a)?;
    finite(&b)?;
    finite(&[amount])?;
    let x = a[0] * b[0];
    let y = a[1] * b[1];
    let dot = a[2].mul_add(b[2], x) + a[3].mul_add(b[3], y);
    let c = dot.abs();
    let mut weights = [1. - amount, amount];
    if 1. - c > f32::from_bits(0x33d6_bf95) {
        let square_gap = (-c).mul_add(c, 1.);
        let gap = 1. - c;
        let inverse_sine = reciprocal_sqrt(square_gap);
        let root_gap = gap * reciprocal_sqrt(gap);
        let coefficients = [
            0xbaa5_7a2c,
            0x3bda_90c5,
            0xbc8b_fc66,
            0x3cfd_10f8,
            0xbd4d_8392,
            0x3db6_3a9e,
        ];
        let mut polynomial =
            f32::from_bits(coefficients[0]).mul_add(c, f32::from_bits(coefficients[1]));
        for coefficient in &coefficients[2..] {
            polynomial = polynomial.mul_add(c, f32::from_bits(*coefficient));
        }
        polynomial = polynomial.mul_add(c * c, f32::from_bits(0x3fc9_0fda));
        polynomial = f32::from_bits(0xbe5b_bfca).mul_add(c, polynomial);
        let angle = polynomial * root_gap;
        for weight in &mut weights {
            let angle = *weight * angle;
            let square = angle * angle;
            let fourth = square * square;
            let mut sine = f32::from_bits(0xb2cd_4ca8).mul_add(square, f32::from_bits(0x3638_b94f));
            sine = sine.mul_add(square, f32::from_bits(0xb950_0c03));
            sine = sine.mul_add(square, f32::from_bits(0x3c08_8887));
            sine = sine.mul_add(fourth, 1.);
            sine = f32::from_bits(0xbe2a_aaab).mul_add(square, sine);
            *weight = (angle * sine) * inverse_sine;
        }
    }
    if dot < 0. {
        weights[0] = -weights[0];
    }
    let value = std::array::from_fn(|i| a[i].mul_add(weights[0], b[i] * weights[1]));
    finite(&value)?;
    Ok(value)
}

pub fn sample_quaternion(
    keys: &[Key<4>],
    frame: f32,
    cursor: &mut usize,
) -> Result<([f32; 4], bool)> {
    let (current, next, remaining, done) = window(keys, frame, cursor)?;
    Ok((
        quaternion_slerp(next.value, current.value, remaining)?,
        done,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InstanceKind {
    Entity,
    Script,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub kind: InstanceKind,
    pub slot: u8,
    pub parent_slot: Option<u8>,
    pub selector: u16,
    pub reference: u32,
    /// Native command+c; not used as an initial transform by common creation.
    pub word_0c_raw: u32,
    pub file_reference: u32,
}

#[derive(Debug, Clone)]
pub struct Command {
    pub start: i16,
    /// Preserved metadata; forward execution does not expire at this duration.
    pub duration: u16,
    pub kind: CommandKind,
}

#[derive(Debug, Clone)]
pub enum CommandKind {
    Instance(Instance),
    Remove(Vec<u8>),
    /// The caller must explicitly handle nonvisual commands and their effects.
    External {
        opcode: u8,
        payload: Vec<u8>,
    },
    /// Native opcode0x10 mode1. The asset adapter validates `variable` against
    /// its header's variable count. Only empty-live restart at frame0 is
    /// implemented; other rebuild-seek shapes produce explicit errors.
    Loop {
        variable: u8,
        count: i32,
        target_frame: u32,
    },
    Terminal,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub fps: f32,
    pub slots: u8,
    pub commands: Vec<Command>,
}

pub trait Source {
    fn script(&mut self, reference: u32) -> Result<Script>;
    fn tracks(&mut self, script: u32, selector: u16) -> Result<Tracks>;
    /// Bind actual creation defaults/transform data explicitly, including absent
    /// transform-reference semantics. No missing resource becomes identity here.
    fn initial_pose(&mut self, script: u32, instance: &Instance) -> Result<Pose>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    Create(Instance),
    Remove {
        slot: u8,
        reference: u32,
    },
    External {
        opcode: u8,
        payload: Vec<u8>,
    },
    Loop {
        variable: u8,
        counter: i32,
        target_frame: u32,
        taken: bool,
    },
    Terminal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub script: u32,
    /// Nested script slot path; empty for the root script.
    pub path: Vec<u8>,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub steps: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceSnapshot {
    pub key: Vec<u8>,
    pub parent_key: Option<Vec<u8>>,
    pub source_script: u32,
    pub reference: u32,
    pub word_0c_raw: u32,
    pub kind: InstanceKind,
    pub pose: Pose,
    pub world_matrix: [f32; 16],
    pub world_color: [f32; 4],
    pub frame_selection: Option<FrameSelection>,
}

#[derive(Debug, Clone, Default)]
struct Cursors {
    translation: usize,
    rotation: usize,
    scale: usize,
    rgb: usize,
    alpha: usize,
    frame_selector: usize,
    finished: u32,
}

#[derive(Debug, Clone)]
struct Live {
    instance: Instance,
    start_offset: f32,
    local_frame: f32,
    pose: Pose,
    tracks: Option<Tracks>,
    cursors: Cursors,
    child: Option<Box<Runtime>>,
    serial: u64,
    frame_selection: Option<FrameSelection>,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    reference: u32,
    script: Script,
    frame: f32,
    next_command: usize,
    terminal: bool,
    slots: Vec<Option<Live>>,
    next_serial: u64,
    /// Native signed variable words, initially0. A u8 key bounds this sparse
    /// representation to at most256 counters; source header bounds are checked
    /// by the asset adapter before constructing a typed Loop command.
    loop_counters: BTreeMap<u8, i32>,
}

impl Runtime {
    pub fn new(reference: u32, source: &mut impl Source) -> Result<Self> {
        let script = source.script(reference)?;
        ensure!(
            script.fps.is_finite() && script.fps > 0.,
            "invalid frontend script frame rate"
        );
        Ok(Self {
            reference,
            frame: 0.,
            next_command: 0,
            terminal: false,
            slots: vec![None; usize::from(script.slots)],
            next_serial: 0,
            loop_counters: BTreeMap::new(),
            script,
        })
    }

    pub fn frame(&self) -> f32 {
        self.frame
    }
    pub fn terminal(&self) -> bool {
        self.terminal
    }

    /// Native UI tag lookup scans this script's live slots in slot order. This
    /// targets the matching child script independently of the root command clock,
    /// including after its terminal command. Backward child time reconstructs
    /// its command state; the caller owns source UI clock accumulation/wrapping.
    pub fn seek_tag_seconds(
        &mut self,
        tag: u32,
        seconds: f32,
        source: &mut impl Source,
        callback: &mut impl FnMut(&Event) -> Result<Control>,
        budget: Budget,
    ) -> Result<bool> {
        finite(&[seconds])?;
        ensure!(seconds >= 0., "negative frontend tagged script time");
        let Some(live) = self
            .slots
            .iter_mut()
            .flatten()
            .find(|live| live.instance.word_0c_raw == tag)
        else {
            return Ok(false);
        };
        let child = live
            .child
            .as_mut()
            .context("frontend tag does not identify a script")?;
        let frame = seconds * child.script.fps;
        finite(&[frame])?;
        if frame < child.frame {
            **child = Self::new(live.instance.reference, source)?;
        }
        let mut steps = budget.steps;
        child.advance(
            frame,
            source,
            callback,
            &mut steps,
            budget.depth,
            &[live.instance.slot],
        )?;
        Ok(true)
    }

    pub fn advance_to(
        &mut self,
        frame: f32,
        source: &mut impl Source,
        callback: &mut impl FnMut(&Event) -> Result<Control>,
        budget: Budget,
    ) -> Result<()> {
        let mut steps = budget.steps;
        self.advance(frame, source, callback, &mut steps, budget.depth, &[])
    }

    fn emit(
        &self,
        path: &[u8],
        kind: EventKind,
        callback: &mut impl FnMut(&Event) -> Result<Control>,
    ) -> Result<Control> {
        callback(&Event {
            script: self.reference,
            path: path.to_vec(),
            kind,
        })
    }

    fn remove(
        &mut self,
        slot: u8,
        path: &[u8],
        callback: &mut impl FnMut(&Event) -> Result<Control>,
    ) -> Result<()> {
        // Common removal destroys attached ordinary children recursively first.
        // Clear the root slot before recursion so malformed parent cycles cannot
        // recurse indefinitely; the snapshot traversal also rejects cycles.
        let live = self
            .slots
            .get_mut(usize::from(slot))
            .context("frontend removal slot out of range")?
            .take();
        if let Some(live) = live {
            self.emit(
                path,
                EventKind::Remove {
                    slot,
                    reference: live.instance.reference,
                },
                callback,
            )?;
            let mut children: Vec<_> = self
                .slots
                .iter()
                .flatten()
                .filter(|child| child.instance.parent_slot == Some(slot))
                .map(|child| (child.serial, child.instance.slot))
                .collect();
            children.sort_unstable();
            for (_, child) in children {
                self.remove(child, path, callback)?;
            }
        }
        Ok(())
    }

    fn advance(
        &mut self,
        mut frame: f32,
        source: &mut impl Source,
        callback: &mut impl FnMut(&Event) -> Result<Control>,
        steps: &mut usize,
        depth: usize,
        path: &[u8],
    ) -> Result<()> {
        finite(&[frame])?;
        ensure!(
            frame >= self.frame,
            "frontend animation rewind requires a fresh runtime"
        );
        ensure!(depth != 0, "frontend animation depth budget exhausted");
        self.frame = frame;
        if self.terminal {
            return Ok(());
        }
        while let Some(command) = self.script.commands.get(self.next_command).cloned() {
            if f32::from(command.start) > frame.trunc() {
                break;
            }
            consume(steps)?;
            match command.kind {
                CommandKind::Instance(instance) => {
                    ensure!(
                        usize::from(instance.slot) < self.slots.len(),
                        "frontend instance slot out of range"
                    );
                    if let Some(parent) = instance.parent_slot {
                        ensure!(
                            usize::from(parent) < self.slots.len() && parent != instance.slot,
                            "frontend instance parent slot invalid"
                        );
                    }
                    self.remove(instance.slot, path, callback)?;
                    let parent = instance
                        .parent_slot
                        .and_then(|slot| self.slots.get(usize::from(slot)))
                        .and_then(Option::as_ref);
                    let parent_slot = parent.map(|_| instance.parent_slot.expect("parent exists"));
                    let offset =
                        f32::from(command.start) - parent.map_or(0., |parent| parent.start_offset);
                    let mut instance = instance;
                    instance.parent_slot = parent_slot;
                    let pose = source.initial_pose(self.reference, &instance)?;
                    let child = if instance.kind == InstanceKind::Script {
                        Some(Box::new(Self::new(instance.reference, source)?))
                    } else {
                        None
                    };
                    self.emit(path, EventKind::Create(instance.clone()), callback)?;
                    let slot = usize::from(instance.slot);
                    self.slots[slot] = Some(Live {
                        instance,
                        start_offset: offset,
                        local_frame: 0.,
                        pose,
                        tracks: None,
                        cursors: Cursors::default(),
                        child,
                        serial: self.next_serial,
                        frame_selection: None,
                    });
                    self.next_serial = self
                        .next_serial
                        .checked_add(1)
                        .context("frontend instance serial overflow")?;
                }
                CommandKind::Remove(slots) => {
                    for slot in slots {
                        consume(steps)?;
                        self.remove(slot, path, callback)?;
                    }
                }
                CommandKind::External { opcode, payload } => {
                    if self.emit(path, EventKind::External { opcode, payload }, callback)?
                        == Control::Stop
                    {
                        // Native external-stop leaves this command current but
                        // still evaluates the existing active slots this tick.
                        break;
                    }
                }
                CommandKind::Loop {
                    variable,
                    count,
                    target_frame,
                } => {
                    // 8031ceac: positive current values decrement first and
                    // continue when they become0. Nonpositive values reload
                    // the stored count, then jump even if that count is0.
                    let counter = self.loop_counters.entry(variable).or_default();
                    let taken = if *counter > 0 {
                        *counter -= 1;
                        *counter != 0
                    } else {
                        *counter = count;
                        true
                    };
                    let counter = *counter;
                    self.emit(
                        path,
                        EventKind::Loop {
                            variable,
                            counter,
                            target_frame,
                            taken,
                        },
                        callback,
                    )?;
                    if taken {
                        ensure!(
                            target_frame == 0,
                            "unsupported frontend loop rebuild target"
                        );
                        // The title glint loop has removed all instances before
                        // reaching this command. Native8031c350(0) reconstructs
                        // the selected frame0 creations and resets their curve
                        // cursors. Counters persist and overshoot is discarded.
                        // The native rebuild helper returns immediately when
                        // currentframe already equals target. In that case the
                        // current command remains current, even with live slots.
                        if frame != 0. {
                            ensure!(
                                self.slots.iter().all(Option::is_none),
                                "unsupported frontend loop rebuild with live instances"
                            );
                            self.next_command = 0;
                            self.frame = 0.;
                            frame = 0.;
                        }
                        continue;
                    }
                }
                CommandKind::Terminal => {
                    self.terminal = true;
                    self.emit(path, EventKind::Terminal, callback)?;
                    return Ok(());
                }
            }
            self.next_command += 1;
        }
        // Native active-track evaluation is slot order, distinct from attachment
        // order used for rendering. Parent local-frame reads observe this order.
        for slot in 0..self.slots.len() {
            if self.slots[slot].is_none() {
                continue;
            }
            consume(steps)?;
            let parent_slot = self.slots[slot]
                .as_ref()
                .and_then(|live| live.instance.parent_slot);
            let input = if let Some(parent) = parent_slot {
                self.slots
                    .get(usize::from(parent))
                    .and_then(Option::as_ref)
                    .context("frontend parent was removed")?
                    .local_frame
            } else {
                frame
            };
            let live = self.slots[slot].as_mut().expect("live slot");
            live.local_frame = input - live.start_offset;
            if live.tracks.is_none() {
                live.tracks = Some(source.tracks(self.reference, live.instance.selector)?);
            }
            apply_tracks(
                live.tracks.as_ref().expect("loaded tracks"),
                live.local_frame,
                &mut live.cursors,
                &mut live.pose,
                &mut live.frame_selection,
            )?;
            if let Some(child) = &mut live.child {
                let seconds = live.local_frame / self.script.fps;
                let child_frame = seconds * child.script.fps;
                // A UI-tagged clock may have advanced this child independently
                // while its parent was held. Native virtual+2c reconstructs the
                // child when the resumed parent requests an earlier time.
                // As with seek_tag_seconds, this scoped reconstruction replays
                // child callbacks; general native object-reuse parity is open.
                if child_frame < child.frame {
                    **child = Self::new(live.instance.reference, source)?;
                }
                let mut child_path = path.to_vec();
                child_path.push(slot as u8);
                child.advance(child_frame, source, callback, steps, depth - 1, &child_path)?;
            }
        }
        Ok(())
    }

    pub fn instances(&self) -> Result<Vec<InstanceSnapshot>> {
        let mut output = Vec::new();
        self.snapshots(&[], &Pose::default().matrix()?, [1.; 4], 0, &mut output)?;
        Ok(output)
    }

    fn snapshots(
        &self,
        path: &[u8],
        parent: &[f32; 16],
        parent_color: [f32; 4],
        parent_color_flags: u16,
        output: &mut Vec<InstanceSnapshot>,
    ) -> Result<()> {
        self.snapshot_children(
            None,
            path,
            parent,
            parent_color,
            parent_color_flags,
            output,
            self.slots.len(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn snapshot_children(
        &self,
        parent_slot: Option<u8>,
        path: &[u8],
        parent: &[f32; 16],
        parent_color: [f32; 4],
        parent_color_flags: u16,
        output: &mut Vec<InstanceSnapshot>,
        remaining: usize,
    ) -> Result<()> {
        let mut live: Vec<_> = self
            .slots
            .iter()
            .flatten()
            .filter(|live| live.instance.parent_slot == parent_slot)
            .collect();
        live.sort_by_key(|live| live.serial);
        for instance in live {
            ensure!(remaining != 0, "frontend snapshot parent cycle");
            let world = self.world_for(instance.instance.slot, parent, self.slots.len())?;
            let (world_color, world_color_flags) = self.color_for(
                instance.instance.slot,
                parent_color,
                parent_color_flags,
                self.slots.len(),
            )?;
            let mut key = path.to_vec();
            key.push(instance.instance.slot);
            let parent_key = if let Some(slot) = instance.instance.parent_slot {
                let mut p = path.to_vec();
                p.push(slot);
                Some(p)
            } else if path.is_empty() {
                None
            } else {
                Some(path.to_vec())
            };
            output.push(InstanceSnapshot {
                key: key.clone(),
                parent_key,
                source_script: self.reference,
                reference: instance.instance.reference,
                word_0c_raw: instance.instance.word_0c_raw,
                kind: instance.instance.kind,
                pose: instance.pose,
                world_matrix: world,
                world_color,
                frame_selection: instance.frame_selection,
            });
            if let Some(child) = &instance.child {
                child.snapshots(&key, &world, world_color, world_color_flags, output)?;
            }
            self.snapshot_children(
                Some(instance.instance.slot),
                path,
                parent,
                parent_color,
                parent_color_flags,
                output,
                remaining - 1,
            )?;
        }
        Ok(())
    }

    fn world_for(&self, slot: u8, parent: &[f32; 16], remaining: usize) -> Result<[f32; 16]> {
        ensure!(remaining != 0, "frontend pose parent cycle");
        let live = self
            .slots
            .get(usize::from(slot))
            .and_then(Option::as_ref)
            .context("frontend pose parent missing")?;
        let parent = if let Some(parent_slot) = live.instance.parent_slot {
            self.world_for(parent_slot, parent, remaining - 1)?
        } else {
            *parent
        };
        multiply(&parent, &live.pose.matrix()?)
    }

    fn color_for(
        &self,
        slot: u8,
        parent: [f32; 4],
        flags: u16,
        remaining: usize,
    ) -> Result<([f32; 4], u16)> {
        ensure!(remaining != 0, "frontend color parent cycle");
        let live = self
            .slots
            .get(usize::from(slot))
            .and_then(Option::as_ref)
            .context("frontend color parent missing")?;
        let (parent, flags) = if let Some(slot) = live.instance.parent_slot {
            self.color_for(slot, parent, flags, remaining - 1)?
        } else {
            (parent, flags)
        };
        let own = live.pose.color_flags_raw;
        let color = if flags & 6 == 0 || own & 8 != 0 {
            live.pose.color
        } else if own & 6 != 0 {
            std::array::from_fn(|i| parent[i] * live.pose.color[i])
        } else {
            parent
        };
        let flags = if flags & 6 != 0 && own & 8 != 0 {
            ((flags | own) & !6) | (own & 6)
        } else {
            flags | own
        };
        finite(&color)?;
        Ok((color, flags))
    }
}

fn consume(steps: &mut usize) -> Result<()> {
    ensure!(*steps != 0, "frontend animation step budget exhausted");
    *steps -= 1;
    Ok(())
}

fn apply_tracks(
    tracks: &Tracks,
    frame: f32,
    cursors: &mut Cursors,
    pose: &mut Pose,
    selection: &mut Option<FrameSelection>,
) -> Result<()> {
    if cursors.finished & 4 == 0
        && let Some(keys) = &tracks.translation
    {
        let (v, done) = sample_vector(keys, frame, &mut cursors.translation)?;
        pose.translation = v;
        if done {
            cursors.finished |= 4;
        }
    }
    if cursors.finished & 8 == 0
        && let Some(keys) = &tracks.rotation
    {
        let (v, done) = sample_quaternion(keys, frame, &mut cursors.rotation)?;
        pose.rotation = v;
        if done {
            cursors.finished |= 8;
        }
    }
    if cursors.finished & 16 == 0
        && let Some(keys) = &tracks.scale
    {
        let (v, done) = sample_vector(keys, frame, &mut cursors.scale)?;
        pose.scale = [v[0], v[1], v[2], 0.];
        if done {
            cursors.finished |= 16;
        }
    }
    if cursors.finished & 64 == 0
        && let Some(keys) = &tracks.rgb
    {
        let (v, done) = sample_vector(keys, frame, &mut cursors.rgb)?;
        pose.color[..3].copy_from_slice(&v);
        pose.color_flags_raw |= 4;
        if done {
            cursors.finished |= 64;
        }
    }
    if cursors.finished & 128 == 0
        && let Some(keys) = &tracks.alpha
    {
        let (v, done) = sample_scalar(keys, frame, &mut cursors.alpha)?;
        pose.color[3] = v;
        if v < 0.999 {
            pose.color_flags_raw |= 2;
        } else {
            pose.color_flags_raw &= !2;
        }
        if done {
            cursors.finished |= 128;
        }
    }
    if cursors.finished & 512 == 0
        && let Some(selector) = &tracks.frame_selector
    {
        let (v, done) = sample_scalar(&selector.keys, frame, &mut cursors.frame_selector)?;
        *selection = Some(select_frame(selector, v)?);
        if done {
            cursors.finished |= 512;
        }
    }
    Ok(())
}

fn multiply(a: &[f32; 16], b: &[f32; 16]) -> Result<[f32; 16]> {
    let value = std::array::from_fn(|index| {
        let row = index % 4;
        let col = index / 4 * 4;
        let x = a[row] * b[col];
        let y = a[row + 4].mul_add(b[col + 1], x);
        let z = a[row + 8].mul_add(b[col + 2], y);
        a[row + 12].mul_add(b[col + 3], z)
    });
    finite(&value)?;
    Ok(value)
}

impl Pose {
    /// Native quaternion matrix, right-concatenated diagonal scale (R*S), then
    /// translation installed. No quaternion normalization or Euler conversion.
    pub fn matrix(&self) -> Result<[f32; 16]> {
        finite(&self.rotation)?;
        finite(&self.scale)?;
        finite(&self.translation)?;
        let [x, y, z, w] = self.rotation;
        let xx = x + x;
        let yy = y + y;
        let zz = z + z;
        let xy = x * yy;
        let xz = x * zz;
        let yz = y * zz;
        let yw = yy * w;
        let zw = w * zz;
        let xw = xx * w;
        let y2 = y * yy;
        let z2 = z * zz;
        let mut matrix = [
            (y2 + z2).mul_add(-1., 1.),
            xy + zw,
            xz - yw,
            0.,
            xy - zw,
            x.mul_add(xx, z2).mul_add(-1., 1.),
            yz + xw,
            0.,
            xz + yw,
            yz - xw,
            x.mul_add(xx, y2).mul_add(-1., 1.),
            0.,
            0.,
            0.,
            0.,
            1.,
        ];
        if self
            .scale
            .iter()
            .any(|v| (*v - 1.).abs() >= f32::from_bits(0x3a83_126f))
        {
            let mut diagonal = [0.; 16];
            for i in 0..4 {
                diagonal[i * 5] = self.scale[i];
            }
            matrix = multiply(&matrix, &diagonal)?;
        }
        matrix[12..15].copy_from_slice(&self.translation);
        matrix[15] = 1.;
        finite(&matrix)?;
        Ok(matrix)
    }
}

#[cfg(test)]
#[path = "tests/frontend_animation.rs"]
mod tests;
