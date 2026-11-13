use super::*;
use crate::animation::{layers::NodeId, nodes::Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetterReturn {
    pub return_word: u32,
    pub history_updated: bool,
    pub primary_changed: Option<bool>,
}
trait Source {
    fn history(&mut self, offset: u16, word: u32) -> Result<()>;
    fn primary(&mut self) -> Result<(Option<NodeId>, u32)>;
}
struct Owners<'a> {
    player: PlayerId,
    reset: &'a mut PendingReset,
    preparation: &'a Preparation,
}
impl Source for Owners<'_> {
    fn history(&mut self, offset: u16, word: u32) -> Result<()> {
        self.reset.store_action_history(self.player, offset, word)
    }
    fn primary(&mut self) -> Result<(Option<NodeId>, u32)> {
        let owner = self
            .preparation
            .animator()
            .context("TP setter head owner absent")?;
        ensure!(
            owner.player() == self.player,
            "TP setter head player differs"
        );
        let head = owner.layers().context("TP setter layers absent")?.slots()[0]
            .nodes_head()
            .first()
            .copied();
        let word = if let Some(id) = head {
            match owner
                .node_ids()
                .kind(id)
                .context("TP setter head type absent")?
            {
                Kind::Control => {
                    ensure!(
                        owner.controls().get(id).is_some(),
                        "TP setter head control absent"
                    );
                    0
                }
                Kind::Script => owner
                    .script_children()
                    .get(id)
                    .context("TP setter head script absent")?
                    .word(0x10)
                    .context("TP setter head reference unknown")?,
            }
        } else {
            0
        };
        Ok((head, word))
    }
}
fn finish(
    source: &mut impl Source,
    trace: &mut SetterReturn,
    tracks: bool,
    saved: Option<(Option<NodeId>, u32)>,
    generic: u32,
    mapped: u32,
) -> Result<()> {
    // The untracked call and a zero selection return skip every history/head read.
    if !tracks || trace.return_word == 0 {
        return Ok(());
    }
    source.history(0x70c, generic)?;
    source.history(0x714, mapped)?;
    trace.history_updated = true;
    let current = source.primary()?;
    let changed = current != saved.context("TP setter saved primary comparison absent")?;
    trace.primary_changed = Some(changed);
    if changed {
        source.history(0x710, 0)?;
    }
    Ok(())
}
impl Pending {
    pub fn finish_third_person_setter(
        &mut self,
        reset: &mut PendingReset,
        preparation: &Preparation,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonSetterTail && reset.player() == self.player,
            "TP setter return owner/order differs"
        );
        let return_word = self
            .selector
            .as_ref()
            .context("TP setter cursor absent")?
            .return_word()
            .context("TP setter selection has not returned")?;
        let tracks = self
            .tracks_player_modes
            .context("TP setter qualification absent")?;
        let mapped = self
            .resolved
            .get(1)
            .context("TP setter mapping absent")?
            .output;
        let dispatch = self
            .dispatch
            .as_mut()
            .context("TP dispatch entry snapshot absent")?;
        ensure!(
            dispatch.setter.is_none() && dispatch.stage == DispatchStage::AwaitingSetter,
            "TP setter return replays"
        );
        dispatch.setter = Some(SetterReturn {
            return_word,
            history_updated: false,
            primary_changed: None,
        });
        let result = finish(
            &mut Owners {
                player: self.player,
                reset,
                preparation,
            },
            dispatch.setter.as_mut().expect("retained setter"),
            tracks,
            self.primary_head,
            self.generic,
            mapped,
        );
        self.stage = if result.is_ok() {
            Stage::AwaitingThirdPersonDispatchTail
        } else {
            Stage::ThirdPersonDispatchFailed
        };
        if result.is_err() {
            dispatch.stage = DispatchStage::Failed;
        }
        result
    }
}

#[cfg(test)]
#[path = "tests/setter.rs"]
mod tests;
