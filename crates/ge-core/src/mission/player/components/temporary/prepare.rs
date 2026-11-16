//! Skeleton sample preparation reached by the outer evaluator with copyDefaults
//! 0. The primary skin lookup always runs; only its known results may skip
//! later work. Unknown publication pauses with the completed prefix retained.
use super::ResourceBinding;
use crate::animation::scratch::evaluation::Step;
use anyhow::{Result, bail, ensure};

/// Owner-bound descriptor of the actual skeletal primary binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinDescriptor {
    pub file: u32,
    pub reference: u32,
    /// Publication generation of the retained wrapper at lookup time.
    pub generation: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinLookup {
    Absent,
    Present(SkinDescriptor),
    Pending,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinBody {
    Absent,
    Resident,
    Pending,
}
/// App adapter over the same retained skin file that initialized the skeleton.
pub trait SkinSource {
    fn lookup(&mut self, primary: ResourceBinding) -> Result<SkinLookup>;
    fn generation(&mut self, primary: ResourceBinding) -> Result<u32>;
    /// Runtime body pointer at descriptor+c. Unknown publication is Pending.
    fn body(&mut self, descriptor: SkinDescriptor) -> Result<SkinBody>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Return {
    DescriptorAbsent,
    BodyAbsent,
    /// Resident body, copyDefaults 0 and auxiliary count 0.
    AuxiliaryZero,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    Lookup,
    Body,
    /// Nonzero auxiliary count reaches an unimplemented scratch clear.
    Auxiliary,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepare {
    primary: ResourceBinding,
    descriptor: Option<SkinDescriptor>,
    body_resident: bool,
    auxiliary_words: Option<usize>,
    lookups: u32,
    body_reads: u32,
    returned: Option<Return>,
    pause: Option<Pause>,
}
impl Prepare {
    pub(super) fn new(primary: ResourceBinding) -> Self {
        Self {
            primary,
            descriptor: None,
            body_resident: false,
            auxiliary_words: None,
            lookups: 0,
            body_reads: 0,
            returned: None,
            pause: None,
        }
    }
    pub(super) fn advance(
        &mut self,
        source: &mut impl SkinSource,
        auxiliary_words: usize,
    ) -> Result<Step<()>> {
        ensure!(self.returned.is_none(), "skin preparation replays");
        self.pause = None;
        let descriptor = match self.descriptor {
            Some(descriptor) => descriptor,
            None => {
                self.lookups += 1;
                match source.lookup(self.primary)? {
                    SkinLookup::Pending => return Ok(self.paused(Pause::Lookup)),
                    SkinLookup::Absent => return Ok(self.finish(Return::DescriptorAbsent)),
                    SkinLookup::Present(descriptor) => {
                        ensure!(
                            descriptor.file == self.primary.file
                                && descriptor.reference == self.primary.reference,
                            "skin descriptor belongs to another skeletal binding"
                        );
                        self.descriptor = Some(descriptor);
                        descriptor
                    }
                }
            }
        };
        if !self.body_resident {
            // The source is synchronous. After a host pause the same wrapper
            // must still publish the generation the lookup observed.
            if self.body_reads > 0 && source.generation(self.primary)? != descriptor.generation {
                bail!("skin publication generation changed while paused");
            }
            self.body_reads += 1;
            match source.body(descriptor)? {
                SkinBody::Pending => return Ok(self.paused(Pause::Body)),
                SkinBody::Absent => return Ok(self.finish(Return::BodyAbsent)),
                SkinBody::Resident => self.body_resident = true,
            }
        }
        // copyDefaults 0 skips the default-local copy loop entirely.
        self.auxiliary_words = Some(auxiliary_words);
        if auxiliary_words != 0 {
            return Ok(self.paused(Pause::Auxiliary));
        }
        Ok(self.finish(Return::AuxiliaryZero))
    }
    fn paused(&mut self, pause: Pause) -> Step<()> {
        self.pause = Some(pause);
        Step::Pending
    }
    fn finish(&mut self, value: Return) -> Step<()> {
        self.returned = Some(value);
        Step::Done(())
    }
    pub fn primary(&self) -> ResourceBinding {
        self.primary
    }
    pub fn descriptor(&self) -> Option<SkinDescriptor> {
        self.descriptor
    }
    pub fn body_resident(&self) -> bool {
        self.body_resident
    }
    pub fn auxiliary_words(&self) -> Option<usize> {
        self.auxiliary_words
    }
    pub fn lookups(&self) -> u32 {
        self.lookups
    }
    pub fn body_reads(&self) -> u32 {
        self.body_reads
    }
    pub fn returned(&self) -> Option<Return> {
        self.returned
    }
    pub fn pause(&self) -> Option<Pause> {
        self.pause
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(self.primary.file);
        emit(self.primary.reference);
        emit(u32::from(self.descriptor.is_some()));
        if let Some(d) = self.descriptor {
            emit(d.file);
            emit(d.reference);
            emit(d.generation);
        }
        emit(u32::from(self.body_resident));
        emit(u32::from(self.auxiliary_words.is_some()));
        if let Some(words) = self.auxiliary_words {
            emit(words as u32);
        }
        emit(self.lookups);
        emit(self.body_reads);
        emit(self.returned.map_or(0, |r| r as u32 + 1));
        emit(self.pause.map_or(0, |p| p as u32 + 1));
    }
}
