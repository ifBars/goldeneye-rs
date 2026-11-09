//! Source row order from SJBE52 r0 menu definitions. Visual coordinates,
//! controller bindings and save-conditioned availability remain separate work.
use super::{Action, Screen};

pub(super) fn actions(screen: Screen) -> &'static [Action] {
    match screen {
        Screen::MainMenu => &[
            Action::NewGame,
            Action::ContinueGame,
            Action::SelectProfile,
            Action::MissionSelect,
            Action::Multiplayer,
            Action::Options,
            Action::Extras,
        ],
        // Tutorial-specific page 7d000110 omits menu1c's Time Trial branch.
        Screen::NewGameDifficulty => &[
            Action::Operative,
            Action::Agent,
            Action::DoubleOSeven,
            Action::Classic,
        ],
        Screen::Options => &[
            Action::ControlSettings,
            Action::ScreenSettings,
            Action::SoundSettings,
        ],
        Screen::ControlSettings => &[
            Action::ButtonLayout,
            Action::WiiRemotePresets,
            Action::ControllerSettings,
            Action::AdsSnap,
        ],
        Screen::ScreenSettings => &[Action::Brightness],
        Screen::SoundSettings => &[
            Action::MusicVolume,
            Action::SoundVolume,
            Action::DialogVolume,
        ],
        Screen::Extras => &[Action::Credits, Action::CheatCodes],
        Screen::Credits | Screen::Unavailable(_) => &[],
    }
}

pub(super) const fn destination(action: Action) -> Screen {
    match action {
        Action::NewGame => Screen::NewGameDifficulty,
        Action::Options => Screen::Options,
        Action::ControlSettings => Screen::ControlSettings,
        Action::ScreenSettings => Screen::ScreenSettings,
        Action::SoundSettings => Screen::SoundSettings,
        Action::Extras => Screen::Extras,
        Action::Credits => Screen::Credits,
        _ => Screen::Unavailable(action),
    }
}
