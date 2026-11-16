//! Class-four child destructor (803253b0, delete=1) as a resumable cursor in
//! source order: +0x8c linked base child, then the +0x76/+0x5c heap clip free,
//! then the +0x78 listener drain, then the header free. Each completed phase's
//! effect stays applied; an unknown or populated field needing an unaudited
//! release pauses at that exact phase. The header leaves the store only when
//! the destructor actually returns.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildDependency {
    /// A gating word/byte, or the heap metadata a free needs, is not known.
    ChildState,
    /// +0x78 listener list is populated.
    ChildReferenceList,
    /// +0x8c linked base child is populated.
    LinkedBaseChild,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    LinkedChild,
    Clips,
    ReferenceList,
    Free,
    Returned,
}
/// Retained position inside one header's destructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destructor {
    node: NodeId,
    phase: Phase,
    heap_records: Option<usize>,
}
impl Destructor {
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// Heap clip records already released by the reached clip phase.
    pub fn heap_records(&self) -> Option<usize> {
        self.heap_records
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit((self.node.get() >> 32) as u32);
        emit(self.node.get() as u32);
        emit(self.phase as u32);
        emit(u32::from(self.heap_records.is_some()));
        if let Some(records) = self.heap_records {
            emit(records as u32);
        }
    }
}
/// Bounded receipt of one destroyed header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildReceipt {
    pub node: NodeId,
    pub heap_records: Option<usize>,
}
impl ChildReceipt {
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit((self.node.get() >> 32) as u32);
        emit(self.node.get() as u32);
        emit(u32::from(self.heap_records.is_some()));
        if let Some(records) = self.heap_records {
            emit(records as u32);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Returned(ChildReceipt),
    Blocked(ChildDependency),
}
impl Store {
    /// Entry of the virtual destructor for an already unlinked header.
    pub(crate) fn begin_destructor(&self, id: NodeId) -> Result<Destructor> {
        ensure!(
            self.headers.contains_key(&id),
            "destroyed script header absent"
        );
        Ok(Destructor {
            node: id,
            phase: Phase::LinkedChild,
            heap_records: None,
        })
    }
    /// Runs from the retained phase until a typed pause or the return.
    pub(crate) fn advance_destructor(&mut self, destructor: &mut Destructor) -> Result<Step> {
        loop {
            let id = destructor.node;
            match destructor.phase {
                Phase::LinkedChild => {
                    let header = self.headers.get(&id).context("destroyed script absent")?;
                    match header.word(0x8c) {
                        Some(0) => destructor.phase = Phase::Clips,
                        Some(_) => return Ok(Step::Blocked(ChildDependency::LinkedBaseChild)),
                        None => return Ok(Step::Blocked(ChildDependency::ChildState)),
                    }
                }
                Phase::Clips => {
                    let header = self
                        .headers
                        .get_mut(&id)
                        .context("destroyed script absent")?;
                    match header.byte(0x76) {
                        None => return Ok(Step::Blocked(ChildDependency::ChildState)),
                        Some(0) => {}
                        // A NULL +0x5c skips the free in the source.
                        Some(_) if header.current_clip == ClipPointer::Null => {}
                        Some(_) => {
                            let heap = matches!(
                                (&header.clips, header.current_clip),
                                (
                                    Some(initialization::Storage::Heap(_)),
                                    ClipPointer::Record(0)
                                )
                            );
                            if !heap {
                                // Never fake a free of unmodeled heap metadata.
                                return Ok(Step::Blocked(ChildDependency::ChildState));
                            }
                            let Some(initialization::Storage::Heap(records)) = header.clips.take()
                            else {
                                unreachable!("validated heap clip storage");
                            };
                            destructor.heap_records = Some(records.len());
                        }
                    }
                    destructor.phase = Phase::ReferenceList;
                }
                Phase::ReferenceList => {
                    let header = self.headers.get(&id).context("destroyed script absent")?;
                    match header.word(0x78) {
                        Some(0) => destructor.phase = Phase::Free,
                        Some(_) => return Ok(Step::Blocked(ChildDependency::ChildReferenceList)),
                        None => return Ok(Step::Blocked(ChildDependency::ChildState)),
                    }
                }
                Phase::Free => {
                    self.headers
                        .remove(&id)
                        .context("destroyed script absent")?;
                    destructor.phase = Phase::Returned;
                }
                Phase::Returned => {
                    return Ok(Step::Returned(ChildReceipt {
                        node: id,
                        heap_records: destructor.heap_records,
                    }));
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "destroy/tests/fixtures.rs"]
pub(crate) mod fixtures;
#[cfg(test)]
#[path = "destroy/tests.rs"]
mod tests;
