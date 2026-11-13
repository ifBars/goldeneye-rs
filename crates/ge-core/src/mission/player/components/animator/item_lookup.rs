use super::*;
use crate::mission::player::{identity::ComponentId, item::Item};
use anyhow::Context;

impl super::super::Preparation {
    pub(crate) fn item_animator(
        &self,
        item: &Item,
        player: PlayerId,
        id: ComponentId,
    ) -> Result<Identity> {
        ensure!(
            item.handler() == Some(player),
            "item animator handler differs"
        );
        let owner = self
            .animator()
            .into_iter()
            .chain(self.first_person().map(|fp| fp.animator()))
            .find(|owner| owner.identity().id() == id)
            .context("item component owner provider absent")?;
        ensure!(
            owner.player() == player && owner.item() == Some(item.id()),
            "item animator identity/backlink differs"
        );
        Ok(owner.identity())
    }

    /// Source-ordered lookup over the actual item's component headers. A known
    /// NULL wrapper skips that node; an unknown typed owner remains an error.
    pub fn item_animator_by_file(
        &self,
        item: &Item,
        player: PlayerId,
        file: u32,
    ) -> Result<Option<Identity>> {
        for id in item.components() {
            let identity = self.item_animator(item, player, *id)?;
            if self
                .mode_animator(identity)?
                .component()
                .is_some_and(|binding| binding.file == file)
            {
                return Ok(Some(identity));
            }
        }
        Ok(None)
    }
}
