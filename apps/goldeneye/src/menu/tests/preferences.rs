use super::*;
fn path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "goldeneye-settings-{}-{}.json",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn missing_defaults_do_not_write_and_changes_roundtrip_replace() {
    let path = path();
    let (mut prefs, mut settings) = Preferences::open(Some(path.clone())).unwrap();
    assert!(!path.exists());
    assert!(!prefs.save(settings).unwrap());
    settings.adjust(ge_core::frontend::settings::Setting::MusicVolume, -1);
    assert!(prefs.save(settings).unwrap());
    assert_eq!(Preferences::open(Some(path.clone())).unwrap().1, settings);
    settings.adjust(ge_core::frontend::settings::Setting::SoundVolume, -1);
    assert!(prefs.save(settings).unwrap());
    assert_eq!(Preferences::open(Some(path.clone())).unwrap().1, settings);
    fs::remove_file(path).unwrap();
}
#[test]
fn malformed_unknown_schema_and_out_of_range_documents_are_preserved() {
    let path = path();
    for bytes in [b"broken".to_vec(), vec![b' '; MAX_BYTES as usize + 1],
        br#"{"schema_version":2,"settings":{"music_volume":66,"sound_volume":100,"dialog_volume":100,"brightness":100}}"#.to_vec(),
        br#"{"schema_version":1,"settings":{"music_volume":101,"sound_volume":100,"dialog_volume":100,"brightness":100}}"#.to_vec(),
        br#"{"schema_version":1,"settings":{"music_volume":66,"sound_volume":100,"dialog_volume":100,"brightness":0}}"#.to_vec(),
        br#"{"schema_version":1,"settings":{"music_volume":66,"sound_volume":100,"dialog_volume":100,"brightness":100},"extra":true}"#.to_vec()] {
        fs::write(&path, &bytes).unwrap();
        assert!(Preferences::open(Some(path.clone())).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_file(path).unwrap();
}
