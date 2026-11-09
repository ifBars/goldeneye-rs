//! Source-bound native menu presentation, settings persistence and resident SFX
//! submission. Developer inputs use the app handler, not OS input automation.
mod game_session;
use crate::{cargo, executable, now, output, record, report, source_hash, write_json};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

// Six menu images, title logo, seven warning/legal images, font, solid fill and
// movie target. This is an asset-set requirement, not a sampled corpus count.
const MINIMUM_TEXTURES: u64 = 17;

pub fn run(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    let result = (|| -> Result<Value> {
        cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
        let folder =
            Path::new("local/menu-smoke").join(format!("{}-{}", now(), std::process::id()));
        fs::create_dir_all("local/menu-smoke")?;
        fs::create_dir(&folder)?;
        let capture = folder.join("main-menu.png");
        let preferences = folder.join("preferences.json");
        let settings_file = preferences
            .to_str()
            .context("menu settings path must be UTF-8")?;
        let disc = disc.to_str().context("menu disc path must be UTF-8")?;
        let inventory: Value =
            serde_json::from_slice(&output(&executable("release"), &["inspect", disc])?)
                .context("menu disc inspection did not emit JSON")?;
        let raw = output(
            &executable("release"),
            &[
                "play",
                disc,
                "--skip-startup",
                "--mute",
                "--settings-file",
                settings_file,
                "--frames",
                "3",
                "--hidden",
                "--smoke-resize",
                "--capture",
                capture
                    .to_str()
                    .context("menu capture path must be UTF-8")?,
            ],
        )?;
        let window: Value =
            serde_json::from_slice(&raw).context("native menu did not emit JSON")?;
        write_json(&folder.join("main-menu.json"), &window)?;
        let bytes = fs::read(&capture).context("native menu capture missing")?;
        validate(&window, &bytes)?;
        ensure!(
            !preferences.exists(),
            "unchanged default settings wrote a preferences file"
        );
        let interaction = probe(
            disc,
            settings_file,
            &folder,
            "settings-sfx",
            "up,up,confirm,down,down,confirm,left,down,left,back",
            false,
        )?;
        validate_interaction(&interaction, false)?;
        let stored = fs::read(&preferences).context("changed menu settings were not persisted")?;
        validate_preferences(&stored)?;
        let reopened = probe(
            disc,
            settings_file,
            &folder,
            "settings-reopened",
            "up,up,confirm,down,down,confirm",
            true,
        )?;
        validate_interaction(&reopened, true)?;
        ensure!(
            fs::read(&preferences)? == stored,
            "read-only reopen rewrote preferences"
        );
        let credits = probe(
            disc,
            settings_file,
            &folder,
            "credits",
            "up,confirm,confirm",
            true,
        )?;
        validate_credits(&credits, false)?;
        let credits_return = probe(
            disc,
            settings_file,
            &folder,
            "credits-return",
            "up,confirm,confirm,back",
            true,
        )?;
        validate_credits(&credits_return, true)?;
        ensure!(
            credits["window"]["capture_rgba_sha256"]
                != credits_return["window"]["capture_rgba_sha256"],
            "credits/returned-Extras captures did not change"
        );
        ensure!(
            fs::read(&preferences)? == stored,
            "Extras/Credits rewrote preferences"
        );
        let game_preferences = folder.join("game-session-preferences.json");
        let game_session = game_session::run(
            disc,
            game_preferences
                .to_str()
                .context("session preferences path must be UTF-8")?,
            &folder,
        )?;
        ensure!(
            !game_preferences.exists(),
            "New Game wrote unrelated menu preferences"
        );
        ensure!(
            inventory["disc"]["game_id"] == "SJBE52" && inventory["disc"]["revision"] == 0,
            "menu smoke requires SJBE52 revision zero"
        );
        let details = json!({
            "disc": inventory["disc"], "window": window,
            "capture_png_sha256":format!("{:x}",Sha256::digest(&bytes)),
            "capture_png_bytes":bytes.len(), "minimum_textures":MINIMUM_TEXTURES,
            "interaction": interaction, "persisted_reopen": reopened,
            "credits":credits,"credits_return":credits_return,
            "game_session":game_session,
            "preferences_bytes":stored.len(), "preferences_sha256":format!("{:x}", Sha256::digest(&stored)),
            "reference_compared":false,
            "scope":"Hidden native main-menu presentation/resize and shared-handler inputs navigate Options/Sound Settings, change Music/Sound sliders, return to Options and reopen persisted host preferences. Extras/Credits opens the local source-text roll and returns through the same handler. Four exact source resident-cue bindings decode and finish independent voices while music continues; source-selected host bus gains apply. All probe captures are local. No OS input automation, original listening/visual comparison, Wii saves, controller mapping or complete menu/gameplay parity."
        });
        println!(
            "Native menu: presentation, settings/SFX and seven New Game/session probes passed. {}",
            capture.display()
        );
        Ok(details)
    })();
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Native menu smoke failed: {error:#}");
            (
                false,
                json!({"message":"Native menu smoke failed; inspect local terminal output.","reference_compared":false}),
            )
        }
    };
    record("menu", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "native menu smoke failed");
    Ok(())
}

