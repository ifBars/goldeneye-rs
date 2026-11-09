//! Source-selected frontend settings, without persistence or device ownership.
//!
//! Sound sliders step by four and map their normalized value through square
//! root. Brightness steps by five and supplies a separate linear scale. Applying
//! those values to host audio/rendering does not establish original mixer or
//! display parity. Controller and gameplay settings are not represented here.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Setting {
    MusicVolume,
    SoundVolume,
    DialogVolume,
    Brightness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Values")]
pub struct Settings {
    music_volume: u8,
    sound_volume: u8,
    dialog_volume: u8,
    brightness: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Values {
    music_volume: u8,
    sound_volume: u8,
    dialog_volume: u8,
    brightness: u8,
}

impl TryFrom<Values> for Settings {
    type Error = &'static str;

    fn try_from(value: Values) -> Result<Self, Self::Error> {
        Self::from_values(
            value.music_volume,
            value.sound_volume,
            value.dialog_volume,
            value.brightness,
        )
        .ok_or("frontend settings exceed their supported ranges")
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            music_volume: 66,
            sound_volume: 100,
            dialog_volume: 100,
            brightness: 100,
        }
    }
}

impl Settings {
    /// Accepts bounded values without rounding imported values onto a step grid.
    pub fn from_values(
        music_volume: u8,
        sound_volume: u8,
        dialog_volume: u8,
        brightness: u8,
    ) -> Option<Self> {
        (music_volume <= 100
            && sound_volume <= 100
            && dialog_volume <= 100
            && (25..=125).contains(&brightness))
        .then_some(Self {
            music_volume,
            sound_volume,
            dialog_volume,
            brightness,
        })
    }

    pub fn music_volume(&self) -> u8 {
        self.music_volume
    }

    pub fn sound_volume(&self) -> u8 {
        self.sound_volume
    }

    pub fn dialog_volume(&self) -> u8 {
        self.dialog_volume
    }

    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    pub fn value(&self, setting: Setting) -> u8 {
        match setting {
            Setting::MusicVolume => self.music_volume,
            Setting::SoundVolume => self.sound_volume,
            Setting::DialogVolume => self.dialog_volume,
            Setting::Brightness => self.brightness,
        }
    }

    /// Preserves single-precision normalization before the double square root.
    /// The host's square-root result is not a Wii floating-point parity claim.
    pub fn gain(&self, setting: Setting) -> f32 {
        let normalized = f32::from(self.value(setting)) / 100.0;
        match setting {
            Setting::Brightness => normalized,
            _ => f64::from(normalized).sqrt() as f32,
        }
    }

    /// Handles only the source's decrement/increment requests; endpoints do not
    /// wrap. Returns whether the stored value changed.
    pub fn adjust(&mut self, setting: Setting, direction: i8) -> bool {
        if !matches!(direction, -1 | 1) {
            return false;
        }
        let (value, minimum, maximum, step) = match setting {
            Setting::MusicVolume => (&mut self.music_volume, 0, 100, 4),
            Setting::SoundVolume => (&mut self.sound_volume, 0, 100, 4),
            Setting::DialogVolume => (&mut self.dialog_volume, 0, 100, 4),
            Setting::Brightness => (&mut self.brightness, 25, 125, 5),
        };
        let next = (i16::from(*value) + i16::from(direction) * step).clamp(minimum, maximum);
        let changed = next != i16::from(*value);
        *value = next as u8;
        changed
    }

    /// Resets only the three sound sliders, preserving screen settings.
    pub fn reset_sound(&mut self) -> bool {
        let changed = (self.music_volume, self.sound_volume, self.dialog_volume) != (66, 100, 100);
        self.music_volume = 66;
        self.sound_volume = 100;
        self.dialog_volume = 100;
        changed
    }

    pub fn reset_screen(&mut self) -> bool {
        let changed = self.brightness != 100;
        self.brightness = 100;
        changed
    }
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
