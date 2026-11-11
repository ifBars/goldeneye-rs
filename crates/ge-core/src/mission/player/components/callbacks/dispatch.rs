use super::Table;
use crate::mission::player::{
    base_handler::{AnimationInterface, Owner},
    components::Animator,
    world::World,
};
use anyhow::{Context, Result, ensure};

#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub kind: u8,
    pub file: u32,
    pub parameter: u32,
    pub control_channel: Option<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCall {
    pub receiver: AnimationInterface,
    pub kind: u8,
    pub file: u32,
    pub parameter: u32,
    pub control_channel: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventOutcome {
    pub value: i32,
    pub call: Option<EventCall>,
}

fn receiver(
    animator: &Animator,
    world: &World,
    handler: Option<&Owner>,
) -> Result<Option<AnimationInterface>> {
    // The reached animator is directly attached to an actual EXItem. No opaque
    // parent chain or unbound owner is substituted with a native NULL result.
    let root = world.item(
        animator
            .item()
            .context("callback animator item is unbound")?,
    )?;
    let Some(player) = root.handler() else {
        return Ok(None);
    };
    let handler = handler.context("callback handler owner provider is absent")?;
    ensure!(
        handler.player() == player,
        "callback handler identity differs"
    );
    let Some(item) = handler.item() else {
        return Ok(None);
    };
    let Some(interface) = world.item(item)?.animation_interface() else {
        return Ok(None);
    };
    ensure!(
        interface == handler.animation_interface()?,
        "callback receiver requires its actual target owner"
    );
    Ok(Some(interface))
}
impl Table {
    fn check_bound(&self, animator: &Animator) -> Result<()> {
        ensure!(
            animator
                .callback_table()
                .is_some_and(|table| std::ptr::eq(self, table.as_ref())),
            "callback invocation table differs from the animator's actual binding"
        );
        Ok(())
    }
    pub fn event(
        &self,
        animator: &Animator,
        world: &World,
        handler: Option<&Owner>,
        event: Event,
        mut read_base_byte: impl FnMut() -> Result<u8>,
    ) -> Result<EventOutcome> {
        self.check_bound(animator)?;
        let Some(receiver) = receiver(animator, world, handler)? else {
            return Ok(EventOutcome {
                value: 0,
                call: None,
            });
        };
        // Source resolves the sentinel before switching on kind, even for an
        // otherwise ignored kind. The component pair is already owner-bound.
        let file = if event.file == u32::MAX {
            animator
                .component()
                .context("callback component file is unbound")?
                .file
        } else {
            event.file
        };
        if !matches!(event.kind, 3..=5) {
            return Ok(EventOutcome {
                value: 0,
                call: None,
            });
        }
        let control_channel = event
            .control_channel
            .context("callback control channel is unknown")?;
        let call = EventCall {
            receiver,
            kind: event.kind,
            file,
            parameter: event.parameter,
            control_channel,
        };
        // PlayerNew slots +8/+10 are literal zero. Slot +18 reads the SAME
        // persistent reset byte, then the dispatcher sign-extends its low byte.
        let value = if event.kind == 5 {
            i32::from(read_base_byte()? as i8)
        } else {
            0
        };
        Ok(EventOutcome {
            value,
            call: Some(call),
        })
    }
    pub fn acknowledge(
        &self,
        animator: &Animator,
        world: &World,
        handler: Option<&Owner>,
        token: &mut u32,
        mut read_base_byte: impl FnMut() -> Result<u8>,
    ) -> Result<bool> {
        self.check_bound(animator)?;
        if receiver(animator, world, handler)?.is_none() {
            return Ok(false);
        }
        // The nonzero shared-byte branch acknowledges immediately. Otherwise
        // the source checks for the Sound subclass. This API accepts the real
        // TP/FP Animator class, whose ancestry does not include Sound.
        let accepted = read_base_byte()? != 0;
        if accepted {
            *token = u32::MAX;
        }
        Ok(accepted)
    }
}
