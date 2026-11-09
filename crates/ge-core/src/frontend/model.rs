use super::settings::Setting;
use crate::mission::{Difficulty, session::NewGameRequest};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    NewGame,
    Operative,
    Agent,
    DoubleOSeven,
    Classic,
    ContinueGame,
    SelectProfile,
    MissionSelect,
    Multiplayer,
    Options,
    Extras,
    ControlSettings,
    ScreenSettings,
    SoundSettings,
    ButtonLayout,
    WiiRemotePresets,
    ControllerSettings,
    AdsSnap,
    Brightness,
    MusicVolume,
    SoundVolume,
    DialogVolume,
    Credits,
    CheatCodes,
}

impl Action {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NewGame => "New Game",
            Self::Operative => Difficulty::Operative.label(),
            Self::Agent => Difficulty::Agent.label(),
            Self::DoubleOSeven => Difficulty::DoubleOSeven.label(),
            Self::Classic => Difficulty::Classic.label(),
            Self::ContinueGame => "Continue Game",
            Self::SelectProfile => "Select Profile",
            Self::MissionSelect => "Mission Select",
            Self::Multiplayer => "Multiplayer",
            Self::Options => "Options",
            Self::Extras => "Extras",
            Self::ControlSettings => "Control Settings",
            Self::ScreenSettings => "Screen Settings",
            Self::SoundSettings => "Sound Settings",
            Self::ButtonLayout => "Button Layout",
            Self::WiiRemotePresets => "Wii Remote Presets",
            Self::ControllerSettings => "Controller Settings",
            Self::AdsSnap => "ADS Snap",
            Self::Brightness => "Brightness",
            Self::MusicVolume => "Music Volume",
            Self::SoundVolume => "Sound Volume",
            Self::DialogVolume => "Dialog Volume",
            Self::Credits => "Credits",
            Self::CheatCodes => "Cheat Codes",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::NewGame => "Start a new playthrough of the single-player experience.",
            Self::Operative => "Choose a gentler single-player challenge.",
            Self::Agent => "Choose a tougher single-player challenge.",
            Self::DoubleOSeven => "Choose the advanced single-player challenge.",
            Self::Classic => "Choose classic difficulty with health and body armor.",
            Self::ContinueGame => "Continue from your last save.",
            Self::SelectProfile => "Manage game profiles.",
            Self::MissionSelect => "Choose a mission to replay.",
            Self::Multiplayer => "Play with others online or split screen.",
            Self::Options => "Change audio, visual, and control settings.",
            Self::Extras => "Access additional features and bonus content.",
            Self::ControlSettings => "Adjust controls and gameplay options.",
            Self::ScreenSettings => "Adjust game visuals.",
            Self::SoundSettings => "Adjust game audio.",
            Self::ButtonLayout => "Assign button layouts to use with each control style.",
            Self::WiiRemotePresets => "Adjust options specific to the Wii Remote and Wii Zapper.",
            Self::ControllerSettings => {
                "Adjust options specific to the Classic Controller, Classic Controller Pro and Nintendo GameCube Controller."
            }
            Self::AdsSnap => {
                "Aim Down Sights will snap to nearby targets and also provide targeting assistance when using a Classic Controller, Classic Controller Pro and Nintendo GameCube Controller."
            }
            Self::Brightness => "Adjust the brightness so that you can see the logo.",
            Self::MusicVolume => "Adjust the volume of in-game music.",
            Self::SoundVolume => "Adjust the volume of in-game sound effects.",
            Self::DialogVolume => "Adjust the volume of in-game speech.",
            Self::Credits => "View the credit roll.",
            Self::CheatCodes => "Enter cheat codes.",
        }
    }

    pub const fn setting(self) -> Option<Setting> {
        match self {
            Self::MusicVolume => Some(Setting::MusicVolume),
            Self::SoundVolume => Some(Setting::SoundVolume),
            Self::DialogVolume => Some(Setting::DialogVolume),
            Self::Brightness => Some(Setting::Brightness),
            _ => None,
        }
    }

    /// Tutorial menu 7d000110 operations 0x13..0x16, in source row order.
    pub const fn difficulty(self) -> Option<Difficulty> {
        match self {
            Self::Operative => Some(Difficulty::Operative),
            Self::Agent => Some(Difficulty::Agent),
            Self::DoubleOSeven => Some(Difficulty::DoubleOSeven),
            Self::Classic => Some(Difficulty::Classic),
            _ => None,
        }
    }
}

/// One-shot semantic work for the host. Audio effects remain independent;
/// accepting a difficulty does not publish a loaded world/player or session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    NewGame(NewGameRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Item {
    pub action: Action,
    pub label: &'static str,
    pub description: &'static str,
    pub enabled: bool,
    /// Source slider position: sound 0..100, brightness 25..125.
    pub value: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Reset,
}

/// A successful state change, for host sound/persistence routing. Rejected or
/// clamped input emits None, so it cannot masquerade as a changed setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    None,
    Navigate,
    Accept,
    Back,
    Adjust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    MainMenu,
    NewGameDifficulty,
    Options,
    ControlSettings,
    ScreenSettings,
    SoundSettings,
    Extras,
    Credits,
    /// The selected source action exists, but its runtime is not reconstructed.
    Unavailable(Action),
}

impl Screen {
    pub const fn title(self) -> &'static str {
        match self {
            Self::MainMenu => "Main Menu",
            Self::NewGameDifficulty => "Select Difficulty",
            Self::Options => "Options",
            Self::ControlSettings => "Control Settings",
            Self::ScreenSettings => "Screen Settings",
            Self::SoundSettings => "Sound Settings",
            Self::Extras => "Extras",
            Self::Credits => "Credits",
            Self::Unavailable(action) => action.label(),
        }
    }
}
