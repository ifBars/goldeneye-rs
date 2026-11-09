use super::{Host, NodeId, SlotOverride, State};
use crate::animation::mode::{CommandPlan, SlotInputs, StartSetPlan, WeightRequest};
use anyhow::{Context, Result, ensure};

impl State {
    pub fn execute(
        &mut self,
        host: &mut impl Host,
        commands: &[CommandPlan],
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            !self.failed,
            "animation layer state is failed; discard the attempt"
        );
        let result = self.execute_inner(host, commands, slot_override);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn execute_inner(
        &mut self,
        host: &mut impl Host,
        commands: &[CommandPlan],
        slot_override: SlotOverride,
    ) -> Result<()> {
        for command in commands {
            charge(&mut self.work.commands, self.budget.commands, "command")?;
            match command {
                CommandPlan::MaskOperation { mask_raw, .. } => self.mask(host, *mask_raw)?,
                CommandPlan::StartSet(set) => self.start_set(host, set, slot_override)?,
            }
        }
        Ok(())
    }

    fn allocation_request(&mut self) -> Result<()> {
        charge(&mut self.work.nodes, self.budget.nodes, "node factory")
    }
    fn register_node(&mut self, id: NodeId) -> Result<NodeId> {
        ensure!(
            self.identities.insert(id),
            "animation factory returned a reused node identity"
        );
        self.created
            .try_reserve(1)
            .context("retaining animation node identity")?;
        self.created.push(id);
        Ok(id)
    }
    fn insert(&mut self, slot: usize, id: NodeId) -> Result<()> {
        self.slots[slot]
            .nodes
            .try_reserve(1)
            .context("retaining animation layer membership")?;
        self.slots[slot].nodes.insert(0, id);
        self.active_count = self.active_count.max(slot as u8 + 1);
        Ok(())
    }
    fn clear(&mut self, host: &mut impl Host, slot: usize) -> Result<()> {
        // Source unlinks the head before destroying it, leaving flags, weight
        // and active count intact. A failed destructor latches partial failure.
        while !self.slots[slot].nodes.is_empty() {
            let id = self.slots[slot].nodes.remove(0);
            host.destroy_node(id)?;
        }
        Ok(())
    }
    fn mask(&mut self, host: &mut impl Host, mask: u16) -> Result<()> {
        let control = if mask != 0 {
            self.allocation_request()?;
            let id = host
                .create_control()?
                .context("animation control factory returned no node")?;
            Some(self.register_node(id)?)
        } else {
            None
        };
        let selected = mask & u16::from(self.selection_mask);
        let mut inserted_original = false;
        let mut slot = 0usize;
        // Deliberately current active count, not highest mask bit/four slots.
        while slot < usize::from(self.active_count) {
            if self.clear_mask & (1 << slot) != 0 {
                self.clear(host, slot)?;
            }
            if selected & (1 << slot) != 0 {
                let source = control.context("selected animation mask has no control")?;
                let id = if !inserted_original {
                    inserted_original = true;
                    source
                } else {
                    self.allocation_request()?;
                    let clone = host
                        .clone_control(source)?
                        .context("animation control clone returned no node")?;
                    self.register_node(clone)?
                };
                self.insert(slot, id)?;
            }
            slot += 1;
        }
        Ok(())
    }
    fn start_set(
        &mut self,
        host: &mut impl Host,
        set: &StartSetPlan,
        slot_override: SlotOverride,
    ) -> Result<()> {
        let mut flags_or = 0u16;
        for group in &set.groups {
            charge(&mut self.work.groups, self.budget.groups, "group")?;
            let request = group.layer_request(SlotInputs {
                override_enabled_raw: slot_override.enabled_raw,
                override_slot_raw: slot_override.slot_raw,
                current_weight_bits: std::array::from_fn(|i| self.slots[i].weight_bits),
            })?;
            let slot = request.effective_slot as usize;
            self.slots[slot].weight_bits = match request.weight {
                WeightRequest::Assign(bits) | WeightRequest::Preserve(bits) => bits,
            };
            for child in &group.children {
                charge(&mut self.work.children, self.budget.children, "child")?;
                flags_or |= child.effective_flags;
                self.allocation_request()?;
                let Some(id) = host.create_child(slot as u8, child)? else {
                    continue;
                };
                let id = self.register_node(id)?;
                if child.effective_flags & 4 != 0
                    && let Some(&head) = self.slots[slot].nodes.first()
                {
                    host.copy_head(id, head)?;
                }
                ensure!(
                    child.effective_flags & 8 == 0,
                    "unsupported reached animation wrapper flag 0x8"
                );
                self.insert(slot, id)?;
                host.initialize_child(id, child)?;
            }
        }
        // Use actual reached effective flags and original serialized slots.
        // An intent label alone cannot assert live clear-mask/slot-flag facts.
        if flags_or & 0x20 == 0 {
            for slot in 0..usize::from(self.active_count) {
                if !set
                    .groups
                    .iter()
                    .any(|group| usize::from(group.source_slot) == slot)
                    && self.slots[slot].flags_raw & 0x0800 == 0
                    && self.clear_mask & (1 << slot) != 0
                {
                    self.clear(host, slot)?;
                }
            }
        }
        Ok(())
    }
}

fn charge(used: &mut usize, limit: usize, label: &str) -> Result<()> {
    let next = used
        .checked_add(1)
        .context("animation layer work overflow")?;
    ensure!(next <= limit, "animation layer {label} budget exhausted");
    *used = next;
    Ok(())
}
