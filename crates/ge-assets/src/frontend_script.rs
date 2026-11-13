//! Bounded, lazy views of frontend 0x300 scripts. This is an asset reader, not a
//! bytecode interpreter: unknown commands and track descriptors remain explicit.
use crate::{binary::Bytes, edb::Table};
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
mod live;
pub use live::SelectionEntry;

pub const TRANSLATION: u32 = 0x04;
pub const ROTATION: u32 = 0x08;
pub const SCALE: u32 = 0x10;
pub const RGB: u32 = 0x40;
pub const ALPHA: u32 = 0x80;
pub const FRAME_SELECTOR: u32 = 0x200;

#[derive(Debug, Clone, Serialize)]
pub struct Header {
    pub address: usize,
    pub duration_raw: u32,
    pub slots: u8,
    pub variables: u8,
    pub flags_raw: u16,
    pub frames_per_second: f32,
    /// Stored min/max XYZW words; unused lanes are not interpreted or validated.
    pub bounds_words_raw: [u32; 8],
    pub table_30_relative_raw: i32,
    pub tracks_relative_raw: i32,
    pub slot_resources_relative_raw: i32,
    pub command_count_raw: u16,
    pub channel_cache_count_raw: u16,
    pub state_flags_raw: u32,
}

#[derive(Clone, Copy)]
pub struct Script<'a> {
    data: &'a [u8],
    address: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Command {
    pub address: usize,
    pub opcode: u8,
    pub next_bytes: u8,
    pub previous_bytes: u8,
    pub flags_raw: u8,
    pub start: i16,
    pub duration: i16,
    /// The complete addressed record, including its common header.
    pub bytes_raw: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum InstanceKind {
    Entity,
    Script,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceCommand {
    pub kind: InstanceKind,
    pub track_selector: u16,
    pub slot: u8,
    pub parent_slot_raw: u8,
    /// Lookup tag used to find the live component created by this command.
    /// This is not a pose/transform reference.
    pub word_0c_raw: u32,
    /// 0xffffffff selects the owning file. Other values remain coded references.
    pub file_reference_raw: u32,
    pub reference_raw: u32,
}

/// Opcode two's source clip list, before resident-file binding or sampling.
/// Original records deliberately have no public receipt serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayAnimation {
    pub address: usize,
    pub track_selector: u16,
    pub word_0c_raw: u32,
    pub normal_count_raw: u8,
    pub flags_raw: u8,
    pub initial_index_raw: u8,
    pub secondary_count_raw: u8,
    pub word_4_raw: u32,
    pub word_8_raw: u32,
    pub normal_clips: Vec<AnimationClip>,
    pub secondary_clips: Vec<SecondaryAnimationClip>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationClip {
    pub source_address: usize,
    /// 0xffffffff selects the owning file; other tokens remain unresolved.
    pub file_reference_raw: u32,
    pub animation_reference: u32,
    pub blend_key_bits: u32,
    pub start_raw: i16,
    pub end_raw: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondaryAnimationClip {
    pub source_address: usize,
    pub file_reference_raw: u32,
    pub animation_reference: u32,
    pub start_raw: i16,
    pub end_raw: i16,
}

impl Command {
    /// Decode only a reached opcode-two payload. Clip references, repeated rows,
    /// float bits and signed ranges stay raw; no files or animation bodies load.
    pub fn play_animation(&self, maximum_clips: usize) -> Result<Option<PlayAnimation>> {
        if self.opcode != 2 {
            return Ok(None);
        }
        let b = Bytes(&self.bytes_raw);
        let header = b.range(0x10, 12).context("short play-animation header")?;
        let normal_count = usize::from(header[0]);
        let secondary_count = usize::from(header[3]);
        ensure!(
            normal_count + secondary_count <= maximum_clips,
            "play-animation clip budget exhausted"
        );
        let secondary_at = 0x1c + normal_count * 16;
        b.range(0x1c, normal_count * 16 + secondary_count * 12)
            .context("short play-animation clip list")?;
        let mut normal_clips = Vec::new();
        normal_clips
            .try_reserve_exact(normal_count)
            .context("allocating bounded animation clips")?;
        for index in 0..normal_count {
            let at = 0x1c + index * 16;
            normal_clips.push(AnimationClip {
                source_address: self
                    .address
                    .checked_add(at)
                    .context("clip address overflow")?,
                file_reference_raw: b.u32(at)?,
                animation_reference: b.u32(at + 4)?,
                blend_key_bits: b.u32(at + 8)?,
                start_raw: b.u16(at + 12)? as i16,
                end_raw: b.u16(at + 14)? as i16,
            });
        }
        let mut secondary_clips = Vec::new();
        secondary_clips
            .try_reserve_exact(secondary_count)
            .context("allocating bounded secondary animation clips")?;
        for index in 0..secondary_count {
            let at = secondary_at + index * 12;
            secondary_clips.push(SecondaryAnimationClip {
                source_address: self
                    .address
                    .checked_add(at)
                    .context("clip address overflow")?,
                file_reference_raw: b.u32(at)?,
                animation_reference: b.u32(at + 4)?,
                start_raw: b.u16(at + 8)? as i16,
                end_raw: b.u16(at + 10)? as i16,
            });
        }
        Ok(Some(PlayAnimation {
            address: self
                .address
                .checked_add(0x10)
                .context("play address overflow")?,
            track_selector: b.u16(8)?,
            word_0c_raw: b.u32(12)?,
            normal_count_raw: header[0],
            flags_raw: header[1],
            initial_index_raw: header[2],
            secondary_count_raw: header[3],
            word_4_raw: b.u32(0x14)?,
            word_8_raw: b.u32(0x18)?,
            normal_clips,
            secondary_clips,
        }))
    }

    pub fn instance(&self) -> Result<Option<InstanceCommand>> {
        let kind = match self.opcode {
            3 => InstanceKind::Entity,
            4 => InstanceKind::Script,
            _ => return Ok(None),
        };
        let b = Bytes(&self.bytes_raw);
        b.range(0, 24).context("short frontend instance command")?;
        Ok(Some(InstanceCommand {
            kind,
            track_selector: b.u16(8)?,
            slot: self.bytes_raw[10],
            parent_slot_raw: self.bytes_raw[11],
            word_0c_raw: b.u32(12)?,
            file_reference_raw: b.u32(16)?,
            reference_raw: b.u32(20)?,
        }))
    }
}

/// Resolve one coded table-five reference; no nested scripts or tracks are read.
pub fn read<'a>(data: &'a [u8], table: &Table, reference: u32) -> Result<Option<Script<'a>>> {
    ensure!(
        table.name == "animation_scripts",
        "script reader requires table five"
    );
    let Some(index) = crate::reference::resolve(data, table, 16, reference)? else {
        return Ok(None);
    };
    let at = table
        .offset
        .checked_add(index.checked_mul(16).context("script row overflow")?)
        .context("script row overflow")?;
    let b = Bytes(data);
    b.range(at, 16)?;
    Ok(Some(read_at(data, b.u32(at + 8)? as usize)?))
}

pub fn read_at(data: &[u8], address: usize) -> Result<Script<'_>> {
    let b = Bytes(data);
    b.range(address, 0x44)
        .context("script header outside resource")?;
    ensure!(b.u32(address)? == 0x300, "unsupported frontend script type");
    Ok(Script { data, address })
}

