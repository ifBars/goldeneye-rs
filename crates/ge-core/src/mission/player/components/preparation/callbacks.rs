use super::{Identity, Item, ItemId, Preparation, Stage};
use crate::mission::player::{
    base_handler::{AnimationInterface, Owner},
    components::callbacks::Table,
};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

#[derive(Debug)]
pub struct ThirdPersonReturn {
    component: Identity,
    item: ItemId,
    interface: AnimationInterface,
    table_owner: u64,
}
impl Preparation {
    /// Source tail stores callback targets without invoking them. Pose
    /// postprocessing and event scheduling remain separate reached operations.
    pub fn finish_third_person(
        &mut self,
        base: &Owner,
        item: &mut Item,
        table: Arc<Table>,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingCallbacks,
            "TP callback binding is out of source order"
        );
        let animator = self
            .animator
            .as_mut()
            .context("TP callback animator is absent")?;
        let postbind = self
            .postbind
            .as_ref()
            .context("TP visual postbind has not returned")?;
        ensure!(
            postbind.component() == animator.identity()
                && postbind.item() == item.id()
                && base.player() == animator.player()
                && base.item() == Some(item.id())
                && animator.item() == Some(item.id())
                && item.components() == [animator.identity().id()],
            "TP callback item/component/player ownership differs"
        );
        let interface = base.animation_interface()?;
        self.stage = Stage::BindingCallbacks;
        let result = (|| {
            // Item +16c precedes animator +118. The following skeletal +8
            // assignment is a known target; no callback executes on this tail.
            item.bind_animation_interface(interface)?;
            let table_owner = table.owner();
            animator.bind_callback_table(table)?;
            animator.assign_player_pose_callback()?;
            self.third_person_return = Some(ThirdPersonReturn {
                component: animator.identity(),
                item: item.id(),
                interface,
                table_owner,
            });
            Ok(())
        })();
        self.stage = if result.is_ok() {
            Stage::AwaitingFirstPerson
        } else {
            Stage::Failed
        };
        result
    }
    pub fn third_person_return(&self) -> Option<&ThirdPersonReturn> {
        self.third_person_return.as_ref()
    }
}
impl ThirdPersonReturn {
    pub fn component(&self) -> Identity {
        self.component
    }
    pub fn item(&self) -> ItemId {
        self.item
    }
    pub fn interface(&self) -> AnimationInterface {
        self.interface
    }
    pub fn table_owner(&self) -> u64 {
        self.table_owner
    }
    pub fn visit_state_words(&self, mut emit: impl FnMut(u32)) {
        emit(1);
        self.component.visit_state_words(&mut emit);
        for id in [self.item.world().get(), self.item.get(), self.table_owner] {
            emit((id >> 32) as u32);
            emit(id as u32);
        }
        self.interface.visit_state_words(emit);
    }
}
