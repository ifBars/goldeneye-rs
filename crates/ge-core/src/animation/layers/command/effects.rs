use super::*;
use crate::animation::{
    controls,
    nodes::{Ids, Kind},
    script_child,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectStage {
    AwaitingFactory,
    Copy,
    Wrapper,
    Inserted,
    Scalars,
    Complete,
}
#[derive(Debug)]
pub struct Effects {
    pub request: ChildRequest,
    pub node: super::super::NodeId,
    pub stage: EffectStage,
    pub copied_head: Option<super::super::NodeId>,
    pub copied_bits: Option<u32>,
    pub inserted: bool,
    pub post_word: Option<u32>,
}
impl Pending {
    pub fn effects(&self) -> &[Effects] {
        &self.effects
    }
    pub fn last_completed(&self) -> Option<&Effects> {
        self.effects
            .iter()
            .rev()
            .find(|e| e.stage == EffectStage::Complete)
    }
    pub(crate) fn finish_script_child(
        &mut self,
        layers: &mut State,
        ids: &mut Ids,
        controls: &controls::Store,
        scripts: &mut script_child::Store,
        source: &mut impl Source,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::ChildFactoryPending && !layers.failed,
            "child effects are out of source order"
        );
        let result = self.finish_script_child_inner(layers, ids, controls, scripts, source);
        if result.is_err() {
            self.fail_factory(layers);
        }
        result
    }
    fn finish_script_child_inner(
        &mut self,
        layers: &mut State,
        ids: &mut Ids,
        controls: &controls::Store,
        scripts: &mut script_child::Store,
        source: &mut impl Source,
    ) -> Result<()> {
        let request = self.child.context("child effect request absent")?;
        let id = self
            .factory_node
            .context("child effect allocation absent")?;
        let effect = self
            .effects
            .last_mut()
            .context("child effect history absent")?;
        ensure!(
            effect.node == id && effect.stage == EffectStage::AwaitingFactory,
            "child effects differ or replay"
        );
        let h = scripts
            .get(id)
            .context("script child effect owner absent")?;
        ensure!(
            h.owner() == ids.owner() && h.initializer_returned() && h.slot() == Some(request.slot),
            "child factory has not returned for this owner/slot"
        );
        effect.stage = EffectStage::Copy;
        let slot = usize::from(request.slot);
        if request.effective_flags & 4 != 0
            && let Some(head) = layers.slots[slot].nodes.first().copied()
        {
            let bits = match ids.kind(head) {
                Some(Kind::Control) => {
                    let control = controls.get(head).context("copy source control absent")?;
                    ensure!(
                        control.component() == ids.owner(),
                        "copy source control owner differs"
                    );
                    control.position_bits()
                }
                Some(Kind::Script) => scripts
                    .get(head)
                    .context("copy source script absent")?
                    .word(0x28)
                    .context("copy source position unknown")?,
                _ => anyhow::bail!("copy source node kind provider absent"),
            };
            effect.copied_head = Some(head);
            scripts.copy_parent_phase(id, bits)?;
            effect.copied_bits = Some(bits);
        }
        effect.stage = EffectStage::Wrapper;
        if request.effective_flags & 8 != 0 {
            self.stage = Stage::ChildWrapperPending;
            return Ok(());
        }
        // Source raises the active count BEFORE the intrusive head insertion.
        layers.active_count = layers.active_count.max(request.slot + 1);
        layers.slots[slot]
            .nodes
            .try_reserve(1)
            .context("retaining script layer member")?;
        ids.link_head(request.slot, id, layers.slots[slot].nodes.first().copied())?;
        layers.slots[slot].nodes.insert(0, id);
        effect.inserted = true;
        effect.stage = EffectStage::Inserted;
        scripts.initialize_parent_scalars(id, request.divisor)?;
        effect.stage = EffectStage::Scalars;
        let word = source.child_word(request.ordinal)?;
        scripts.store_parent_word(id, word)?;
        effect.post_word = Some(word);
        effect.stage = EffectStage::Complete;
        self.stage = Stage::ChildContinuationPending;
        Ok(())
    }
    pub fn advance_completed_child(
        &mut self,
        source: &mut impl Source,
        layers: &mut State,
        slot_override: SlotOverride,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::ChildContinuationPending && !layers.failed,
            "child continuation differs or replays"
        );
        let result = (|| {
            let old = self.child.context("completed child request absent")?;
            ensure!(
                self.effects
                    .last()
                    .is_some_and(|e| e.stage == EffectStage::Complete),
                "child continuation precedes post-word return"
            );
            let count = source.child_count()?;
            self.groups[old.group].child_count = Some(count);
            ensure!(count >= 0, "unsupported negative continued child count");
            self.factory_node = None;
            self.factory_started = false;
            self.child = None;
            let next = old
                .ordinal
                .checked_add(1)
                .context("child ordinal overflow")?;
            if next < usize::try_from(count)? {
                return self.request_child(source, layers, old.group, next, old.slot);
            }
            source.next_group(count as u16)?;
            self.prepare_groups(source, layers, slot_override, old.group + 1)
        })();
        if result.is_err() {
            self.fail_factory(layers);
        }
        result
    }
}