impl<'a> Script<'a> {
    pub fn header(self) -> Result<Header> {
        let b = Bytes(self.data);
        let a = self.address;
        let mut bounds_words_raw = [0; 8];
        for (i, value) in bounds_words_raw.iter_mut().enumerate() {
            *value = b.u32(a + 0x10 + i * 4)?;
        }
        Ok(Header {
            address: a,
            duration_raw: b.u32(a + 4)?,
            slots: self.data[a + 8],
            variables: self.data[a + 9],
            flags_raw: b.u16(a + 10)?,
            frames_per_second: b.f32(a + 12)?,
            bounds_words_raw,
            table_30_relative_raw: b.u32(a + 0x30)? as i32,
            tracks_relative_raw: b.u32(a + 0x34)? as i32,
            slot_resources_relative_raw: b.u32(a + 0x38)? as i32,
            command_count_raw: b.u16(a + 0x3c)?,
            channel_cache_count_raw: b.u16(a + 0x3e)?,
            state_flags_raw: b.u32(a + 0x40)?,
        })
    }

    /// Read only this record. Forward and previous links are byte displacements,
    /// never instruction counts. A terminal record has an eight-byte header.
    pub fn command_at(self, address: usize) -> Result<Command> {
        let b = Bytes(self.data);
        let header = b.range(address, 8)?;
        let length = if header[1] == 0 {
            8
        } else {
            usize::from(header[1])
        };
        ensure!(length >= 8, "frontend command step overlaps header");
        let command = Command {
            address,
            opcode: header[0],
            next_bytes: header[1],
            previous_bytes: header[2],
            flags_raw: header[3],
            start: b.u16(address + 4)? as i16,
            duration: b.u16(address + 6)? as i16,
            bytes_raw: b.range(address, length)?.to_vec(),
        };
        if matches!(command.opcode, 3 | 4) {
            command.instance()?;
        }
        Ok(command)
    }

