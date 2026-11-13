use super::*;

/// Asset words stay runtime-only. Public evidence exposes counts and digests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskValue {
    pub directory_index: usize,
    pub attribute_present: bool,
    words: [u32; 4],
}
impl MaskValue {
    pub fn new(directory_index: usize, attribute_present: bool, words: [u32; 4]) -> Self {
        Self {
            directory_index,
            attribute_present,
            words,
        }
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(self.directory_index as u32);
        emit(u32::from(self.attribute_present));
        for word in self.words {
            emit(word);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskLookup {
    MissingDescriptor,
    MissingBody,
    Pending,
    Resident(MaskValue),
}
pub trait MaskSource {
    /// Resolve and convert this body before the caller requests the next hash.
    /// Not resident is pending, never descriptor/body absence.
    fn mask(&mut self, receiver: Identity, reference: u32) -> Result<MaskLookup>;
}

#[derive(Debug)]
pub struct MaskHelper {
    pub(super) target: Target,
    pub(super) receiver: Option<Identity>,
    pub(super) searched: bool,
    lookups: [Option<MaskLookup>; 2],
    applied_slots: Vec<u8>,
    pub(super) returned: bool,
}
impl MaskHelper {
    pub(super) fn new(target: Target) -> Self {
        Self {
            target,
            receiver: None,
            searched: false,
            lookups: [None; 2],
            applied_slots: Vec::new(),
            returned: false,
        }
    }
    pub fn target(&self) -> Target {
        self.target
    }
    pub fn receiver(&self) -> Option<Identity> {
        self.receiver
    }
    pub fn searched(&self) -> bool {
        self.searched
    }
    pub fn lookups(&self) -> &[Option<MaskLookup>; 2] {
        &self.lookups
    }
    pub fn applied_slots(&self) -> &[u8] {
        &self.applied_slots
    }
    pub fn returned(&self) -> bool {
        self.returned
    }
    pub(super) fn advance(
        &mut self,
        state: u8,
        source: &mut impl MaskSource,
        mut apply: impl FnMut(Identity, u8, [u32; 4]) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            !self.returned && self.searched,
            "mask helper lookup order differs or replays"
        );
        let Some(receiver) = self.receiver else {
            self.returned = true;
            return Ok(());
        };
        for (index, reference) in [0x0300_0147, 0x0300_0148].into_iter().enumerate() {
            if matches!(self.lookups[index], None | Some(MaskLookup::Pending)) {
                self.lookups[index] = Some(source.mask(receiver, reference)?);
            }
            match self.lookups[index].expect("retained mask lookup") {
                MaskLookup::MissingDescriptor | MaskLookup::MissingBody => {
                    self.returned = true;
                    return Ok(());
                }
                MaskLookup::Pending => return Ok(()),
                MaskLookup::Resident(_) => {}
            }
        }
        let [Some(MaskLookup::Resident(a)), Some(MaskLookup::Resident(b))] = self.lookups else {
            unreachable!("both converted masks")
        };
        for (slot, words) in mapping(self.target, state, a.words, b.words) {
            apply(receiver, slot, words)?;
            self.applied_slots.push(slot);
        }
        self.returned = true;
        Ok(())
    }
    pub fn visit_state_words(&self, emit: &mut impl FnMut(u32)) {
        emit(self.target as u32);
        emit(u32::from(self.searched));
        emit(u32::from(self.receiver.is_some()));
        if let Some(receiver) = self.receiver {
            receiver.visit_state_words(&mut *emit);
        }
        for lookup in self.lookups {
            emit(match lookup {
                None => 0,
                Some(MaskLookup::MissingDescriptor) => 1,
                Some(MaskLookup::MissingBody) => 2,
                Some(MaskLookup::Pending) => 3,
                Some(MaskLookup::Resident(_)) => 4,
            });
            if let Some(MaskLookup::Resident(value)) = lookup {
                value.visit_state_words(emit);
            }
        }
        emit(self.applied_slots.len() as u32);
        for slot in &self.applied_slots {
            emit(u32::from(*slot));
        }
        emit(u32::from(self.returned));
    }
}

fn mapping(target: Target, state: u8, a: [u32; 4], b: [u32; 4]) -> Vec<(u8, [u32; 4])> {
    match target {
        Target::FirstPerson => {
            let mut writes = match state {
                0 => vec![(0, a), (1, a)],
                1..=3 => vec![(0, a), (1, b)],
                _ => vec![],
            };
            writes.push((2, a));
            writes
        }
        Target::ThirdPerson => match state {
            0 => vec![(0, a), (1, a), (2, a), (3, a)],
            1 => vec![(0, a), (1, a), (2, b), (3, b)],
            2 => vec![
                (0, a),
                (1, std::array::from_fn(|i| a[i] ^ b[i])),
                (2, b),
                (3, b),
            ],
            3 => vec![(0, a), (1, a), (2, b), (3, [0; 4])],
            _ => vec![],
        },
        Target::ThirdPersonSecondary => unreachable!("secondary target has no mask helper"),
    }
}
