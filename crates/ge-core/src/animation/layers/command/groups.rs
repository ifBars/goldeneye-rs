use super::*;
impl Pending {
    pub(super) fn prepare_groups(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        slot_override: SlotOverride,
        start: usize,
    ) -> Result<()> {
        let count = self.group_count.context("set group count absent")?;
        for ordinal in start..count {
            charge(&mut layers.work.groups, layers.budget.groups, "group")?;
            let source_slot = source.group_slot()?;
            let slot = if slot_override.enabled_raw != 0 {
                slot_override.slot_raw
            } else {
                u32::from(source_slot)
            };
            self.groups.try_reserve(1)?;
            ensure!(
                self.groups.len() == ordinal,
                "continued group ordinal differs"
            );
            self.groups.push(Group {
                source_slot,
                effective_slot: slot,
                weight_write: None,
                child_count: None,
            });
            let slot = usize::try_from(slot)?;
            ensure!(slot < 4, "unsupported reached effective animation slot");
            let weight = if slot == 0 {
                Some(1f32.to_bits())
            } else if f32::from_bits(source.group_weight()?) == 0.0 {
                Some(0)
            } else {
                None
            };
            if let Some(weight) = weight {
                layers.slots[slot].weight_bits = weight;
                self.groups[ordinal].weight_write = Some(weight);
            }
            let children = source.child_count()?;
            self.groups[ordinal].child_count = Some(children);
            ensure!(
                children >= 0,
                "unsupported negative reached animation child count"
            );
            if children > 0 {
                return self.request_child(source, layers, ordinal, 0, slot as u8);
            }
            source.next_group(children as u16)?;
        }
        self.stage = if self.flags_or & 0x20 != 0 {
            Stage::Returned
        } else {
            Stage::CleanupPending
        };
        Ok(())
    }
    pub(super) fn request_child(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        group: usize,
        ordinal: usize,
        slot: u8,
    ) -> Result<()> {
        charge(&mut layers.work.children, layers.budget.children, "child")?;
        let divisor = self.divisor.context("command divisor unknown")?;
        let child_divisor = if self.opcode == 0x0c00_0003 {
            divisor.wrapping_add(u32::from(source.child_divisor(ordinal)?))
        } else if divisor != 0 {
            divisor
        } else {
            u32::from(source.child_divisor(ordinal)?)
        };
        let flags = self.flags.context("command flags unknown")?;
        let effective_flags = if flags != 0 {
            flags
        } else {
            source.child_flags(ordinal)?
        };
        let reference = source.child_reference(ordinal)?;
        self.flags_or |= effective_flags;
        self.child = Some(ChildRequest {
            group,
            ordinal,
            slot,
            reference,
            effective_flags,
            divisor: child_divisor,
        });
        self.stage = Stage::ChildFactoryPending;
        Ok(())
    }
}
