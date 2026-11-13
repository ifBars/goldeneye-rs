use super::*;
use crate::animation::layers::NodeId;
impl Pending {
    pub fn factory_node(&self) -> Option<NodeId> {
        self.factory_node
    }
    pub(crate) fn begin_factory(&mut self, layers: &mut State) -> Result<ChildRequest> {
        ensure!(
            self.stage == Stage::ChildFactoryPending && !self.factory_started && !layers.failed,
            "child factory cannot precede its request or replay"
        );
        charge(&mut layers.work.nodes, layers.budget.nodes, "node factory")?;
        self.factory_started = true;
        self.child.context("child factory request absent")
    }
    pub(crate) fn retain_factory_node(&mut self, layers: &mut State, id: NodeId) -> Result<()> {
        ensure!(
            self.factory_started && self.factory_node.is_none(),
            "child factory allocation cannot replay"
        );
        self.factory_node = Some(id);
        layers.register_node(id)?;
        self.effects.try_reserve(1)?;
        self.effects.push(Effects {
            request: self.child.context("factory child request absent")?,
            node: id,
            stage: EffectStage::AwaitingFactory,
            copied_head: None,
            copied_bits: None,
            inserted: false,
            post_word: None,
        });
        Ok(())
    }
    pub(crate) fn fail_factory(&mut self, layers: &mut State) {
        self.stage = Stage::Failed;
        layers.failed = true;
    }
}
