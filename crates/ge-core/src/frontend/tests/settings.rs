use super::{Setting, Settings};
use serde::{Deserialize, de::value::MapDeserializer};

#[test]
fn defaults_and_gain_keep_music_at_its_independent_level() {
    let settings = Settings::default();
    assert_eq!(settings.music_volume(), 66);
    assert_eq!(settings.sound_volume(), 100);
    assert_eq!(settings.dialog_volume(), 100);
    assert_eq!(settings.brightness(), 100);
    assert_eq!(settings.gain(Setting::MusicVolume), 0.812_403_86);
    assert_eq!(settings.gain(Setting::SoundVolume), 1.0);
    assert_eq!(settings.gain(Setting::Brightness), 1.0);
}

#[test]
fn audio_steps_clamp_without_rounding_off_grid_values() {
    let mut settings = Settings::from_values(2, 99, 0, 100).unwrap();
    assert!(settings.adjust(Setting::MusicVolume, -1));
    assert_eq!(settings.music_volume(), 0);
    assert!(!settings.adjust(Setting::MusicVolume, -1));
    assert!(settings.adjust(Setting::SoundVolume, 1));
    assert_eq!(settings.sound_volume(), 100);
    assert!(!settings.adjust(Setting::SoundVolume, 1));
    assert!(settings.adjust(Setting::DialogVolume, 1));
    assert_eq!(settings.dialog_volume(), 4);
    assert!(settings.adjust(Setting::MusicVolume, 1));
    assert_eq!(settings.music_volume(), 4);
    assert_eq!(settings.gain(Setting::DialogVolume), 0.2);
    assert_eq!(settings.brightness(), 100);
}

#[test]
fn brightness_uses_its_own_step_limits_and_linear_scale() {
    let mut settings = Settings::from_values(66, 100, 100, 123).unwrap();
    assert!(settings.adjust(Setting::Brightness, 1));
    assert_eq!(settings.brightness(), 125);
    assert_eq!(settings.gain(Setting::Brightness), 1.25);
    assert!(!settings.adjust(Setting::Brightness, 1));
    for _ in 0..30 {
        settings.adjust(Setting::Brightness, -1);
    }
    assert_eq!(settings.brightness(), 25);
    assert_eq!(settings.gain(Setting::Brightness), 0.25);
    assert!(!settings.adjust(Setting::Brightness, -1));
    assert_eq!(settings.music_volume(), 66);
}

#[test]
fn invalid_directions_do_not_mutate_and_resets_are_scoped() {
    let mut settings = Settings::from_values(30, 40, 50, 60).unwrap();
    let saved = settings;
    for direction in [i8::MIN, -2, 0, 2, i8::MAX] {
        assert!(!settings.adjust(Setting::MusicVolume, direction));
        assert!(!settings.adjust(Setting::Brightness, direction));
        assert_eq!(settings, saved);
    }
    assert!(settings.reset_sound());
    assert_eq!(settings, Settings::from_values(66, 100, 100, 60).unwrap());
    assert!(!settings.reset_sound());
    assert!(settings.reset_screen());
    assert_eq!(settings, Settings::default());
    assert!(!settings.reset_screen());
}

fn deserialize(fields: &[(&str, u8)]) -> Result<Settings, serde::de::value::Error> {
    Settings::deserialize(MapDeserializer::new(fields.iter().copied()))
}

#[test]
fn deserialization_rejects_out_of_range_unknown_missing_and_duplicate_fields() {
    let valid = [
        ("music_volume", 66),
        ("sound_volume", 100),
        ("dialog_volume", 100),
        ("brightness", 100),
    ];
    assert_eq!(deserialize(&valid).unwrap(), Settings::default());
    for (index, invalid) in [(0, 101), (1, 255), (2, 101), (3, 24), (3, 126)] {
        let mut fields = valid;
        fields[index].1 = invalid;
        assert!(deserialize(&fields).is_err());
    }
    assert!(deserialize(&valid[..3]).is_err());
    let mut fields = valid.to_vec();
    fields.push(("master_volume", 100));
    assert!(deserialize(&fields).is_err());
    fields.pop();
    fields.push(("music_volume", 60));
    assert!(deserialize(&fields).is_err());
}
