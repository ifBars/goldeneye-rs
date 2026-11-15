use super::*;

#[test]
fn reference_capture_uses_an_explicit_bounded_physical_canvas() {
    let cli = Cli::try_parse_from([
        "goldeneye",
        "play",
        "disc.rvz",
        "--frames",
        "1",
        "--capture-size",
        "640",
        "480",
    ])
    .unwrap();
    let Some(Command::Play { options }) = cli.command else {
        panic!("play command absent")
    };
    assert_eq!(options.window.capture_size, [640, 480]);
    options.window.validate().unwrap();
    for args in [
        vec!["goldeneye", "play", "--capture-size", "640", "480"],
        vec![
            "goldeneye",
            "play",
            "--frames",
            "1",
            "--capture-size",
            "640",
        ],
        vec![
            "goldeneye",
            "play",
            "--frames",
            "1",
            "--capture-size",
            "0",
            "480",
        ],
        vec![
            "goldeneye",
            "play",
            "--frames",
            "1",
            "--capture-size",
            "640",
            "4097",
        ],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
