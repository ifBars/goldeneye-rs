//! 800f7e40 after the second evaluation: six supplied tag hashes, each looked
//! up through the temporary's primary skin, copy the ACTUAL current bone record
//! (own index, local translation, local rotation) into the selected Array34
//! entry. No forward kinematics or matrix is read. Missing tags store only the
//! index 0xffff; a zero hash also leaves the hash cursor in place.
mod state;
use super::{Call, CallStage, Request, post_bind::PostBind, post_bind::Stage as PostBindStage};
use crate::mission::player::{
    components::{
        Identity, ResourceBinding,
        temporary::{self, SkinBody, SkinDescriptor, SkinLookup, SkinSource},
    },
    constructor::{Lease, PoseRegion, Shared},
};
use anyhow::{Context, Result, bail, ensure};

pub const REQUESTED: u8 = 6;
const MISSING_INDEX: u16 = 0xffff;

/// 80339ae0's walk over the skin's tag entries (low 16 bits compared).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagWalk {
    Found {
        bone: u16,
    },
    /// The 0xffff sentinel: the source NULL result.
    Sentinel,
    /// Provider cannot answer yet; never a missing tag.
    Pending,
}
/// Same retained skin owner as the evaluation's preparation, plus the six
/// supplied hash words and the tag walk. Errors are never "missing".
pub trait BoneTagSource: SkinSource {
    fn hash(&mut self, index: u8) -> Result<u32>;
    fn tag(&mut self, descriptor: SkinDescriptor, hash: u32) -> Result<TagWalk>;
}
/// The exact call this work belongs to; a tombstone carries it to the commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallBinding {
    pub request: Request,
    pub entry: u8,
    pub mapped: u32,
    pub reference: u32,
    pub temporary: Identity,
}
impl CallBinding {
    /// Fields read from the actual call; the temporary is attached by caller.
    pub(super) fn of_call(call: &Call, temporary: Identity) -> Result<Self> {
        ensure!(
            call.stage() == CallStage::AwaitingTemporary && call.extraction_returned(),
            "call has not reached its temporary after reference extraction"
        );
        Ok(Self {
            request: call.request(),
            entry: call.entry_index().context("call entry is unselected")?,
            mapped: call.mapping().context("call mapping is absent")?.output,
            reference: call.reference().unwrap_or(0),
            temporary,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    ZeroHash,
    DescriptorAbsent,
    BodyAbsent,
    Sentinel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Missing(Missing),
    Found { bone: u16 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Index,
    TranslationLow,
    TranslationHigh,
    RotationLow,
    RotationHigh,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Hash,
    Lookup,
    Body,
    Tag,
    Store(Step),
    Done,
    Failed,
    Aborted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependency {
    SkinLookup,
    SkinBody,
    Tag,
    Destructor,
}
impl Dependency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SkinLookup => "preload_bone_output_skin_lookup",
            Self::SkinBody => "preload_bone_output_skin_body",
            Self::Tag => "preload_bone_output_tag",
            Self::Destructor => "preload_temporary_destructor",
        }
    }
}
/// Per-iteration lookup prefix retained across a pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lookup {
    hash: u32,
    descriptor: Option<SkinDescriptor>,
    paused: bool,
}
#[derive(Debug)]
pub struct BoneOutputs {
    binding: CallBinding,
    stage: Stage,
    iteration: u8,
    hash_cursor: u8,
    hash_reads: u32,
    lookups: u32,
    body_reads: u32,
    tag_walks: u32,
    stores: u32,
    current: Option<Lookup>,
    found: Option<u16>,
    outcomes: [Option<Outcome>; REQUESTED as usize],
}
impl BoneOutputs {
    pub fn begin(
        call: &Call,
        post_bind: &PostBind,
        temporary: &temporary::Pending,
    ) -> Result<Self> {
        let identity = temporary.animator().identity();
        let binding = CallBinding::of_call(call, identity)?;
        ensure!(
            post_bind.stage() == PostBindStage::AwaitingBoneOutputs
                && post_bind.request() == binding.request
                && post_bind.temporary_identity() == identity
                && temporary.bind_return() == Some(1)
                && !temporary.teardown_started()
                && temporary.source_receiver() == binding.request.receiver
                && temporary.component().reference == binding.reference,
            "bone outputs require this call's actual post-bind temporary"
        );
        Ok(Self {
            binding,
            stage: Stage::Hash,
            iteration: 0,
            hash_cursor: 0,
            hash_reads: 0,
            lookups: 0,
            body_reads: 0,
            tag_walks: 0,
            stores: 0,
            current: None,
            found: None,
            outcomes: [None; REQUESTED as usize],
        })
    }
    pub fn advance(
        &mut self,
        lease: Lease,
        shared: &mut Shared,
        temporary: &temporary::Pending,
        source: &mut impl BoneTagSource,
    ) -> Result<()> {
        match self.stage {
            Stage::Failed => bail!("bone output failure is latched"),
            Stage::Aborted => bail!("bone outputs were cancelled"),
            Stage::Done => return Ok(()),
            _ => {}
        }
        ensure!(
            lease.player() == self.binding.request.player
                && temporary.animator().identity() == self.binding.temporary
                && temporary.bind_return() == Some(1)
                && !temporary.teardown_started(),
            "bone outputs resumed with another owner"
        );
        loop {
            match self.step(lease, shared, temporary, source) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(error) => {
                    self.stage = Stage::Failed;
                    return Err(error);
                }
            }
        }
    }
    fn primary(temporary: &temporary::Pending) -> Result<ResourceBinding> {
        Ok(temporary
            .animator()
            .skeleton()
            .context("temporary skeleton absent")?
            .primary())
    }
    /// After a host pause the same wrapper must still publish its generation.
    fn ensure_generation(
        lookup: &mut Lookup,
        primary: ResourceBinding,
        source: &mut impl BoneTagSource,
    ) -> Result<SkinDescriptor> {
        let descriptor = lookup.descriptor.context("tag descriptor absent")?;
        if lookup.paused {
            ensure!(
                source.generation(primary)? == descriptor.generation,
                "skin publication generation changed during bone outputs"
            );
            lookup.paused = false;
        }
        Ok(descriptor)
    }
    fn step(
        &mut self,
        lease: Lease,
        shared: &mut Shared,
        temporary: &temporary::Pending,
        source: &mut impl BoneTagSource,
    ) -> Result<bool> {
        let entry = self.binding.entry;
        match self.stage {
            Stage::Hash => {
                if self.iteration == REQUESTED {
                    self.stage = Stage::Done;
                    return Ok(false);
                }
                let hash = source.hash(self.hash_cursor)?;
                self.hash_reads += 1;
                if hash == 0 {
                    // Source stores 0xffff and does not advance the cursor.
                    shared.store_preload_entry_bone_index(
                        lease,
                        entry,
                        self.iteration,
                        MISSING_INDEX,
                    )?;
                    self.stores += 1;
                    self.outcomes[usize::from(self.iteration)] =
                        Some(Outcome::Missing(Missing::ZeroHash));
                    self.iteration += 1;
                } else {
                    self.current = Some(Lookup {
                        hash,
                        descriptor: None,
                        paused: false,
                    });
                    self.stage = Stage::Lookup;
                }
            }
            Stage::Lookup => {
                let primary = Self::primary(temporary)?;
                self.lookups += 1;
                match source.lookup(primary)? {
                    SkinLookup::Pending => return Ok(false),
                    SkinLookup::Absent => self.missing(lease, shared, Missing::DescriptorAbsent)?,
                    SkinLookup::Present(descriptor) => {
                        ensure!(
                            descriptor.file == primary.file
                                && descriptor.reference == primary.reference,
                            "tag descriptor belongs to another skeletal binding"
                        );
                        self.current
                            .as_mut()
                            .context("tag lookup absent")?
                            .descriptor = Some(descriptor);
                        self.stage = Stage::Body;
                    }
                }
            }
            Stage::Body => {
                let primary = Self::primary(temporary)?;
                let lookup = self.current.as_mut().context("tag lookup absent")?;
                let descriptor = Self::ensure_generation(lookup, primary, source)?;
                self.body_reads += 1;
                match source.body(descriptor)? {
                    SkinBody::Pending => {
                        lookup.paused = true;
                        return Ok(false);
                    }
                    SkinBody::Absent => self.missing(lease, shared, Missing::BodyAbsent)?,
                    SkinBody::Resident => self.stage = Stage::Tag,
                }
            }
            Stage::Tag => {
                let primary = Self::primary(temporary)?;
                let lookup = self.current.as_mut().context("tag lookup absent")?;
                let descriptor = Self::ensure_generation(lookup, primary, source)?;
                let hash = lookup.hash;
                self.tag_walks += 1;
                match source.tag(descriptor, hash)? {
                    TagWalk::Pending => {
                        lookup.paused = true;
                        return Ok(false);
                    }
                    TagWalk::Sentinel => self.missing(lease, shared, Missing::Sentinel)?,
                    TagWalk::Found { bone } => {
                        // The source does not range-check; the host must not
                        // copy outside the actual runtime bone records.
                        let record = Self::bone(temporary, bone)?;
                        ensure!(
                            record.index() == bone,
                            "runtime bone record index differs from its tag bone"
                        );
                        self.found = Some(bone);
                        self.outcomes[usize::from(self.iteration)] = Some(Outcome::Found { bone });
                        self.stage = Stage::Store(Step::Index);
                    }
                }
            }
            Stage::Store(step) => {
                let bone = self.found.context("found bone absent")?;
                let record = Self::bone(temporary, bone)?;
                let ordinal = self.iteration;
                let (region, high, bits) = match step {
                    Step::Index => {
                        shared.store_preload_entry_bone_index(
                            lease,
                            entry,
                            ordinal,
                            record.index(),
                        )?;
                        self.stores += 1;
                        self.stage = Stage::Store(Step::TranslationLow);
                        return Ok(true);
                    }
                    Step::TranslationLow => (
                        PoseRegion::Translation,
                        false,
                        record.local_translation_bits(),
                    ),
                    Step::TranslationHigh => (
                        PoseRegion::Translation,
                        true,
                        record.local_translation_bits(),
                    ),
                    Step::RotationLow => {
                        (PoseRegion::Rotation, false, record.local_quaternion_bits())
                    }
                    Step::RotationHigh => {
                        (PoseRegion::Rotation, true, record.local_quaternion_bits())
                    }
                };
                let lanes = if high {
                    [bits[2], bits[3]]
                } else {
                    [bits[0], bits[1]]
                };
                shared.store_preload_entry_pose_pair(lease, entry, region, ordinal, high, lanes)?;
                self.stores += 1;
                self.stage = match step {
                    Step::Index | Step::TranslationLow => Stage::Store(Step::TranslationHigh),
                    Step::TranslationHigh => Stage::Store(Step::RotationLow),
                    Step::RotationLow => Stage::Store(Step::RotationHigh),
                    Step::RotationHigh => {
                        self.found = None;
                        self.advance_iteration();
                        Stage::Hash
                    }
                };
            }
            Stage::Done | Stage::Failed | Stage::Aborted => return Ok(false),
        }
        Ok(true)
    }
    fn bone(
        temporary: &temporary::Pending,
        bone: u16,
    ) -> Result<&crate::animation::skeleton::Bone> {
        temporary
            .animator()
            .skeleton()
            .context("temporary skeleton absent")?
            .skeleton()
            .bones()
            .get(usize::from(bone))
            .context("tag bone is outside the runtime skeleton")
    }
    /// A source NULL result: only the index 0xffff is stored.
    fn missing(&mut self, lease: Lease, shared: &mut Shared, missing: Missing) -> Result<()> {
        shared.store_preload_entry_bone_index(
            lease,
            self.binding.entry,
            self.iteration,
            MISSING_INDEX,
        )?;
        self.stores += 1;
        self.outcomes[usize::from(self.iteration)] = Some(Outcome::Missing(missing));
        self.advance_iteration();
        self.stage = Stage::Hash;
        Ok(())
    }
    fn advance_iteration(&mut self) {
        self.current = None;
        self.iteration += 1;
        self.hash_cursor += 1;
    }
    /// Keeps every completed store; resumption is rejected afterwards.
    pub fn abort(&mut self) {
        if self.stage != Stage::Done {
            self.stage = Stage::Aborted;
        }
    }
    pub fn dependency(&self) -> Option<Dependency> {
        match self.stage {
            Stage::Lookup => Some(Dependency::SkinLookup),
            Stage::Body => Some(Dependency::SkinBody),
            Stage::Tag => Some(Dependency::Tag),
            Stage::Done => Some(Dependency::Destructor),
            _ => None,
        }
    }
    pub fn binding(&self) -> CallBinding {
        self.binding
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn iteration(&self) -> u8 {
        self.iteration
    }
    pub fn hash_cursor(&self) -> u8 {
        self.hash_cursor
    }
    pub fn hash_reads(&self) -> u32 {
        self.hash_reads
    }
    pub fn lookups(&self) -> u32 {
        self.lookups
    }
    pub fn body_reads(&self) -> u32 {
        self.body_reads
    }
    pub fn tag_walks(&self) -> u32 {
        self.tag_walks
    }
    pub fn stores(&self) -> u32 {
        self.stores
    }
    pub fn outcomes(&self) -> &[Option<Outcome>; REQUESTED as usize] {
        &self.outcomes
    }
}

#[cfg(test)]
pub(super) mod tests;
