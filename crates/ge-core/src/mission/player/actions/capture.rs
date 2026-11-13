use super::*;
use crate::animation::{
    dispatch::Fallback,
    scratch::{Arena, Context as Scratch},
};

impl Pending {
    pub fn capture_third_person(
        &mut self,
        components: &mut Preparation,
        context: &mut Scratch,
        arena: &mut Arena,
        fallback: &mut Fallback,
    ) -> Result<()> {
        ensure!(
            self.stage == Stage::AwaitingThirdPersonDispatch
                && self.generic == 0x0900_071e
                && self.components.is_some_and(|ids| components
                    .animator()
                    .is_some_and(|tp| tp.identity() == ids[1])),
            "initial TP capture order or owner differs"
        );
        self.stage = Stage::CapturingThirdPerson;
        fallback.store(0x0900_000b)?;
        let tp = components
            .animator()
            .context("TP dispatch position owner absent")?;
        ensure!(
            tp.word(0x178) == Some(0) && tp.word(0x17c) == Some(0),
            "fresh TP slot2 position query requires actual empty head"
        );
        self.dispatch = Some(ThirdPersonDispatch::new(0));
        // The actual slot2 list is empty, so position query returns +0.
        // First request's +0 parameter selects capture(maskF, preserve3, 10).
        components.capture_third_person_control(context, arena)?;
        fallback.store(0x0900_06c0)?;
        self.fallback = Some(fallback.value()?);
        self.fallback_writes = Some(fallback.writes());
        self.stage = Stage::AwaitingThirdPersonSelector;
        Ok(())
    }
}