fn probe(
    disc: &str,
    preferences: &str,
    folder: &Path,
    label: &str,
    inputs: &str,
    muted: bool,
) -> Result<Value> {
    let capture = folder.join(format!("{label}.png"));
    let mut args = vec![
        "play",
        disc,
        "--skip-startup",
        "--hidden",
        "--frames",
        "120",
        "--settings-file",
        preferences,
        "--menu-inputs",
        inputs,
        "--capture",
        capture
            .to_str()
            .context("menu probe capture path must be UTF-8")?,
    ];
    if muted {
        args.push("--mute");
    }
    let window: Value = serde_json::from_slice(&output(&executable("release"), &args)?)
        .context("native settings probe did not emit JSON")?;
    write_json(&folder.join(format!("{label}.json")), &window)?;
    let bytes = fs::read(&capture)?;
    validate_presentation(&window, &bytes, 120, false)?;
    Ok(
        json!({"window":window,"capture_png_sha256":format!("{:x}",Sha256::digest(&bytes)),
        "capture_png_bytes":bytes.len(),"reference_compared":false}),
    )
}

fn validate(window: &Value, png: &[u8]) -> Result<()> {
    validate_presentation(window, png, 3, true)?;
    ensure!(
        window["menu"]["screen"] == "main_menu"
            && window["menu"]["selected"] == 0
            && window["menu"]["has_save"] == false,
        "menu did not retain the initial New Game checkpoint state"
    );
    ensure!(
        window["playback"]["stage"] == "main_menu" && window["playback"]["muted"] == true,
        "menu smoke did not honor direct-start/mute options"
    );
    Ok(())
}

fn validate_presentation(window: &Value, png: &[u8], frames: u64, resized: bool) -> Result<()> {
    ensure!(
        window["hidden"] == true
            && window["frames_presented"]
                .as_u64()
                .is_some_and(|n| n >= frames)
            && (!resized || window["resize_count"].as_u64().is_some_and(|n| n >= 1))
            && window["scene_uploads"] == 1,
        "menu native presentation/resize/upload checks failed"
    );
    ensure!(
        window["textures"]
            .as_u64()
            .is_some_and(|n| n >= MINIMUM_TEXTURES),
        "menu did not load its required texture set"
    );
    for field in ["adapter", "backend", "surface_format"] {
        ensure!(
            window[field].as_str().is_some_and(|s| !s.trim().is_empty()),
            "menu renderer identity missing {field}"
        );
    }
    let hash = window["capture_rgba_sha256"]
        .as_str()
        .context("menu capture RGBA hash missing")?;
    ensure!(
        hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "menu capture RGBA hash malformed"
    );
    let dimensions = window["final_size"]
        .as_array()
        .context("menu final size missing")?;
    ensure!(
        dimensions.len() == 2,
        "menu final size must have two dimensions"
    );
    let width = dimensions[0].as_u64().context("menu width missing")?;
    let height = dimensions[1].as_u64().context("menu height missing")?;
    ensure!(
        width > 0 && height > 0 && width <= 4096 && height <= 4096,
        "menu final dimensions out of bounds"
    );
    let pixels = width
        .checked_mul(height)
        .context("menu pixel count overflow")?;
    ensure!(
        window["capture_non_background_pixels"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= pixels),
        "menu capture pixel diagnostics invalid"
    );
    // This validates capture container/header dimensions, not a second decode or
    // an independent RGBA/reference comparison. The native renderer reports RGBA.
    ensure!(
        png.len() >= 45
            && png[..8] == *b"\x89PNG\r\n\x1a\n"
            && png[8..12] == 13u32.to_be_bytes()
            && png[12..16] == *b"IHDR"
            && png[png.len() - 12..png.len() - 8] == [0; 4]
            && png[png.len() - 8..png.len() - 4] == *b"IEND",
        "menu capture PNG container missing/truncated"
    );
    let png_width = u32::from_be_bytes(png[16..20].try_into()?);
    let png_height = u32::from_be_bytes(png[20..24].try_into()?);
    ensure!(
        u64::from(png_width) == width && u64::from(png_height) == height,
        "menu PNG dimensions differ from native presentation"
    );
    Ok(())
}

