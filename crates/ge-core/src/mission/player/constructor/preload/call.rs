use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallStage {
    Remap,
    RawFlag,
    RawKey,
    RawIndices,
    EntryScan,
    RawOccupied,
    RawWeapon,
    RawMapping,
    EntryKey,
    Invalidate,
    Reference,
    AwaitingTemporary,
    Returned,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnReason {
    VariantUnavailable,
    RawFull,
    EntriesFull,
    ExistingEntry,
}

#[derive(Debug)]
pub struct Call {
    pub(super) request: Request,
    pub(super) stage: CallStage,
    pub(super) mapping: Option<Mapping>,
    pub(super) raw_cursor: u8,
    pub(super) raw_flag: Option<u8>,
    pub(super) raw_flags_read: usize,
    pub(super) raw_keys_read: usize,
    pub(super) raw_index: Option<u8>,
    pub(super) raw_indices_written: bool,
    pub(super) entry_cursor: u8,
    pub(super) entry_keys_read: usize,
    pub(super) entry_index: Option<u8>,
    pub(super) occupied_written: bool,
    pub(super) weapon_written: bool,
    pub(super) mapping_written: bool,
    pub(super) selected_key: Option<u32>,
    pub(super) invalidated: bool,
    pub(super) extraction_returned: bool,
    pub(super) reference: Option<u32>,
    pub(super) return_word: Option<u32>,
    pub(super) return_reason: Option<ReturnReason>,
}
impl Call {
    pub fn begin(request: Request) -> Result<Self> {
        ensure!(
            request.variant < 3 && request.receiver.player() == request.player,
            "preload call variant or receiver ownership differs"
        );
        Ok(Self {
            request,
            stage: CallStage::Remap,
            mapping: None,
            raw_cursor: 0,
            raw_flag: None,
            raw_flags_read: 0,
            raw_keys_read: 0,
            raw_index: None,
            raw_indices_written: false,
            entry_cursor: 0,
            entry_keys_read: 0,
            entry_index: None,
            occupied_written: false,
            weapon_written: false,
            mapping_written: false,
            selected_key: None,
            invalidated: false,
            extraction_returned: false,
            reference: None,
            return_word: None,
            return_reason: None,
        })
    }
    fn returned(&mut self, word: u32, reason: ReturnReason) {
        self.return_word = Some(word);
        self.return_reason = Some(reason);
        self.stage = CallStage::Returned;
    }
    pub fn advance(
        &mut self,
        lease: Lease,
        shared: &mut Shared,
        caches: &mut transform_cache::Owner,
        source: &mut impl Source,
    ) -> Result<()> {
        ensure!(
            lease.player() == self.request.player
                && shared.reset_stage(lease)? == ResetStage::Complete,
            "preload call has a stale or incomplete constructor lease"
        );
        loop {
            match self.stage {
                CallStage::Remap => {
                    let mapped = source.remap(self.request)?;
                    self.mapping = Some(mapped);
                    if !mapped.accepted && self.request.variant != 0 {
                        self.returned(0, ReturnReason::VariantUnavailable);
                        return Ok(());
                    }
                    self.stage = CallStage::RawFlag;
                }
                CallStage::RawFlag => {
                    if self.raw_cursor == 32 {
                        self.returned(0, ReturnReason::RawFull);
                        return Ok(());
                    }
                    let flag = shared.preload_raw_flag(lease, self.raw_cursor)?;
                    self.raw_flag = Some(flag);
                    self.raw_flags_read += 1;
                    if flag == 0 {
                        self.raw_index = Some(self.raw_cursor);
                        self.stage = CallStage::RawIndices;
                    } else {
                        self.stage = CallStage::RawKey;
                    }
                }
                CallStage::RawKey => {
                    let key = shared.preload_raw_key(lease, self.raw_cursor)?;
                    self.raw_keys_read += 1;
                    if key == self.request.weapon {
                        self.raw_index = Some(self.raw_cursor);
                        self.stage = CallStage::EntryScan;
                    } else {
                        self.raw_cursor += 1;
                        self.stage = CallStage::RawFlag;
                    }
                }
                CallStage::RawIndices => {
                    shared.initialize_preload_raw_indices(
                        lease,
                        self.raw_index.context("preload raw index absent")?,
                    )?;
                    self.raw_indices_written = true;
                    self.stage = CallStage::EntryScan;
                }
                CallStage::EntryScan => {
                    if self.entry_cursor == 34 {
                        self.returned(0, ReturnReason::EntriesFull);
                        return Ok(());
                    }
                    let key = shared.preload_entry_key(lease, self.entry_cursor)?;
                    self.entry_keys_read += 1;
                    if key == 0 || key == self.mapping.context("preload mapping absent")?.output {
                        self.entry_index = Some(self.entry_cursor);
                        self.stage = CallStage::RawOccupied;
                    } else {
                        self.entry_cursor += 1;
                    }
                }
                CallStage::RawOccupied => {
                    shared.store_preload_raw_occupied(
                        lease,
                        self.raw_index.context("preload raw index absent")?,
                    )?;
                    self.occupied_written = true;
                    self.stage = CallStage::RawWeapon;
                }
                CallStage::RawWeapon => {
                    shared.store_preload_raw_weapon(
                        lease,
                        self.raw_index.context("preload raw index absent")?,
                        self.request.weapon,
                    )?;
                    self.weapon_written = true;
                    self.stage = CallStage::RawMapping;
                }
                CallStage::RawMapping => {
                    shared.store_preload_raw_mapping(
                        lease,
                        self.raw_index.context("preload raw index absent")?,
                        self.request.alternate,
                        self.request.variant,
                        self.entry_index.context("preload entry index absent")?,
                    )?;
                    self.mapping_written = true;
                    self.stage = CallStage::EntryKey;
                }
                CallStage::EntryKey => {
                    let key = shared.preload_entry_key(
                        lease,
                        self.entry_index.context("preload entry index absent")?,
                    )?;
                    self.selected_key = Some(key);
                    self.entry_keys_read += 1;
                    if key != 0 {
                        self.returned(1, ReturnReason::ExistingEntry);
                        return Ok(());
                    }
                    self.stage = CallStage::Invalidate;
                }
                CallStage::Invalidate => {
                    caches.invalidate_selected()?;
                    self.invalidated = true;
                    self.stage = CallStage::Reference;
                }
                CallStage::Reference => {
                    let reference = source.extract_reference(
                        self.request.receiver,
                        self.mapping.context("preload mapping absent")?.output,
                    )?;
                    if let Some(reference) = reference {
                        ensure!(
                            reference != 0 && (reference >> 24) & 0x7f == 4,
                            "preload extracted reference is not class four"
                        );
                    }
                    self.reference = reference;
                    self.extraction_returned = true;
                    // A known zero reference still requires a new temporary
                    // animator and its actual bind/evaluation/destructor.
                    self.stage = CallStage::AwaitingTemporary;
                }
                CallStage::AwaitingTemporary | CallStage::Returned => return Ok(()),
            }
        }
    }
    pub fn request(&self) -> Request {
        self.request
    }
    pub fn stage(&self) -> CallStage {
        self.stage
    }
    pub fn mapping(&self) -> Option<Mapping> {
        self.mapping
    }
    pub fn raw_flags_read(&self) -> usize {
        self.raw_flags_read
    }
    pub fn raw_keys_read(&self) -> usize {
        self.raw_keys_read
    }
    pub fn raw_index(&self) -> Option<u8> {
        self.raw_index
    }
    pub fn raw_indices_written(&self) -> bool {
        self.raw_indices_written
    }
    pub fn entry_keys_read(&self) -> usize {
        self.entry_keys_read
    }
    pub fn entry_index(&self) -> Option<u8> {
        self.entry_index
    }
    pub fn occupied_written(&self) -> bool {
        self.occupied_written
    }
    pub fn weapon_written(&self) -> bool {
        self.weapon_written
    }
    pub fn mapping_written(&self) -> bool {
        self.mapping_written
    }
    pub fn selected_key(&self) -> Option<u32> {
        self.selected_key
    }
    pub fn invalidated(&self) -> bool {
        self.invalidated
    }
    pub fn extraction_returned(&self) -> bool {
        self.extraction_returned
    }
    pub fn reference(&self) -> Option<u32> {
        self.reference
    }
    pub fn return_word(&self) -> Option<u32> {
        self.return_word
    }
    pub fn return_reason(&self) -> Option<ReturnReason> {
        self.return_reason
    }
}
