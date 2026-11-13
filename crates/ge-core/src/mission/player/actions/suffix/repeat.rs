use super::*;

/// Required next action in the fresh virtual callback. This is a request,
/// never proof that the second dispatch or its enclosing callback returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatRequest {
    pub generic: u32,
    pub parameter_bits: u32,
}

impl Pending {
    pub fn prepare_repeated_action(&mut self, reset: &PendingReset) -> Result<()> {
        ensure!(
            self.stage == Stage::Returned && reset.player() == self.player,
            "repeated action precedes first invocation return or owner differs"
        );
        let suffix = self.suffix.as_mut().context("repeat suffix absent")?;
        ensure!(
            suffix.repeat_request.is_none(),
            "repeated action request replays"
        );
        let secondary = reset
            .word(0x710)
            .context("repeat predicates secondary history unknown")?;
        // All four source predicates reject zero. Other secondary histories
        // require their exact predicate routes before selecting a repeat.
        if secondary != 0 {
            suffix.parameter_dependency = Some("action_callback_secondary_predicates");
            return Ok(());
        }
        let alternate = reset
            .byte(0xd00)
            .context("repeat action alternate flag unknown")?;
        suffix.repeat_request = Some(RepeatRequest {
            generic: if alternate == 0 {
                0x0900_0765
            } else {
                0x0900_07a7
            },
            parameter_bits: f32::MAX.to_bits(),
        });
        Ok(())
    }
}