    /// Walk storage order through the terminal 0x12, including that record.
    /// Header counts are retained metadata, not a substitute for link traversal.
    pub fn commands(self, maximum_commands: usize) -> Result<Vec<Command>> {
        let mut at = self.address + 0x44;
        let mut commands = Vec::new();
        loop {
            ensure!(
                commands.len() < maximum_commands,
                "frontend command budget exhausted"
            );
            let command = self.command_at(at)?;
            let next = command.next_bytes;
            let terminal = command.opcode == 0x12;
            commands
                .try_reserve(1)
                .context("allocating frontend commands")?;
            commands.push(command);
            if terminal {
                ensure!(next == 0, "frontend terminal command has forward link");
                return Ok(commands);
            }
            ensure!(next != 0, "frontend command chain lacks terminal opcode");
            at = at
                .checked_add(usize::from(next))
                .context("frontend command address overflow")?;
        }
    }

    /// Address one bundle via the command's u16 selector and table +0x34.
    /// No reliable serialized table-length field is established; the selector
    /// range is u16 and every addressed field is checked against the resource.
    /// Unselected pointers, descriptors and key arrays remain unread.
    pub fn track_bundle(self, selector: u16) -> Result<TrackBundle<'a>> {
        let b = Bytes(self.data);
        let table = b.relative(self.address + 0x34)?;
        let field = table
            .checked_add(usize::from(selector) * 4)
            .context("track selector overflow")?;
        ensure!(b.u32(field)? != 0, "null frontend track bundle pointer");
        let address = b.relative(field)?;
        b.range(address, 12)?;
        Ok(TrackBundle {
            data: self.data,
            address,
            header_word_raw: b.u32(address)?,
            channel_mask: b.u32(address + 4)?,
            option_flags_raw: b.u32(address + 8)?,
        })
    }
}