const MENU_CUES: [(u32, u32, u64, u32, &str); 4] = [
    (
        0x2d70044a,
        0x2da01128,
        4480,
        0x8b,
        "af4ccf127746653148c15146a7bb7289e248a255b97cd8931231176f421621b0",
    ),
    (
        0x2d70044d,
        0x2da01125,
        23184,
        0x2d3,
        "5699db8837ca5b0037d18566853054f4b257f22a1d0c3e9e49dca02eb96f042b",
    ),
    (
        0x2d700449,
        0x2da01127,
        9016,
        0x118,
        "6fce95d5529739041f5a6ed01f2ee2ca0f2451626c750ba93824c48083560288",
    ),
    (
        0x2d70044e,
        0x2da01126,
        2688,
        0x52,
        "6a2fbea8507d3ce9b09813289be200b5297d89ad49dd4d9522510e35cfec6857",
    ),
];

fn settings(music: u64, sound: u64) -> Value {
    json!({"music_volume":music,"sound_volume":sound,"dialog_volume":100,"brightness":100})
}

fn validate_preferences(bytes: &[u8]) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= 4096,
        "preferences size outside host budget"
    );
    let value: Value = serde_json::from_slice(bytes)?;
    ensure!(
        value == json!({"schema_version":1,"settings":settings(62,96)}),
        "persisted preferences differ from changed settings or contain unexpected data"
    );
    Ok(())
}

fn validate_credits(probe: &Value, returned: bool) -> Result<()> {
    let window = &probe["window"];
    ensure!(
        window["credits_lines"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 4096)
            && window["menu_elapsed_seconds"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n > 0.),
        "credits source lines or active screen clock missing"
    );
    ensure!(
        window["menu"]["screen"] == if returned { "extras" } else { "credits" }
            && window["menu"]["selected"] == 0
            && window["menu"]["has_save"] == false
            && window["menu"]["settings"] == settings(62, 96),
        "credits navigation did not retain source screen/parent/settings state"
    );
    let session = &window["menu_session"];
    ensure!(
        session["writes"] == 0 && session["dirty"] == false,
        "credits-only navigation mutated preferences"
    );
    let steps = session["probe"]
        .as_array()
        .context("credits input trace missing")?;
    let expected = [
        ("up", "navigate", "main_menu", 6),
        ("confirm", "accept", "extras", 0),
        ("confirm", "accept", "credits", 0),
        ("back", "back", "extras", 0),
    ];
    ensure!(
        steps.len() == if returned { 4 } else { 3 },
        "credits input trace incomplete"
    );
    for (index, (step, &(input, effect, screen, selected))) in
        steps.iter().zip(&expected).enumerate()
    {
        ensure!(
            step["frame"] == (index as u64 + 1) * 6
                && step["input"] == input
                && step["effect"] == effect
                && step["screen"] == screen
                && step["selected"] == selected
                && step["settings"] == settings(62, 96),
            "credits trace step{index} differs"
        );
    }
    let playback = &window["playback"];
    ensure!(
        playback["stage"] == "main_menu"
            && playback["muted"] == true
            && playback["audio"].is_null()
            && playback["menu_audio"].is_null()
            && playback["menu_effect_requests"]
                == if returned {
                    json!({"navigate":1,"accept":2,"back":1})
                } else {
                    json!({"navigate":1,"accept":2})
                },
        "credits probe did not honor muted accepted effects"
    );
    Ok(())
}

