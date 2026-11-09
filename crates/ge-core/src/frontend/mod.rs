//! Pure frontend navigation and settings. Device input, persistence, assets and
//! rendering belong to the host app. The menu tree follows the supplied Wii
//! definitions; unavailable gameplay and controller actions remain explicit.
mod model;
mod screens;
pub mod settings;

pub use model::{Action, Command, Effect, Input, Item, Screen};
use serde::Serialize;
pub use settings::{Setting, Settings};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct State {
    screen: Screen,
    selected: usize,
    has_save: bool,
    settings: Settings,
    // Navigation records selections at every depth, including unavailable pages.
    parents: Vec<(Screen, usize)>,
    /// Ephemeral host work is neither persisted menu state nor audio metadata.
    #[serde(skip)]
    command: Option<Command>,
    /// Taking a command must not re-arm a held/repeated confirmation. Returning
    /// through Back explicitly cancels the page and permits a fresh selection.
    #[serde(skip)]
    command_submitted: bool,
}

impl State {
    /// Save availability is explicit supplied state, never an inferred save file.
    pub fn new(has_save: bool) -> Self {
        Self::with_settings(has_save, Settings::default())
    }

    pub fn with_settings(has_save: bool, settings: Settings) -> Self {
        Self {
            screen: Screen::MainMenu,
            selected: 0,
            has_save,
            settings,
            parents: Vec::new(),
            command: None,
            command_submitted: false,
        }
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn title(&self) -> &'static str {
        self.screen.title()
    }

    pub fn description(&self) -> &'static str {
        screens::actions(self.screen)
            .get(self.selected)
            .map_or("", |action| action.description())
    }

    /// Drain at most one accepted request. Repeated input stays latched until
    /// Back leaves the difficulty page, even after the host consumes this work.
    pub fn take_command(&mut self) -> Option<Command> {
        self.command.take()
    }

    /// Pages contain at most seven rows; values are current settings, not cached
    /// copies that can diverge from audio/display application in the host.
    pub fn items(&self) -> Vec<Item> {
        screens::actions(self.screen)
            .iter()
            .map(|&action| Item {
                action,
                label: action.label(),
                description: action.description(),
                enabled: !self.command_submitted
                    && (action != Action::ContinueGame || self.has_save),
                value: action.setting().map(|setting| self.settings.value(setting)),
            })
            .collect()
    }

    /// Host pointer adaptation. Re-selecting a valid row succeeds; callers can
    /// compare the previous index to avoid a repeated hover sound.
    pub fn select_index(&mut self, index: usize) -> bool {
        if self.command_submitted {
            return false;
        }
        let Some(&action) = screens::actions(self.screen).get(index) else {
            return false;
        };
        if action == Action::ContinueGame && !self.has_save {
            return false;
        }
        self.selected = index;
        true
    }

    pub fn input(&mut self, input: Input) -> Effect {
        if input == Input::Reset {
            let changed = match self.screen {
                Screen::SoundSettings => self.settings.reset_sound(),
                Screen::ScreenSettings => self.settings.reset_screen(),
                _ => false,
            };
            return if changed {
                Effect::Adjust
            } else {
                Effect::None
            };
        }
        if input == Input::Back {
            return if let Some((screen, selected)) = self.parents.pop() {
                self.command = None;
                self.command_submitted = false;
                self.screen = screen;
                self.selected = selected;
                Effect::Back
            } else {
                Effect::None
            };
        }
        if self.command_submitted {
            return Effect::None;
        }
        let actions = screens::actions(self.screen);
        let Some(&action) = actions.get(self.selected) else {
            return Effect::None;
        };
        match input {
            Input::Up | Input::Down => {
                let previous = self.selected;
                for _ in 0..actions.len() {
                    self.selected = if input == Input::Up {
                        (self.selected + actions.len() - 1) % actions.len()
                    } else {
                        (self.selected + 1) % actions.len()
                    };
                    if actions[self.selected] != Action::ContinueGame || self.has_save {
                        break;
                    }
                }
                if self.selected == previous {
                    Effect::None
                } else {
                    Effect::Navigate
                }
            }
            Input::Left | Input::Right => {
                let Some(setting) = action.setting() else {
                    return Effect::None;
                };
                let direction = if input == Input::Left { -1 } else { 1 };
                if self.settings.adjust(setting, direction) {
                    Effect::Adjust
                } else {
                    Effect::None
                }
            }
            Input::Confirm => {
                if action.setting().is_some() || (action == Action::ContinueGame && !self.has_save)
                {
                    return Effect::None;
                }
                if self.screen == Screen::NewGameDifficulty {
                    let Some(difficulty) = action.difficulty() else {
                        return Effect::None;
                    };
                    self.command = Some(Command::NewGame(
                        crate::mission::session::NewGameRequest::new(difficulty),
                    ));
                    self.command_submitted = true;
                    return Effect::Accept;
                }
                self.parents.push((self.screen, self.selected));
                self.screen = screens::destination(action);
                self.selected = 0;
                Effect::Accept
            }
            Input::Back | Input::Reset => Effect::None,
        }
    }
}

#[cfg(test)]
mod tests;
