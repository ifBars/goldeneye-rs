use ge_core::mission::session::NewGameRequest;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStage {
    Admitting,
    Loading,
    AwaitingWorld,
    AwaitingPlayer,
    Failed,
}

/// Public UI categories deliberately exclude asset paths and decoder errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCategory {
    Admission,
    Loading,
    Worker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub stage: SessionStage,
    pub difficulty: u8,
    pub level_hash: u32,
    pub start_point_id: u32,
    pub session_active: bool,
    pub local_users: u32,
    pub resources_retained: usize,
    pub map_instances: usize,
    pub source_start_points: usize,
    pub world_initialization: Option<super::WorldEvidence>,
    pub world_registered: bool,
    pub player_created: bool,
    pub camera_ready: bool,
    pub mode: u32,
    pub backend: u32,
    pub manager_ready_flags_raw: u32,
    pub user_ready_flags_raw: u32,
    pub source_create_counter: u32,
    pub created_session_selected: bool,
    pub failure: Option<FailureCategory>,
}

impl Snapshot {
    pub(super) fn admitting(request: NewGameRequest) -> Self {
        Self {
            stage: SessionStage::Admitting,
            difficulty: request.difficulty().value(),
            level_hash: request.level_hash(),
            start_point_id: request.start_point_id(),
            session_active: false,
            local_users: 0,
            resources_retained: 0,
            map_instances: 0,
            source_start_points: 0,
            world_initialization: None,
            world_registered: false,
            player_created: false,
            camera_ready: false,
            mode: 0,
            backend: 0,
            manager_ready_flags_raw: 0,
            user_ready_flags_raw: 0,
            source_create_counter: 8,
            created_session_selected: false,
            failure: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub requested: u64,
    pub loaded: u64,
    pub cancelled: u64,
    pub failed: u64,
    pub retired_workers: usize,
    pub retired_worker_failures: u64,
    pub active: Option<Snapshot>,
    pub provider: &'static str,
    pub native_provider_timing_verified: bool,
    pub scope: &'static str,
}
