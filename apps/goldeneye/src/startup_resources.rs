//! The implemented subset of legal-screen initialization: resident action tables.
//! This does not model the complete native resource queue or menu lifecycle.
use anyhow::{Context, Result, ensure};
use ge_assets::{edb::Edb, resource::GameAssets, spreadsheet};
use ge_core::{
    action_tables::{ActionTables, Budget},
    frontend_sequence::Stage,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
};

const RESOURCE: u32 = 0x01090001;
const ACTIONS: u32 = 0x14000017;
const COMPOSITES: u32 = 0x1400000b;
const RESOURCE_BYTES: u32 = 1024 * 1024;
// Host limits, not limits inferred from the original initializer.
const BUDGET: Budget = Budget {
    actions: 4096,
    composites: 1024,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    NotStarted,
    Loading,
    Initialized,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct Evidence {
    pub state: State,
    pub actions_ready: bool,
    pub resource_hash: u32,
    pub action_sheet_hash: u32,
    pub composite_sheet_hash: u32,
    pub actions: usize,
    pub composites: usize,
    pub resolved_slots: usize,
    pub unresolved_slots: usize,
    pub error: Option<String>,
    pub scope: &'static str,
}

pub struct StartupResources {
    disc: PathBuf,
    state: State,
    response: Option<Receiver<Result<ActionTables>>>,
    worker: Option<JoinHandle<()>>,
    tables: Option<ActionTables>,
    error: Option<String>,
}

impl StartupResources {
    pub fn new(disc: PathBuf) -> Self {
        Self {
            disc,
            state: State::NotStarted,
            response: None,
            worker: None,
            tables: None,
            error: None,
        }
    }

    /// Start once after the legal entry fade; subsequent polls never wait for I/O.
    pub fn tick(&mut self, stage: Stage, fade_active: bool) -> Result<()> {
        if self.state == State::Failed {
            anyhow::bail!(
                "startup action initializer failed: {}",
                self.error.as_deref().unwrap_or("unknown error")
            );
        }
        if self.state == State::NotStarted && stage == Stage::Legal && !fade_active {
            self.state = State::Loading;
            let disc = self.disc.clone();
            let (output, input) = mpsc::sync_channel(1);
            match thread::Builder::new()
                .name("startup-actions".into())
                .spawn(move || {
                    let _ = output.send(load_actions(&disc));
                }) {
                Ok(worker) => {
                    self.worker = Some(worker);
                    self.response = Some(input);
                }
                Err(error) => return self.fail(error.into()),
            }
        }
        let Some(response) = &self.response else {
            return Ok(());
        };
        match response.try_recv() {
            Ok(Ok(tables)) => {
                self.response.take();
                self.tables = Some(tables);
                self.state = State::Initialized;
                Ok(())
            }
            Ok(Err(error)) => self.fail(error),
            Err(mpsc::TryRecvError::Empty) => Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.fail(anyhow::anyhow!("startup action worker disconnected"))
            }
        }
    }

    fn fail(&mut self, error: anyhow::Error) -> Result<()> {
        self.response.take();
        self.state = State::Failed;
        self.error = Some(format!("{error:#}"));
        Err(error.context("initializing startup action tables"))
    }

    pub fn actions_ready(&self) -> bool {
        self.state == State::Initialized && self.tables.is_some()
    }

    pub fn evidence(&self) -> Evidence {
        let (actions, composites, resolved_slots, unresolved_slots) =
            self.tables.as_ref().map_or((0, 0, 0, 0), counts);
        Evidence {
            state: self.state,
            actions_ready: self.actions_ready(),
            resource_hash: RESOURCE,
            action_sheet_hash: ACTIONS,
            composite_sheet_hash: COMPOSITES,
            actions,
            composites,
            resolved_slots,
            unresolved_slots,
            error: self.error.clone(),
            scope: "Only the Action initializer: SJBE52 revision 0 o_action.edb group-zero raw action/composite rows and storage-first nullable binding. Hashes identify the selected resource/sheets; counts describe retained initialized tables, not progress estimates. Other native initializers and frontend-menu lifecycle are NOT READY in this implementation; complete native work-queue readiness is unknown. The 301/30-second legal host fallback remains unverified.",
        }
    }
}

impl Drop for StartupResources {
    fn drop(&mut self) {
        // Release a potentially blocked bounded sender before joining its worker.
        self.response.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn bind_rows(actions: Vec<[u32; 5]>, composites: Vec<[u32; 13]>) -> Result<ActionTables> {
    ActionTables::bind(actions, composites, BUDGET)
}

fn counts(tables: &ActionTables) -> (usize, usize, usize, usize) {
    let resolved = tables
        .composites()
        .iter()
        .flat_map(|row| &row.slots)
        .filter(|slot| slot.action_index.is_some())
        .count();
    (
        tables.actions().len(),
        tables.composites().len(),
        resolved,
        tables.composites().len() * 4 - resolved,
    )
}

fn load_actions(disc: &Path) -> Result<ActionTables> {
    let mut assets = GameAssets::open(disc, "Filelist.bin")?;
    ensure!(
        assets.disc.identity.game_id == "SJBE52" && assets.disc.identity.revision == 0,
        "startup action initialization requires SJBE52 revision 0"
    );
    // Check the payload limit before load_edb allocates the resource. Selection
    // remains unambiguous and uses the same full virtual path for the actual load.
    let mut matches = assets.index.entries.iter().filter(|entry| {
        entry
            .path
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("o_action.edb"))
    });
    let entry = matches.next().context("startup action resource absent")?;
    ensure!(
        matches.next().is_none(),
        "ambiguous startup action resource"
    );
    ensure!(
        entry.hash == RESOURCE,
        "startup action resource hash differs"
    );
    let selector = entry.path.clone();
    let location = entry
        .locations
        .first()
        .context("startup action resource location absent")?;
    let archive = format!("Filelist.{:03}", location.archive);
    let prefix = assets
        .disc
        .read(&archive, u64::from(location.offset), Edb::HEADER_BYTES)?;
    let header = Edb::read_header(&prefix)?;
    ensure!(
        header.hash == RESOURCE && header.payload_bytes <= RESOURCE_BYTES,
        "startup action resource identity/host byte budget differs"
    );
    let edb = assets.load_edb(&selector)?;
    ensure!(
        edb.header.hash == RESOURCE,
        "startup action EDB hash differs"
    );
    let action_sheet = edb
        .spreadsheet(ACTIONS)?
        .context("startup action spreadsheet absent")?;
    let action_rows = action_sheet
        .group(
            0,
            20,
            spreadsheet::Budget {
                group_fields: 1,
                rows: BUDGET.actions,
                row_bytes: 20,
            },
        )?
        .context("startup action group zero absent")?;
    let composite_sheet = edb
        .spreadsheet(COMPOSITES)?
        .context("startup composite spreadsheet absent")?;
    let composite_rows = composite_sheet
        .group(
            0,
            52,
            spreadsheet::Budget {
                group_fields: 1,
                rows: BUDGET.composites,
                row_bytes: 52,
            },
        )?
        .context("startup composite group zero absent")?;
    let actions = (0..action_rows.row_count)
        .map(|i| action_rows.words::<5>(i))
        .collect::<Result<Vec<_>>>()?;
    let composites = (0..composite_rows.row_count)
        .map(|i| composite_rows.words::<13>(i))
        .collect::<Result<Vec<_>>>()?;
    bind_rows(actions, composites)
}

#[cfg(test)]
#[path = "tests/startup_resources.rs"]
mod tests;