fn validate_interaction(probe: &Value, reopened: bool) -> Result<()> {
    let window = &probe["window"];
    let playback = &window["playback"];
    ensure!(
        playback["stage"] == "main_menu" && playback["muted"] == reopened,
        "settings probe did not retain direct-menu/mute state"
    );
    ensure!(
        window["menu"]["has_save"] == false
            && window["menu"]["settings"] == settings(62, 96)
            && window["menu"]["screen"]
                == if reopened {
                    "sound_settings"
                } else {
                    "options"
                }
            && window["menu"]["selected"] == if reopened { 0 } else { 2 },
        "settings navigation did not finish at the expected screen/values"
    );
    let session = &window["menu_session"];
    ensure!(
        session["dirty"] == false
            && session["writes"]
                .as_u64()
                .is_some_and(|n| if reopened { n == 0 } else { n > 0 }),
        "settings session did not flush mutations or rewrote unchanged preferences"
    );
    let steps = session["probe"]
        .as_array()
        .context("settings input trace missing")?;
    let expected = [
        ("up", "navigate", "main_menu", 6),
        ("up", "navigate", "main_menu", 5),
        ("confirm", "accept", "options", 0),
        ("down", "navigate", "options", 1),
        ("down", "navigate", "options", 2),
        ("confirm", "accept", "sound_settings", 0),
        ("left", "adjust", "sound_settings", 0),
        ("down", "navigate", "sound_settings", 1),
        ("left", "adjust", "sound_settings", 1),
        ("back", "back", "options", 2),
    ];
    ensure!(
        steps.len() == if reopened { 6 } else { expected.len() },
        "settings input trace incomplete"
    );
    for (index, (step, &(input, effect, screen, selected))) in
        steps.iter().zip(&expected).enumerate()
    {
        let current = if reopened {
            settings(62, 96)
        } else {
            settings(
                if index >= 6 { 62 } else { 66 },
                if index >= 8 { 96 } else { 100 },
            )
        };
        ensure!(
            step["frame"] == (index as u64 + 1) * 6
                && step["input"] == input
                && step["effect"] == effect
                && step["screen"] == screen
                && step["selected"] == selected
                && step["settings"] == current,
            "settings trace step{index} differs from applied input"
        );
    }
    ensure!(
        playback["menu_effect_requests"]
            == if reopened {
                json!({"navigate":4,"accept":2})
            } else {
                json!({"navigate":5,"accept":2,"adjust":2,"back":1})
            },
        "menu effect request counts differ from accepted input effects"
    );
    if reopened {
        ensure!(
            playback["audio"].is_null() && playback["menu_audio"].is_null(),
            "muted reopened menu created audio transport or cue cache"
        );
        return Ok(());
    }
    validate_menu_audio(playback)
}

