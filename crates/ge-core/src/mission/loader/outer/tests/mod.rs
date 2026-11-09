use super::*;
use anyhow::Result;

mod banks;
mod checkpoint;
mod owner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Level,
    Job,
    Dispatch(u32, u32),
    Session,
}

struct Provider {
    row: Option<BankRow>,
    multiplayer: bool,
    events: Vec<Event>,
    job: JobOutcome,
    dispatch: DispatchOutcome,
    fail_level: bool,
    fail_job: Option<usize>,
    fail_dispatch: Option<usize>,
    jobs: usize,
    dispatches: usize,
}

impl Provider {
    fn new(row: Option<BankRow>) -> Self {
        Self {
            row,
            multiplayer: false,
            events: Vec::new(),
            job: JobOutcome::Existing,
            dispatch: DispatchOutcome::Submitted,
            fail_level: false,
            fail_job: None,
            fail_dispatch: None,
            jobs: 0,
            dispatches: 0,
        }
    }
}

impl BankProvider for Provider {
    fn selected_level(&mut self) -> Result<Option<BankRow>> {
        self.events.push(Event::Level);
        if self.fail_level {
            anyhow::bail!("authored row unavailable");
        }
        Ok(self.row)
    }
    fn multiplayer_extra_required(&mut self) -> Result<bool> {
        self.events.push(Event::Session);
        Ok(self.multiplayer)
    }
    fn ensure_shared_type1_job(&mut self) -> Result<JobOutcome> {
        self.events.push(Event::Job);
        self.jobs += 1;
        if self.fail_job == Some(self.jobs) {
            anyhow::bail!("authored job unavailable");
        }
        Ok(self.job)
    }
    fn dispatch_bank(&mut self, token: u32, flags: u32) -> Result<DispatchOutcome> {
        self.events.push(Event::Dispatch(token, flags));
        self.dispatches += 1;
        if self.fail_dispatch == Some(self.dispatches) {
            anyhow::bail!("authored dispatch unavailable");
        }
        Ok(self.dispatch)
    }
}

fn row(primary: u32) -> BankRow {
    BankRow {
        primary,
        optional: [u32::MAX; 2],
    }
}
fn calls(owner: &BankOwner) -> Vec<u32> {
    owner.calls().iter().map(|call| call.token).collect()
}
fn common_fields() -> BankFields {
    BankFields {
        word_4c: 0x2d50_0399,
        word_50: 0x2d50_0014,
        word_54: 0x2d50_0001,
        byte_58: 0x7f,
    }
}
