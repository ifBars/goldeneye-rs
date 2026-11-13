//! Secondary intent shares selection machinery, but owns a separate history pair.
use super::*;

pub(super) fn enter(
    base: &mut base_handler::Owner,
    reset: &PendingReset,
    tracks: bool,
) -> Result<u32> {
    ensure!(
        base.player() == reset.player(),
        "secondary setter entry player differs"
    );
    enter_source(
        tracks,
        || base.begin_secondary_mode_update(),
        |offset| reset.word(offset),
    )
}

fn enter_source(
    tracks: bool,
    clear: impl FnOnce() -> Result<()>,
    previous: impl FnOnce(u16) -> Option<u32>,
) -> Result<u32> {
    let offset = if tracks {
        clear()?;
        0x718
    } else {
        0x70c
    };
    previous(offset).context("secondary setter previous mode unknown")
}

pub(super) struct Inputs {
    pub player: PlayerId,
    pub generic: u32,
    pub mapped: u32,
    pub tracks: bool,
}

pub(super) fn commit(
    inputs: Inputs,
    reset: &mut PendingReset,
    trace: &mut SetterReturn,
) -> Result<()> {
    finish(&inputs, trace, |offset, word| {
        reset.store_action_history(inputs.player, offset, word)
    })
}

fn finish(
    inputs: &Inputs,
    trace: &mut SetterReturn,
    mut history: impl FnMut(u16, u32) -> Result<()>,
) -> Result<()> {
    if !inputs.tracks || trace.return_word == 0 {
        return Ok(());
    }
    history(0x710, inputs.generic)?;
    history(0x718, inputs.mapped)?;
    trace.history_updated = true;
    trace.primary_changed = None;
    Ok(())
}

#[cfg(test)]
#[path = "secondary_setter/tests.rs"]
mod tests;