fn validate_menu_audio(playback: &Value) -> Result<()> {
    let menu = &playback["menu_audio"];
    let cues = menu["cues"]
        .as_array()
        .context("menu cue cache evidence missing")?;
    ensure!(
        cues.len() == MENU_CUES.len()
            && menu["voices_started"] == 10
            && menu["capacity_dropped"] == 0
            && menu["source_request_gain"] == 1.0,
        "menu cue cache/voice counts/gain differ from input sequence"
    );
    for (cue, &(sound, wave, frames, flags, digest)) in cues.iter().zip(&MENU_CUES) {
        let info = &cue["info"];
        ensure!(
            info["resource_hash"] == 0x2d500001u64
                && info["sound_hash"] == sound
                && info["wave_hash"] == wave
                && info["sample_rate"] == 32000
                && info["channels"] == 2
                && info["sample_frames"] == frames
                && info["codec_raw"] == 0x13
                && info["loop_start_frame"].is_null()
                && cue["source_flags_raw"] == flags
                && cue["stop_on_owner_release"] == false
                && cue["reference_compared"] == false
                && cue["decoded_frames"] == frames
                && cue["interleaved_samples"] == frames * 2
                && cue["decoded_chunks"].as_u64().is_some_and(|n| n > 0)
                && cue["nonzero_samples"]
                    .as_u64()
                    .is_some_and(|n| n > 0 && n <= frames * 2)
                && cue["peak_absolute_sample"]
                    .as_u64()
                    .is_some_and(|n| n > 0 && n <= 32768)
                && cue["pcm_s16le_sha256"] == digest,
            "menu cue binding/complete decode differs from confirmed resident sound{sound:x}"
        );
        let source = &cue["source_origin"];
        for (kind, index, hash) in [
            ("bank", "File_COM.bin", 0x2d500001u64),
            ("sound_info", "File_USA.bin", 0x2d200000u64),
        ] {
            ensure!(
                source[kind]["index"] == index
                    && source[kind]["resource_hash"] == hash
                    && source[kind]["bytes"].as_u64().is_some_and(|n| n > 0)
                    && source[kind]["archive"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty()),
                "menu cue lacks actual indexed {kind} origin"
            );
        }
    }
    let audio = &playback["audio"];
    ensure!(
        audio["backend"] == "WASAPI"
            && audio["device_errors"] == 0
            && audio["paused"] == false
            && audio["source_rate"] == 31999
            && audio["source_channels"] == 2
            && audio["source_frames_consumed"]
                .as_u64()
                .is_some_and(|n| n > 0)
            && audio["oneshot_started"] == 10
            && audio["oneshot_completed"] == 10
            && audio["oneshot_active"] == 0
            && audio["oneshot_voice_limit"] == 8
            && audio["oneshot_retained_samples"] == 0,
        "menu audio did not finish independent effects alongside source music"
    );
    let output_rate = audio["output_rate"]
        .as_u64()
        .context("menu audio output rate missing")?;
    ensure!(
        (8000..=192000).contains(&output_rate),
        "menu host output rate outside bounds"
    );
    let expected_frames: u64 = [(4480u64, 5u64), (23184, 2), (9016, 1), (2688, 2)]
        .iter()
        .map(|(frames, count)| (frames * output_rate).div_ceil(32000) * count)
        .sum();
    ensure!(
        audio["oneshot_output_frames"] == expected_frames,
        "menu effects did not consume their complete host-clock intervals"
    );
    for (field, expected) in [
        ("primary_gain", (0.62f32 as f64).sqrt() as f32),
        ("sound_gain", (0.96f32 as f64).sqrt() as f32),
    ] {
        ensure!(
            audio[field]
                .as_f64()
                .is_some_and(|n| (n - f64::from(expected)).abs() <= 1e-6),
            "menu {field} did not follow source-selected settings"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> Value {
        json!({"hidden":true,"frames_presented":3,"resize_count":1,"scene_uploads":1,
            "menu":{"screen":"main_menu","selected":0,"has_save":false},"textures":17,
            "playback":{"stage":"main_menu","muted":true},
            "adapter":"Authored adapter","backend":"Authored backend","surface_format":"Rgba8Unorm",
            "capture_rgba_sha256":"ab".repeat(32),"capture_non_background_pixels":100,"final_size":[20,10]})
    }
    fn png() -> Vec<u8> {
        // Authored header fixture for the container check; not a decoded picture.
        let mut bytes = vec![0; 45];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&20u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&10u32.to_be_bytes());
        bytes[37..41].copy_from_slice(b"IEND");
        bytes
    }
    fn interaction(reopened: bool) -> Value {
        let commands = [
            ("up", "navigate", "main_menu", 6),
            ("up", "navigate", "main_menu", 5),
            ("confirm", "accept", "options", 0),
            ("down", "navigate", "options", 1),
            ("down", "navigate", "options", 2),
            ("confirm", "accept", "sound_settings", 0),
            ("left", "adjust", "sound_settings", 0),
            ("down", "navigate", "sound_settings", 1),
            ("left", "adjust", "sound_settings", 1),
            ("back", "back", "options", 2),
        ];
        let trace: Vec<_> = commands.iter().take(if reopened {6} else {10}).enumerate()
            .map(|(i,(input,effect,screen,selected))| json!({"frame":(i+1)*6,
                "input":input,"effect":effect,"screen":screen,"selected":selected,
                "settings":if reopened {settings(62,96)} else {settings(if i>=6 {62} else {66},if i>=8 {96} else {100})}})).collect();
        let cues: Vec<_> = MENU_CUES
            .iter()
            .map(|&(sound, wave, frames, flags, digest)| {
                json!({
            "info":{"resource_hash":0x2d500001u64,"sound_hash":sound,"wave_hash":wave,
                "sample_rate":32000,"channels":2,"sample_frames":frames,"codec_raw":0x13,
                "loop_start_frame":null},
            "source_flags_raw":flags,"stop_on_owner_release":false,"reference_compared":false,
            "decoded_frames":frames,"interleaved_samples":frames*2,"decoded_chunks":2,
            "nonzero_samples":frames,"peak_absolute_sample":100,"pcm_s16le_sha256":digest,
            "source_origin":{"bank":{"index":"File_COM.bin","resource_hash":0x2d500001u64,
                "bytes":100,"archive":"File_COM.000"},"sound_info":{"index":"File_USA.bin",
                "resource_hash":0x2d200000u64,"bytes":100,"archive":"File_USA.000"}}})
            })
            .collect();
        let audio = json!({"backend":"WASAPI","device_errors":0,"paused":false,
            "source_rate":31999,"source_channels":2,"source_frames_consumed":10000,
            "oneshot_started":10,"oneshot_completed":10,"oneshot_active":0,
            "oneshot_voice_limit":8,"oneshot_retained_samples":0,"output_rate":48000,
            "oneshot_output_frames":124740,"primary_gain":(0.62f32 as f64).sqrt() as f32,
            "sound_gain":(0.96f32 as f64).sqrt() as f32});
        json!({"window":{"menu":{"has_save":false,"settings":settings(62,96),
            "screen":if reopened {"sound_settings"} else {"options"},"selected":if reopened {0} else {2}},
            "menu_session":{"dirty":false,"writes":if reopened {0} else {1},"probe":trace},
            "playback":{"stage":"main_menu","muted":reopened,
                "menu_effect_requests":if reopened {json!({"navigate":4,"accept":2})} else {
                    json!({"navigate":5,"accept":2,"adjust":2,"back":1})},
                "menu_audio":if reopened {Value::Null} else {json!({"cues":cues,
                    "voices_started":10,"capacity_dropped":0,"source_request_gain":1.0})},
                "audio":if reopened {Value::Null} else {audio}}}})
    }
    #[test]
    fn changed_settings_require_ordered_applied_inputs_flush_and_read_only_reopen() {
        validate_interaction(&interaction(false), false).unwrap();
        validate_interaction(&interaction(true), true).unwrap();
        for (pointer, value) in [
            ("/window/menu/settings/sound_volume", json!(100)),
            ("/window/menu/screen", json!("sound_settings")),
            ("/window/menu_session/dirty", json!(true)),
            ("/window/menu_session/writes", json!(0)),
            ("/window/menu_session/probe/6/effect", json!("none")),
            (
                "/window/menu_session/probe/8/settings/sound_volume",
                json!(100),
            ),
            ("/window/menu_session/probe/9/frame", json!(54)),
            ("/window/playback/menu_effect_requests/back", json!(0)),
        ] {
            let mut current = interaction(false);
            *current.pointer_mut(pointer).unwrap() = value;
            assert!(validate_interaction(&current, false).is_err(), "{pointer}");
        }
        let mut reopened = interaction(true);
        reopened["window"]["menu_session"]["writes"] = json!(1);
        assert!(validate_interaction(&reopened, true).is_err());
        let mut reopened = interaction(true);
        reopened["window"]["playback"]["audio"] = json!({"backend":"WASAPI"});
        assert!(validate_interaction(&reopened, true).is_err());
    }
    #[test]
    fn menu_sound_proof_requires_exact_bindings_complete_voices_music_and_dynamic_gains() {
        for (pointer, value) in [
            ("/menu_audio/voices_started", json!(9)),
            ("/menu_audio/capacity_dropped", json!(1)),
            ("/menu_audio/source_request_gain", json!(0.5)),
            ("/menu_audio/cues/0/info/wave_hash", json!(0x2da01125u64)),
            ("/menu_audio/cues/1/source_flags_raw", json!(1 << 24)),
            (
                "/menu_audio/cues/2/pcm_s16le_sha256",
                json!("ab".repeat(32)),
            ),
            ("/menu_audio/cues/3/decoded_frames", json!(0)),
            ("/audio/oneshot_completed", json!(9)),
            ("/audio/oneshot_output_frames", json!(124739)),
            ("/audio/source_frames_consumed", json!(0)),
            ("/audio/primary_gain", json!(1.0)),
            ("/audio/sound_gain", json!(1.0)),
            ("/audio/device_errors", json!(1)),
        ] {
            let mut playback = interaction(false)["window"]["playback"].clone();
            *playback.pointer_mut(pointer).unwrap() = value;
            assert!(validate_menu_audio(&playback).is_err(), "{pointer}");
        }
    }
    #[test]
    fn persisted_document_is_bounded_versioned_settings_only() {
        let valid =
            serde_json::to_vec(&json!({"schema_version":1,"settings":settings(62,96)})).unwrap();
        validate_preferences(&valid).unwrap();
        for invalid in [
            vec![],
            vec![b' '; 4097],
            b"broken".to_vec(),
            serde_json::to_vec(&json!({"schema_version":2,"settings":settings(62,96)})).unwrap(),
            serde_json::to_vec(&json!({"schema_version":1,"settings":settings(66,100)})).unwrap(),
            serde_json::to_vec(&json!({"schema_version":1,"settings":settings(62,96),"assets":[]}))
                .unwrap(),
        ] {
            assert!(validate_preferences(&invalid).is_err());
        }
    }
    #[test]
    fn credits_require_loaded_source_lines_real_navigation_and_parent_return() {
        for returned in [false, true] {
            let expected = [
                ("up", "navigate", "main_menu", 6),
                ("confirm", "accept", "extras", 0),
                ("confirm", "accept", "credits", 0),
                ("back", "back", "extras", 0),
            ];
            let trace: Vec<_> = expected.iter().take(if returned {4} else {3}).enumerate()
                .map(|(i,(input,effect,screen,selected))| json!({"frame":(i+1)*6,
                    "input":input,"effect":effect,"screen":screen,"selected":selected,"settings":settings(62,96)})).collect();
            let current = json!({"window":{"credits_lines":12,"menu_elapsed_seconds":1.5,
                "menu":{"screen":if returned {"extras"} else {"credits"},"selected":0,
                    "has_save":false,"settings":settings(62,96)},
                "menu_session":{"probe":trace,"writes":0,"dirty":false},
                "playback":{"stage":"main_menu","muted":true,"audio":null,"menu_audio":null,
                    "menu_effect_requests":if returned {json!({"navigate":1,"accept":2,"back":1})}
                        else {json!({"navigate":1,"accept":2})}}}});
            validate_credits(&current, returned).unwrap();
            for (pointer, value) in [
                ("/window/credits_lines", json!(0)),
                ("/window/menu/screen", json!({"unavailable":"credits"})),
                ("/window/menu_session/writes", json!(1)),
                ("/window/menu_session/probe/2/effect", json!("none")),
                ("/window/playback/muted", json!(false)),
            ] {
                let mut altered = current.clone();
                *altered.pointer_mut(pointer).unwrap() = value;
                assert!(validate_credits(&altered, returned).is_err(), "{pointer}");
            }
        }
    }
    #[test]
    fn required_initial_state_and_render_diagnostics_are_not_optional() {
        assert!(validate(&window(), &png()).is_ok());
        for (field, value) in [
            ("frames_presented", json!(2)),
            ("resize_count", json!(0)),
            ("scene_uploads", json!(2)),
            ("hidden", json!(false)),
            ("textures", json!(16)),
            ("capture_non_background_pixels", json!(0)),
            ("capture_non_background_pixels", json!(201)),
            ("capture_rgba_sha256", json!("g".repeat(64))),
            ("adapter", json!("")),
        ] {
            let mut current = window();
            current[field] = value;
            assert!(validate(&current, &png()).is_err(), "{field}");
        }
        for (field, value) in [
            ("screen", json!({"unavailable":"new_game"})),
            ("selected", json!(1)),
            ("has_save", json!(true)),
        ] {
            let mut current = window();
            current["menu"][field] = value;
            assert!(validate(&current, &png()).is_err(), "{field}");
        }
    }
    #[test]
    fn capture_is_required_and_must_match_presentation_dimensions() {
        assert!(validate(&window(), &[]).is_err());
        let mut bytes = png();
        bytes[16..20].copy_from_slice(&21u32.to_be_bytes());
        assert!(validate(&window(), &bytes).is_err());
        let mut bytes = png();
        bytes[37] = 0;
        assert!(validate(&window(), &bytes).is_err());
        let mut current = window();
        current["final_size"] = json!([0, 10]);
        assert!(validate(&current, &png()).is_err());
    }
}
