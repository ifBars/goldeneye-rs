//! Header+60 parameter pointer and writes through it. The base constructor
//! stores header+64 at +60 (80321614/2c); the class-four constructor does not
//! overwrite it. Bytes stay address-free; the role is the pointer identity.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterPointer {
    /// Points at this same header's +64 word.
    ConstructorInlinePhase,
    Null,
    /// A replacement producer that is not modeled.
    Unknown,
}
/// Receipt of one store through the actual pointee. A caller-specific record,
/// distinct from the dispatch-coordinate setter that writes the same word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterStore {
    pub node: NodeId,
    pub pointer: ParameterPointer,
    pub offset: usize,
    pub previous: Option<u32>,
    pub bits: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterWrite {
    Stored(ParameterStore),
    /// The pointee is not known; nothing was written.
    Pending(ParameterPointer),
}
impl ParameterStore {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit((self.node.get() >> 32) as u32);
        emit(self.node.get() as u32);
        emit(self.pointer as u32);
        emit(self.offset as u32);
        emit(u32::from(self.previous.is_some()));
        if let Some(previous) = self.previous {
            emit(previous);
        }
        emit(self.bits);
    }
}
impl Store {
    pub fn store_through_parameter(&mut self, id: NodeId, bits: u32) -> Result<ParameterWrite> {
        let header = self
            .headers
            .get_mut(&id)
            .context("parameter pointee script absent")?;
        ensure!(
            header.initializer_returned(),
            "parameter store precedes the actual factory return"
        );
        match header.parameter {
            ParameterPointer::ConstructorInlinePhase => {
                let previous = header.word(0x64);
                header.w(0x64, bits);
                Ok(ParameterWrite::Stored(ParameterStore {
                    node: id,
                    pointer: header.parameter,
                    offset: 0x64,
                    previous,
                    bits,
                }))
            }
            ParameterPointer::Null => {
                anyhow::bail!("script parameter pointer is NULL; the source store would fault")
            }
            ParameterPointer::Unknown => Ok(ParameterWrite::Pending(header.parameter)),
        }
    }
}
impl Header {
    pub fn parameter_pointer(&self) -> ParameterPointer {
        self.parameter
    }
}

#[cfg(test)]
#[path = "pointee/tests.rs"]
mod tests;