#[derive(Clone, Copy)]
pub struct TrackBundle<'a> {
    data: &'a [u8],
    pub address: usize,
    pub header_word_raw: u32,
    pub channel_mask: u32,
    pub option_flags_raw: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackDescriptor {
    pub address: usize,
    pub channel: u32,
    pub count: i16,
    pub auxiliary_raw: i16,
    pub data_relative_raw: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrameSelectorExtension {
    pub address: usize,
    /// Used as a divisor when the scalar descriptor count is zero.
    pub period_raw: u16,
    /// Zero: direct frame pair; positive: number of signed index entries;
    /// negative: external target/property bindings (not decoded here).
    pub mode_raw: i8,
    pub playback_mode_raw: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Key<const N: usize> {
    pub time: f32,
    #[serde(with = "key_values")]
    pub value: [f32; N],
}
mod key_values {
    use serde::{Serialize, Serializer};
    pub fn serialize<S: Serializer, const N: usize>(
        value: &[f32; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.as_slice().serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum LinearKeys {
    Scalar(Vec<Key<1>>),
    Vector(Vec<Key<3>>),
    Quaternion(Vec<Key<4>>),
}

impl TrackBundle<'_> {
    /// Descriptors are packed in ascending set-bit order, including unknown bits.
    pub fn descriptor(self, channel: u32) -> Result<Option<TrackDescriptor>> {
        ensure!(channel.is_power_of_two(), "track channel must be one bit");
        if self.channel_mask & channel == 0 {
            return Ok(None);
        }
        let ordinal = (self.channel_mask & (channel - 1)).count_ones() as usize;
        let address = self
            .address
            .checked_add(12 + ordinal * 8)
            .context("track descriptor overflow")?;
        let b = Bytes(self.data);
        Ok(Some(TrackDescriptor {
            address,
            channel,
            count: b.u16(address)? as i16,
            auxiliary_raw: b.u16(address + 2)? as i16,
            data_relative_raw: b.u32(address + 4)? as i32,
        }))
    }

    /// Positive-count keys only. Zero-count tracks cannot be evaluated by the
    /// source's count-minus-one cursor. Negative/cubic tracks and timewarp
    /// channels require distinct formats; none become empty animations.
    pub fn linear_keys(self, channel: u32, maximum_keys: usize) -> Result<Option<LinearKeys>> {
        let Some(descriptor) = self.descriptor(channel)? else {
            return Ok(None);
        };
        ensure!(
            descriptor.count >= 0,
            "cubic frontend tracks are unsupported"
        );
        ensure!(
            descriptor.count != 0,
            "zero-count frontend track cannot be evaluated"
        );
        let count = descriptor.count as usize;
        ensure!(count <= maximum_keys, "frontend key budget exhausted");
        let keys = match channel {
            TRANSLATION | SCALE | RGB => LinearKeys::Vector(self.keys::<3>(&descriptor)?),
            ROTATION => LinearKeys::Quaternion(self.keys::<4>(&descriptor)?),
            ALPHA | FRAME_SELECTOR => LinearKeys::Scalar(self.keys::<1>(&descriptor)?),
            _ => bail!("unsupported frontend track channel {channel:#x}"),
        };
        Ok(Some(keys))
    }

    /// Channel 0x200's extension is relative to the signed halfword at descriptor
    /// +2, not relative to the descriptor or its ordinary key-data pointer.
    pub fn frame_selector_extension(self) -> Result<Option<FrameSelectorExtension>> {
        let Some(descriptor) = self.descriptor(FRAME_SELECTOR)? else {
            return Ok(None);
        };
        let address = descriptor.address as i64 + 2 + i64::from(descriptor.auxiliary_raw);
        ensure!(address >= 0, "frame-selector extension underflow");
        let address = usize::try_from(address).context("frame-selector extension overflow")?;
        let bytes = Bytes(self.data).range(address, 4)?;
        Ok(Some(FrameSelectorExtension {
            address,
            period_raw: u16::from_be_bytes([bytes[0], bytes[1]]),
            mode_raw: bytes[2] as i8,
            playback_mode_raw: bytes[3],
        }))
    }

    /// Decode only the positive-mode index table. A direct (zero) mode has no
    /// table. External negative modes fail explicitly rather than dropping work.
    pub fn frame_selector_indices(self, maximum_indices: usize) -> Result<Option<Vec<i16>>> {
        let Some(extension) = self.frame_selector_extension()? else {
            return Ok(None);
        };
        ensure!(
            extension.mode_raw >= 0,
            "external frame-selector bindings are unsupported"
        );
        let count = extension.mode_raw as usize;
        ensure!(
            count <= maximum_indices,
            "frame-selector index budget exhausted"
        );
        let b = Bytes(self.data);
        b.range(extension.address + 4, count * 2)?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(count)
            .context("allocating frame-selector indices")?;
        for index in 0..count {
            indices.push(b.u16(extension.address + 4 + index * 2)? as i16);
        }
        Ok(Some(indices))
    }

    fn keys<const N: usize>(self, descriptor: &TrackDescriptor) -> Result<Vec<Key<N>>> {
        let count = descriptor.count as usize;
        ensure!(
            descriptor.data_relative_raw != 0,
            "null frontend key pointer"
        );
        let b = Bytes(self.data);
        let address = b.relative(descriptor.address + 4)?;
        let stride = (N + 1) * 4;
        b.range(
            address,
            count
                .checked_mul(stride)
                .context("frontend key span overflow")?,
        )?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(count)
            .context("allocating frontend keys")?;
        for index in 0..count {
            let at = address + index * stride;
            let mut value = [0.; N];
            for (lane, value) in value.iter_mut().enumerate() {
                *value = b.f32(at + 4 + lane * 4)?;
            }
            keys.push(Key {
                time: b.f32(at)?,
                value,
            });
        }
        Ok(keys)
    }
}

#[cfg(test)]
#[path = "tests/frontend_script.rs"]
mod tests;
