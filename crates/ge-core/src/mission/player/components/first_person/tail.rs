use super::*;
use crate::mission::player::{
    base_handler::Owner,
    components::callbacks::Table,
    item::{Item, Registration},
};
use std::sync::Arc;

impl FirstPerson {
    pub(in super::super) fn finish_helper(
        &mut self,
        tp: &mut Animator,
        base: &Owner,
        item: &mut Item,
        table: Arc<Table>,
        limit: usize,
    ) -> Result<()> {
        ensure!(
            self.stage == FirstPersonStage::AwaitingAttachment && self.refresh.is_some(),
            "FP helper tail requires actual zero-time bind return"
        );
        ensure!(
            base.player() == self.animator.player()
                && base.item() == Some(item.id())
                && item.handler() == Some(base.player())
                && item.registration() == Registration::Complete
                && item.components() == [tp.identity().id()]
                && tp.identity() != self.animator.identity()
                && tp
                    .callback_table()
                    .is_some_and(|bound| Arc::ptr_eq(bound, &table)),
            "FP helper tail ownership or source list prefix differs"
        );
        let interface = base.animation_interface()?;
        self.stage = FirstPersonStage::Attaching;
        let result = (|| {
            self.animator.append_after(tp, item, limit)?;
            // Flag1 skips TP visual postbind. Shared helper tail is otherwise
            // unchanged: four headers, embedded receiver, shared table, pose target.
            self.animator.configure_headers();
            item.bind_animation_interface(interface)?;
            self.animator.bind_callback_table(table)?;
            self.animator.assign_player_pose_callback()?;
            Ok(())
        })();
        self.stage = if result.is_ok() {
            FirstPersonStage::Returned
        } else {
            FirstPersonStage::Failed
        };
        result
    }
}
