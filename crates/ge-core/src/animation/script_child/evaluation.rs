//! Class-four virtual +2c entry guards. Only the two reference sentinels are
//! executed; every later sampling read stays behind an explicit boundary.
use super::*;

/// Source compares the raw reference with the all-ones sentinel.
pub const NO_REFERENCE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// +5c role is not modeled (replaced storage) or is a NULL pointer.
    ClipPointer,
    /// Current record +4 is unwritten or unknown.
    ClipReference,
    /// Animator+20 was reached but is unknown.
    AnimatorReference,
    /// Both references are real: skeleton, masks, weights and frames follow.
    Sampling,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Literal return byte; the caller sign-extends it.
    Returned(u8),
    Pending(Boundary),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trace {
    pub node: NodeId,
    pub pointer: ClipPointer,
    pub clip_reference: Option<u32>,
    pub animator_reference_read: bool,
    pub animator_reference: Option<u32>,
    pub outcome: Outcome,
}
impl Trace {
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        emit((self.node.get() >> 32) as u32);
        emit(self.node.get() as u32);
        emit(match self.pointer {
            ClipPointer::ConstructorInline => 0,
            ClipPointer::Record(_) => 1,
            ClipPointer::Null => 2,
            ClipPointer::Unknown => 3,
        });
        if let ClipPointer::Record(index) = self.pointer {
            emit(index as u32);
        }
        for value in [self.clip_reference, self.animator_reference] {
            emit(u32::from(value.is_some()));
            if let Some(value) = value {
                emit(value);
            }
        }
        emit(u32::from(self.animator_reference_read));
        match self.outcome {
            Outcome::Returned(byte) => {
                emit(0);
                emit(u32::from(byte));
            }
            Outcome::Pending(boundary) => {
                emit(1);
                emit(boundary as u32);
            }
        }
    }
}
impl Store {
    /// Runs the entry guards of the actual header without mutating it. The
    /// animator reference is requested only when the current clip is real.
    pub fn evaluate_guard(
        &self,
        id: NodeId,
        animator_reference: impl FnOnce() -> Result<Option<u32>>,
    ) -> Result<Trace> {
        let header = self.headers.get(&id).context("evaluated script absent")?;
        ensure!(
            header.initializer_returned(),
            "script evaluation precedes the actual factory return"
        );
        let mut trace = Trace {
            node: id,
            pointer: header.current_clip,
            clip_reference: None,
            animator_reference_read: false,
            animator_reference: None,
            outcome: Outcome::Pending(Boundary::ClipPointer),
        };
        let reference = match header.current_clip {
            ClipPointer::ConstructorInline => header.word(0x44),
            ClipPointer::Record(index) => header.clip_word(index, 4),
            ClipPointer::Null | ClipPointer::Unknown => return Ok(trace),
        };
        let Some(reference) = reference else {
            trace.outcome = Outcome::Pending(Boundary::ClipReference);
            return Ok(trace);
        };
        trace.clip_reference = Some(reference);
        if reference == NO_REFERENCE {
            trace.outcome = Outcome::Returned(0);
            return Ok(trace);
        }
        trace.animator_reference_read = true;
        let Some(animator) = animator_reference()? else {
            trace.outcome = Outcome::Pending(Boundary::AnimatorReference);
            return Ok(trace);
        };
        trace.animator_reference = Some(animator);
        trace.outcome = if animator == NO_REFERENCE {
            Outcome::Returned(0)
        } else {
            Outcome::Pending(Boundary::Sampling)
        };
        Ok(trace)
    }
}

#[cfg(test)]
#[path = "evaluation/tests.rs"]
mod tests;
