//! Source-bound asset decoding and bounded native startup/media submission gate.
use crate::{cargo, executable, now, output, record, report, source_hash, write_json};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

const MOVIES: [(&str, &str, bool); 4] = [
    ("activision", "activision", false),
    ("eurocom", "eurocom", false),
    ("mgm", "mgm", false),
    ("title", "frontend", false),
];
pub fn run(disc: &Path) -> Result<()> {
    let hash = source_hash()?;
    let result = (|| -> Result<Value> {
        cargo(&[
            "test",
            "-p",
            "ge-render",
            "frontend::tests",
            "--locked",
            "--",
            "--ignored",
        ])?;
        cargo(&["build", "--release", "-p", "goldeneye", "--locked"])?;
        let folder =
            Path::new("local/startup-smoke").join(format!("{}-{}", now(), std::process::id()));
        fs::create_dir_all("local/startup-smoke")?;
        fs::create_dir(&folder)?;
        let preferences = folder.join("preferences.json");
        let settings_file = preferences
            .to_str()
            .context("startup settings path must be UTF-8")?;
        let disc = disc.to_str().context("startup disc path must be UTF-8")?;
        let inventory: Value =
            serde_json::from_slice(&output(&executable("release"), &["inspect", disc])?)?;
        ensure!(
            inventory["disc"]["game_id"] == "SJBE52" && inventory["disc"]["revision"] == 0,
            "startup smoke requires SJBE52 revision zero"
        );
        let audit_path = folder.join("media-audit.json");
        let raw = output(
            &executable("release"),
            &[
                "frontend-media-audit",
                disc,
                "--output",
                audit_path
                    .to_str()
                    .context("media audit path must be UTF-8")?,
            ],
        )?;
        let media: Value =
            serde_json::from_slice(&raw).context("frontend media audit did not emit JSON")?;
        let audit_bytes = fs::read(&audit_path).context("frontend media audit output missing")?;
        ensure!(
            serde_json::from_slice::<Value>(&audit_bytes)? == media,
            "media audit stdout/file differ"
        );
        let media_summary = validate_media(&media)?;
        let title_audit_path = folder.join("title-audit.json");
        let raw = output(
            &executable("release"),
            &[
                "frontend-title-audit",
                disc,
                "--output",
                title_audit_path
                    .to_str()
                    .context("title audit path must be UTF-8")?,
            ],
        )?;
        let title_audit: Value =
            serde_json::from_slice(&raw).context("frontend title audit did not emit JSON")?;
        let title_audit_bytes = fs::read(&title_audit_path)?;
        ensure!(
            serde_json::from_slice::<Value>(&title_audit_bytes)? == title_audit,
            "title audit stdout/file differ"
        );
        validate_title_audit(&title_audit)?;
        let mut fade_presentations = Vec::new();
        for (start_at, original_stage, next_stage, warning) in [
            ("strap-fade", "strap", "zapper", true),
            ("zapper-fade", "zapper", "activision", true),
            ("legal", "legal", "legal", false),
        ] {
            let mut previous_fade_capture = None;
            for (phase, frames) in [("first", "1"), ("middle", "8"), ("complete", "60")] {
                let label = format!("{start_at}-{phase}");
                let capture = folder.join(format!("{label}.png"));
                let window: Value = serde_json::from_slice(&output(
                    &executable("release"),
                    &[
                        "play",
                        disc,
                        "--settings-file",
                        settings_file,
                        "--start-at",
                        start_at,
                        "--frames",
                        frames,
                        "--hidden",
                        "--mute",
                        "--capture",
                        capture
                            .to_str()
                            .context("fade capture path must be UTF-8")?,
                    ],
                )?)?;
                let png = fs::read(&capture)?;
                let stage = if phase == "complete" {
                    next_stage
                } else {
                    original_stage
                };
                validate_fade(&window, &png, stage, warning, phase)?;
                if let Some(previous) = previous_fade_capture {
                    ensure!(
                        previous != window["capture_rgba_sha256"],
                        "fade captures did not change in {label}"
                    );
                }
                previous_fade_capture = Some(window["capture_rgba_sha256"].clone());
                write_json(&folder.join(format!("{label}.json")), &window)?;
                fade_presentations.push(json!({"label":label,"phase":phase,"stage":stage,"warning_fade":warning,
                    "frames":frames.parse::<u32>()?,"window":window,"capture_png_sha256":format!("{:x}",Sha256::digest(&png))}));
            }
        }
        let action_capture = folder.join("action-initialization.png");
        let action_window: Value = serde_json::from_slice(&output(
            &executable("release"),
            &[
                "play",
                disc,
                "--settings-file",
                settings_file,
                "--start-at",
                "legal",
                "--frames",
                "120",
                "--hidden",
                "--mute",
                "--capture",
                action_capture
                    .to_str()
                    .context("action capture path must be UTF-8")?,
            ],
        )?)?;
        let action_png =
            fs::read(&action_capture).context("action initialization capture missing")?;
        validate_action_initialization(&action_window, &action_png)?;
        write_json(&folder.join("action-initialization.json"), &action_window)?;
        let action_initialization = json!({"window":action_window,
            "capture_png_sha256":format!("{:x}",Sha256::digest(&action_png)),
            "scope":"Partial startup dependency: a bounded native legal-screen window retained initialized action/composite tables with nullable bindings after its entry fade. This does not establish readiness of the other native initializers, complete resource queue, frontend-menu lifecycle or the 301/30-second host legal fallback."});
        let mut title_presentations = Vec::new();
        for (label, confirm, aspect) in [
            ("title-hold", false, "wide"),
            ("title-standard", false, "standard"),
            ("title-outro", true, "wide"),
        ] {
            let capture = folder.join(format!("{label}.png"));
            let mut args = vec![
                "play",
                disc,
                "--settings-file",
                settings_file,
                "--frames",
                "120",
                "--hidden",
                "--start-at",
                "title",
                "--mute",
                "--title-aspect",
                aspect,
                "--capture",
                capture
                    .to_str()
                    .context("title visual capture path must be UTF-8")?,
            ];
            if confirm {
                args.push("--smoke-confirm-title");
            }
            let window: Value = serde_json::from_slice(&output(&executable("release"), &args)?)?;
            let png = fs::read(&capture)?;
            validate_title_visual(&window, &png, confirm, aspect)?;
            write_json(&folder.join(format!("{label}.json")), &window)?;
            title_presentations.push(json!({"label":label,"window":window,"capture_png_sha256":format!("{:x}",Sha256::digest(&png))}));
        }
        let mut presentations = Vec::new();
        for (stage, movie, muted) in MOVIES {
            let capture = folder.join(format!("{stage}.png"));
            // Include enough host playback time for the frame-11 intro cue to
            // finish naturally while the primary title music keeps consuming.
            let frames = if stage == "title" { "120" } else { "45" };
            let mut args = vec![
                "play",
                disc,
                "--settings-file",
                settings_file,
                "--frames",
                frames,
                "--hidden",
                "--start-at",
                stage,
                "--capture",
                capture
                    .to_str()
                    .context("startup capture path must be UTF-8")?,
            ];
            if muted {
                args.push("--mute");
            }
            let raw = output(&executable("release"), &args)?;
            let native: Value =
                serde_json::from_slice(&raw).context("native startup probe did not emit JSON")?;
            write_json(&folder.join(format!("{stage}.json")), &native)?;
            let png = fs::read(&capture).context("native startup capture missing")?;
            validate_native(&native, &png, stage, movie, muted)?;
            presentations.push(json!({"stage":stage,"window":native,"capture_png_sha256":format!("{:x}",Sha256::digest(&png)),"capture_png_bytes":png.len()}));
        }
        let first_capture = folder.join("title-first.png");
        let raw = output(
            &executable("release"),
            &[
                "play",
                disc,
                "--settings-file",
                settings_file,
                "--frames",
                "1",
                "--hidden",
                "--start-at",
                "title",
                "--mute",
                "--capture",
                first_capture
                    .to_str()
                    .context("title first capture path must be UTF-8")?,
            ],
        )?;
        let first: Value =
            serde_json::from_slice(&raw).context("first title probe did not emit JSON")?;
        let first_png = fs::read(&first_capture).context("first title capture missing")?;
        validate_title_first(&first, &first_png)?;
        write_json(&folder.join("title-first.json"), &first)?;
        let last = presentations
            .iter()
            .find(|probe| probe["stage"] == "title")
            .context("last title probe absent")?;
        ensure!(
            first["capture_rgba_sha256"] != last["window"]["capture_rgba_sha256"],
            "title movie first/last native captures did not change"
        );
        let title_motion = json!({"first":first,"first_capture_png_sha256":format!("{:x}",Sha256::digest(&first_png)),
            "last_capture_rgba_sha256":last["window"]["capture_rgba_sha256"],"captures_differ":true,
            "scope":"Separate bounded native title windows at one and 120 presentations; observed frame change, not original timeline/visual comparison."});
        let eos_capture = folder.join("eurocom-eos.png");
        let raw = output(
            &executable("release"),
            &[
                "play",
                disc,
                "--settings-file",
                settings_file,
                "--frames",
                "300",
                "--hidden",
                "--start-at",
                "eurocom",
                "--capture",
                eos_capture
                    .to_str()
                    .context("end-of-stream capture path must be UTF-8")?,
            ],
        )?;
        let eos: Value =
            serde_json::from_slice(&raw).context("end-of-stream probe did not emit JSON")?;
        let eos_png = fs::read(&eos_capture).context("end-of-stream capture missing")?;
        validate_native(&eos, &eos_png, "mgm", "mgm", false)?;
        validate_eos(&eos)?;
        write_json(&folder.join("eurocom-eos.json"), &eos)?;
        let eos_transition = json!({"from":"eurocom","to":"mgm","window":eos,
            "capture_png_sha256":format!("{:x}",Sha256::digest(&eos_png)),"scope":"Bounded native Eurocom-to-MGM transition with both clips decoded; verifies completion beyond the final audio interpolation endpoint."});
        Ok(
            json!({"disc":inventory["disc"],"media_audit_json_sha256":format!("{:x}",Sha256::digest(&audit_bytes)),
            "media":media_summary,"frontend_gpu_fixtures_passed":true,"presentations":presentations,"fade_presentations":fade_presentations,"title_motion":title_motion,"title_audit":title_audit,
            "title_audit_json_sha256":format!("{:x}",Sha256::digest(&title_audit_bytes)),"title_presentations":title_presentations,
            "eos_transition":eos_transition,"action_initialization":action_initialization,"reference_compared":false,
            "scope":"All startup/frontend THP video packets, startup audio, resident title music and nonlooping title-intro cue decode with exact declared sample totals. Bounded hidden logo/title presentation observes advancing frames, captures and WASAPI PCM submission; the independent title-intro voice completes while primary music consumes source frames. A deterministic 40-second source-title audit checks persistent logo geometry, repeated glint/color loops and intro/hold/outro timing; separate native windows check held title and confirmation through completed outro into the adapted menu. No confirm-button sound is inferred. No original visual/audio synchronization, measured speaker audibility, physical input or gameplay parity claim."}),
        )
    })();
    let (passed, details) = match result {
        Ok(details) => (true, details),
        Err(error) => {
            eprintln!("Native startup smoke failed: {error:#}");
            (
                false,
                json!({"message":format!("Native startup smoke failed: {error:#}"),"reference_compared":false}),
            )
        }
    };
    record("startup", hash, passed, details)?;
    report::generate()?;
    ensure!(passed, "native startup smoke failed");
    Ok(())
}
fn count(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .with_context(|| format!("startup count {key} missing"))
}
fn sha(value: &Value, key: &str) -> Result<()> {
    let text = value[key]
        .as_str()
        .with_context(|| format!("startup hash {key} missing"))?;
    ensure!(
        text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
        "startup hash {key} malformed"
    );
    Ok(())
}
fn validate_media(media: &Value) -> Result<Value> {
    ensure!(
        media["reference_compared"] == false
            && media["disc"]["game_id"] == "SJBE52"
            && media["disc"]["revision"] == 0,
        "startup media target/evidence identity differs"
    );
    let clips = media["clips"]
        .as_array()
        .context("startup media clips missing")?;
    ensure!(
        clips.len() == 4,
        "startup media requires its four declared clips"
    );
    let mut seen = BTreeSet::new();
    let mut total_video = 0u64;
    let mut total_audio = 0u64;
    let mut summaries = Vec::new();
    for clip in clips {
        let movie = clip["movie"]
            .as_str()
            .context("startup media movie identity missing")?;
        ensure!(
            ["activision", "eurocom", "mgm", "frontend"].contains(&movie) && seen.insert(movie),
            "startup media movie duplicate/unknown"
        );
        let header = &clip["header"];
        let frames = count(header, "frame_count")?;
        ensure!(
            frames > 0
                && count(clip, "frames_indexed")? == frames
                && count(clip, "video_frames_decoded")? == frames,
            "startup media did not decode complete video sequence"
        );
        let width = count(header, "width")?;
        let height = count(header, "height")?;
        ensure!(
            width > 0 && height > 0 && width <= 4096 && height <= 4096,
            "startup media dimensions out of bounds"
        );
        ensure!(
            count(clip, "rgba_bytes_decoded")?
                == frames
                    .checked_mul(width)
                    .and_then(|n| n.checked_mul(height))
                    .and_then(|n| n.checked_mul(4))
                    .context("startup RGBA count overflow")?,
            "startup media RGBA total differs"
        );
        sha(clip, "video_sequence_rgba_sha256")?;
        let samples = clip["sampled_video"]
            .as_array()
            .context("startup video samples missing")?;
        let expected: BTreeSet<u64> = [
            0,
            1.min(frames - 1),
            30.min(frames - 1),
            frames / 2,
            frames - 1,
        ]
        .into_iter()
        .collect();
        let mut actual = BTreeSet::new();
        for sample in samples {
            ensure!(
                actual.insert(count(sample, "frame")?),
                "startup duplicate video sample"
            );
            sha(sample, "video_payload_sha256")?;
            sha(sample, "rgba_sha256")?;
        }
        ensure!(
            actual == expected,
            "startup selected video sample coverage differs"
        );
        let audio = if movie == "frontend" {
            ensure!(
                header["audio"].is_null() && clip["audio"].is_null(),
                "frontend background unexpectedly contains audio"
            );
            Value::Null
        } else {
            let declared = &header["audio"];
            let audio = &clip["audio"];
            let channels = count(declared, "channels")?;
            let per_channel = count(declared, "total_samples")?;
            ensure!(
                (1..=2).contains(&channels)
                    && per_channel > 0
                    && count(audio, "frames_decoded")? == frames
                    && count(audio, "channels")? == channels
                    && count(audio, "sample_rate")? == count(declared, "sample_rate")?
                    && count(audio, "samples_per_channel")? == per_channel
                    && count(audio, "interleaved_samples")?
                        == per_channel
                            .checked_mul(channels)
                            .context("startup PCM count overflow")?,
                "startup decoded audio totals/format differ"
            );
            sha(audio, "pcm_s16le_sha256")?;
            total_audio += count(audio, "interleaved_samples")?;
            json!({"interleaved_samples":audio["interleaved_samples"],"pcm_s16le_sha256":audio["pcm_s16le_sha256"]})
        };
        total_video += frames;
        summaries.push(json!({"movie":movie,"header_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(header)?)),
            "video_frames_decoded":frames,"video_sequence_rgba_sha256":clip["video_sequence_rgba_sha256"],"sampled_frames":actual,"audio":audio}));
    }
    ensure!(
        count(media, "video_frames_decoded")? == total_video
            && count(media, "interleaved_audio_samples_decoded")? == total_audio,
        "startup media aggregate totals differ"
    );
    let title_music = validate_music_audit(&media["title_music"])?;
    let intro_cue = validate_intro_cue(&media["intro_cue"])?;
    Ok(
        json!({"clips":summaries,"video_frames_decoded":total_video,"interleaved_audio_samples_decoded":total_audio,"title_music":title_music,"intro_cue":intro_cue}),
    )
}
fn validate_intro_cue(cue: &Value) -> Result<Value> {
    let info = &cue["info"];
    ensure!(
        cue["reference_compared"] == false
            && count(info, "resource_hash")? == 0x2d500001
            && count(info, "sound_hash")? == 0x2d7011ad
            && count(info, "wave_hash")? == 0x2da01cd0
            && count(info, "sample_rate")? == 22050
            && count(info, "channels")? == 2
            && count(info, "sample_frames")? == 11256
            && count(info, "codec_raw")? == 0x13
            && info["loop_start_frame"].is_null()
            && count(cue, "source_flags_raw")? == 0x1fc
            && cue["stop_on_owner_release"] == false,
        "title-intro cue source binding/format/flags differ"
    );
    for (name, index, resource, limit) in [
        ("bank", "File_COM.bin", 0x2d500001, 8 * 1024 * 1024),
        ("sound_info", "File_USA.bin", 0x2d200000, 2 * 1024 * 1024),
    ] {
        let origin = &cue["source_origin"][name];
        let archive = origin["archive"]
            .as_str()
            .context("cue source archive missing")?;
        let prefix = index
            .strip_suffix("bin")
            .context("cue index extension missing")?;
        ensure!(
            origin["index"] == index
                && count(origin, "resource_hash")? == resource
                && (0x2c..=limit).contains(&count(origin, "bytes")?)
                && archive
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.len() >= 3
                        && suffix.bytes().all(|byte| byte.is_ascii_digit())),
            "title-intro cue source origin differs for {name}"
        );
        count(origin, "offset")?;
    }
    ensure!(
        count(cue, "decoded_frames")? == 11256
            && count(cue, "interleaved_samples")? == 22512
            && count(cue, "decoded_chunks")? > 0
            && (1..=22512).contains(&count(cue, "nonzero_samples")?)
            && (1..=32768).contains(&count(cue, "peak_absolute_sample")?),
        "title-intro cue decode totals/signal differ"
    );
    ensure!(
        count(info, "file_bytes")? == count(&cue["source_origin"]["bank"], "bytes")?,
        "cue bank origin size differs from decoded source"
    );
    sha(cue, "pcm_s16le_sha256")?;
    Ok(cue.clone())
}

fn validate_title_cue_playback(window: &Value, muted: bool) -> Result<()> {
    let playback = &window["playback"];
    ensure!(
        count(&window["title"], "cue_requests")? == 1
            && count(playback, "intro_cue_requests")? == 1,
        "title-intro source request was missing or duplicated"
    );
    if muted {
        ensure!(
            count(playback, "intro_cue_voices")? == 0
                && playback["intro_cue"].is_null()
                && playback["audio"].is_null(),
            "muted title cue opened a voice or decoded playback cue"
        );
    } else {
        validate_intro_cue(&playback["intro_cue"])?;
        let audio = &playback["audio"];
        ensure!(
            count(playback, "intro_cue_voices")? == 1
                && playback["intro_cue_gain"] == 1.0
                && count(audio, "oneshot_started")? == 1
                && count(audio, "oneshot_completed")? == 1
                && count(audio, "oneshot_active")? == 0
                && count(audio, "oneshot_output_frames")? > 0
                && count(audio, "source_frames_consumed")? > 11256 * 31999 / 22050
                && audio["source_rate"] == 31999
                && audio["source_channels"] == 2,
            "title-intro voice did not naturally complete alongside primary music"
        );
        count(audio, "mixed_clipped_frames")?;
    }
    Ok(())
}
fn validate_music_info(info: &Value) -> Result<()> {
    ensure!(
        count(info, "resource_hash")? == 0x2d500017
            && count(info, "sound_hash")? == 0x2d700109
            && count(info, "wave_hash")? == 0x2da021bf
            && count(info, "channels")? == 2
            && count(info, "sample_rate")? == 31999
            && count(info, "sample_frames")? > 0
            && info["loop_start_frame"] == 0,
        "startup title music binding/format differs"
    );
    Ok(())
}
fn validate_music_audit(music: &Value) -> Result<Value> {
    ensure!(
        music["reference_compared"] == false,
        "startup title music reference scope differs"
    );
    let info = &music["info"];
    validate_music_info(info)?;
    let frames = count(info, "sample_frames")?;
    let values = frames
        .checked_mul(2)
        .context("title music sample count overflow")?;
    ensure!(
        count(music, "decoded_sample_frames")? == frames
            && count(music, "interleaved_samples")? == values
            && count(music, "nonzero_samples")? > 0
            && count(music, "nonzero_samples")? <= values
            && (1..=32768).contains(&count(music, "peak_absolute_sample")?),
        "startup title music decode totals/signal differ"
    );
    sha(music, "pcm_s16le_sha256")?;
    Ok(
        json!({"resource_hash":info["resource_hash"],"sound_hash":info["sound_hash"],"wave_hash":info["wave_hash"],
        "sample_rate":info["sample_rate"],"channels":info["channels"],"sample_frames":frames,"decoded_sample_frames":music["decoded_sample_frames"],
        "interleaved_samples":values,"nonzero_samples":music["nonzero_samples"],"peak_absolute_sample":music["peak_absolute_sample"],
        "loop_start_frame":info["loop_start_frame"],"pcm_s16le_sha256":music["pcm_s16le_sha256"]}),
    )
}
fn validate_native(
    window: &Value,
    png: &[u8],
    stage: &str,
    movie: &str,
    muted: bool,
) -> Result<()> {
    ensure!(
        window["hidden"] == true
            && count(window, "frames_presented")? >= 45
            && window["scene_uploads"] == 1
            && count(window, "textures")? >= 17,
        "startup native presentation/upload/asset checks failed"
    );
    for field in ["adapter", "backend", "surface_format"] {
        ensure!(
            window[field]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty()),
            "startup renderer identity missing {field}"
        );
    }
    let playback = &window["playback"];
    ensure!(
        playback["stage"] == stage
            && playback["movie"] == movie
            && count(playback, "decoded_frames")? >= 2
            && count(playback, "video_frame")? > 0
            && playback["muted"] == muted
            && playback["clips_decoded"][movie]
                .as_u64()
                .is_some_and(|n| n >= 2),
        "startup stage/advancing media checks failed"
    );
    count(playback, "transitions")?;
    if muted {
        ensure!(
            playback["audio"].is_null(),
            "muted startup probe opened audio unexpectedly"
        );
    } else {
        let audio = &playback["audio"];
        ensure!(
            audio["backend"] == "WASAPI"
                && audio["device"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty())
                && count(audio, "source_frames_consumed")? > 0
                && count(audio, "output_frames_submitted")? > 0
                && count(audio, "device_errors")? == 0
                && audio["paused"] == false,
            "startup native PCM submission checks failed"
        );
    }
    if stage == "title" && !muted {
        let music = &playback["music"];
        validate_music_info(&music["info"])?;
        ensure!(
            count(music, "decoded_frames")? > 0
                && playback["audio"]["source_rate"] == 31999
                && playback["audio"]["source_channels"] == 2,
            "startup title music has no decoded/submitted source frames"
        );
        validate_title_cue_playback(window, false)?;
    }
    validate_capture(window, png)
}
fn validate_title_first(window: &Value, png: &[u8]) -> Result<()> {
    ensure!(
        window["hidden"] == true
            && count(window, "frames_presented")? >= 1
            && window["scene_uploads"] == 1
            && count(window, "textures")? >= 17,
        "first title presentation checks failed"
    );
    let playback = &window["playback"];
    ensure!(
        playback["stage"] == "title"
            && playback["movie"] == "frontend"
            && count(playback, "decoded_frames")? >= 1
            && playback["muted"] == true
            && playback["audio"].is_null(),
        "first title media/mute checks failed"
    );
    count(playback, "video_frame")?;
    validate_capture(window, png)
}
fn validate_fade(
    window: &Value,
    png: &[u8],
    stage: &str,
    warning: bool,
    phase: &str,
) -> Result<()> {
    let frames = match phase {
        "first" => 1,
        "middle" => 8,
        "complete" => 60,
        _ => anyhow::bail!("unknown fade probe phase"),
    };
    ensure!(
        window["hidden"] == true
            && count(window, "frames_presented")? == frames
            && window["scene_uploads"] == 1
            && window["playback"]["stage"] == stage
            && window["playback"]["muted"] == true
            && window["playback"]["audio"].is_null(),
        "fade probe presentation/stage identity differs"
    );
    let fade = &window["playback"]["fade"];
    let color = fade["color"].as_array().context("fade color absent")?;
    ensure!(
        color.len() == 4 && color[..3] == [json!(0), json!(0), json!(0)],
        "fade is not black RGBA"
    );
    let alpha = color[3].as_u64().context("fade alpha absent")?;
    ensure!(alpha <= 255, "fade alpha is outside a byte");
    if phase != "complete" {
        ensure!(
            fade["duration_seconds"] == 0.5 && fade["first_update_pending"] == false,
            "fade source duration or first-update scheduling differs"
        );
    }
    match phase {
        "first" => {
            ensure!(
                fade["active"] == true
                    && fade["elapsed_seconds"] == 0.
                    && alpha == if warning { 0 } else { 255 }
                    && window["playback"]["warning_fading"] == warning,
                "fade first update did not retain the start color/stage"
            );
            if !warning {
                ensure!(
                    count(window, "capture_non_background_pixels")? == 0,
                    "legal fade-in first capture is not completely black"
                );
            }
        }
        "middle" => {
            ensure!(
                fade["active"] == true
                    && alpha > 0
                    && alpha < 255
                    && fade["elapsed_seconds"]
                        .as_f64()
                        .is_some_and(|s| s > 0. && s < 0.5)
                    && window["playback"]["warning_fading"] == warning
                    && count(window, "capture_non_background_pixels")? > 0,
                "fade intermediate color/state or retained artwork missing"
            );
        }
        "complete" => {
            ensure!(
                fade["active"] == false
                    && alpha == 0
                    && window["playback"]["warning_fading"] == false,
                "fade did not auto-clear before stage completion"
            );
        }
        _ => unreachable!(),
    }
    validate_capture(window, png)
}
fn validate_action_initialization(window: &Value, png: &[u8]) -> Result<()> {
    let playback = &window["playback"];
    ensure!(
        window["hidden"] == true
            && count(window, "frames_presented")? == 120
            && window["scene_uploads"] == 1
            && playback["stage"] == "legal"
            && playback["muted"] == true
            && playback["audio"].is_null()
            && playback["fade"]["active"] == false
            && playback["fade"]["color"] == json!([0, 0, 0, 0]),
        "action initializer probe presentation/fade/stage differs"
    );
    let resources = &playback["startup_resources"];
    ensure!(
        resources["state"] == "initialized"
            && resources["actions_ready"] == true
            && resources.get("error") == Some(&Value::Null)
            && count(resources, "resource_hash")? == 0x01090001
            && count(resources, "action_sheet_hash")? == 0x14000017
            && count(resources, "composite_sheet_hash")? == 0x1400000b
            && count(resources, "actions")? > 0
            && count(resources, "composites")? > 0,
        "action initializer did not retain initialized nonempty source tables"
    );
    ensure!(
        count(resources, "resolved_slots")?
            .checked_add(count(resources, "unresolved_slots")?)
            .context("action slot accounting overflow")?
            == count(resources, "composites")?
                .checked_mul(4)
                .context("action composite slot accounting overflow")?,
        "action initializer nullable slot accounting differs"
    );
    validate_capture(window, png)
}

fn validate_title_visual(window: &Value, png: &[u8], confirmed: bool, aspect: &str) -> Result<()> {
    ensure!(
        window["hidden"] == true
            && count(window, "frames_presented")? >= 120
            && window["scene_uploads"] == 1
            && count(window, "textures")? >= 20,
        "source-title presentation/assets missing"
    );
    validate_title_cue_playback(window, true)?;
    let title = &window["title"];
    ensure!(
        count(title, "scripts")? >= 13
            && title["camera_aspect"] == aspect
            && title["camera_fov_degrees"]
                .as_f64()
                .is_some_and(|n| n > 1.99 && n < 2.)
            && count(title, "sorted_entities")? == count(title, "entities")?
            && window["playback"]["muted"] == true
            && window["title_confirmation_accepted"] == confirmed,
        "source-title graph/confirmation identity differs"
    );
    if confirmed {
        ensure!(
            title["phase"] == "complete"
                && title["frame"] == 49.
                && window["playback"]["stage"] == "main_menu",
            "title outro did not finish before entering menu"
        );
    } else {
        ensure!(
            title["phase"] == "hold"
                && title["frame"] == 29.
                && count(title, "entities")? > 0
                && count(title, "draw_triangles")? > 0
                && title["prompt_alpha"].as_f64().is_some_and(|n| n > 0.)
                && window["playback"]["stage"] == "title",
            "title artwork disappeared instead of holding"
        );
    }
    validate_capture(window, png)
}
fn validate_title_audit(audit: &Value) -> Result<()> {
    ensure!(
        audit["resource_hash"] == 0x01080004u32
            && audit["root_script"] == 0x04001bb6u32
            && audit["reference_compared"] == false
            && audit["morph_color_metric"] == "per_mesh_rgb_patterns_no_rgb_tracks"
            && audit["camera_aspect"] == "wide"
            && audit["camera_fov_degrees"]
                .as_f64()
                .is_some_and(|n| n > 1.99 && n < 2.)
            && audit["sample_rate_hz"] == 60
            && audit["source_frames_per_second"] == 60.
            && audit["scheduled_seconds"] == 40.
            && audit["samples"] == 2401
            && audit["intro"] == json!({"start":0.,"end":29.})
            && audit["outro"] == json!({"start":30.,"end":49.}),
        "title audit source identity/segment coverage differs"
    );
    for key in [
        "hold_samples",
        "minimum_hold_logo_triangles",
        "maximum_color_frame_meshes",
        "hold_morph_color_changes",
        "hold_layer_changes",
        "outro_samples",
    ] {
        ensure!(count(audit, key)? > 0, "title audit did not exercise {key}");
    }
    ensure!(
        audit["confirmed_seconds"] == 40.
            && audit["completed_frame"] == 49.
            && audit["completed_seconds"]
                .as_f64()
                .is_some_and(|s| s > 40. && s < 41.)
            && audit["deterministic_replay_verified"] == true
            && count(audit, "deterministic_replay_samples")?
                == count(audit, "samples")? + count(audit, "outro_samples")?,
        "title audit hold/outro/replay checks failed"
    );
    let windows = audit["windows"]
        .as_array()
        .context("title loop windows missing")?;
    ensure!(
        windows.len() == 4,
        "title loop audit did not cover four windows"
    );
    for (index, window) in windows.iter().enumerate() {
        ensure!(
            window["start_seconds"] == (index * 10) as f64
                && window["end_seconds"] == ((index + 1) * 10) as f64
                && count(window, "hold_samples")? > 0
                && count(window, "morph_color_changes")? > 0
                && count(window, "distinct_morph_color_hashes")? > 1,
            "title color animation stopped in window {index}"
        );
    }
    sha(audit, "scheduled_sequence_sha256")?;
    sha(audit, "outro_sequence_sha256")?;
    Ok(())
}
fn validate_eos(window: &Value) -> Result<()> {
    let playback = &window["playback"];
    ensure!(
        count(window, "frames_presented")? >= 300
            && playback["stage"] == "mgm"
            && playback["movie"] == "mgm"
            && playback["clips_decoded"]["eurocom"]
                .as_u64()
                .is_some_and(|n| n > 0)
            && playback["clips_decoded"]["mgm"]
                .as_u64()
                .is_some_and(|n| n > 0),
        "startup Eurocom-to-MGM completion not observed"
    );
    Ok(())
}
fn validate_capture(window: &Value, png: &[u8]) -> Result<()> {
    sha(window, "capture_rgba_sha256")?;
    let size = window["final_size"]
        .as_array()
        .context("startup capture dimensions missing")?;
    ensure!(size.len() == 2, "startup capture size shape differs");
    let width = size[0].as_u64().context("startup capture width missing")?;
    let height = size[1].as_u64().context("startup capture height missing")?;
    ensure!(
        width > 0
            && height > 0
            && width <= 4096
            && height <= 4096
            && count(window, "capture_non_background_pixels")? <= width * height,
        "startup capture dimensions/pixel count invalid"
    );
    ensure!(
        png.len() >= 45
            && png[..8] == *b"\x89PNG\r\n\x1a\n"
            && png[8..12] == 13u32.to_be_bytes()
            && png[12..16] == *b"IHDR"
            && png[png.len() - 12..png.len() - 8] == [0; 4]
            && png[png.len() - 8..png.len() - 4] == *b"IEND",
        "startup capture PNG missing/truncated"
    );
    ensure!(
        u64::from(u32::from_be_bytes(png[16..20].try_into()?)) == width
            && u64::from(u32::from_be_bytes(png[20..24].try_into()?)) == height,
        "startup PNG dimensions differ"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fade_window(stage: &str, warning: bool, phase: &str) -> Value {
        let mut window = native(true);
        let (frames, alpha, elapsed, active) = match phase {
            "first" => (1, if warning { 0 } else { 255 }, 0., true),
            "middle" => (8, 127, 0.25, true),
            "complete" => (60, 0, 0.5, false),
            _ => unreachable!(),
        };
        window["frames_presented"] = json!(frames);
        window["capture_non_background_pixels"] =
            json!(if phase == "first" && !warning { 0 } else { 1 });
        window["playback"]["stage"] = json!(stage);
        window["playback"]["warning_fading"] = json!(warning && active);
        window["playback"]["fade"] = json!({"color":[0,0,0,alpha],"active":active,
            "first_update_pending":false,"elapsed_seconds":elapsed,"duration_seconds":0.5});
        window
    }
    #[test]
    fn native_warning_fade_gate_requires_retained_stage_partial_opacity_and_auto_clear() {
        for phase in ["first", "middle", "complete"] {
            let stage = if phase == "complete" {
                "zapper"
            } else {
                "strap"
            };
            validate_fade(&fade_window(stage, true, phase), &png(), stage, true, phase).unwrap();
        }
        for (pointer, value) in [
            ("/playback/stage", json!("zapper")),
            ("/playback/fade/color/3", json!(0)),
            ("/playback/fade/active", json!(false)),
            ("/playback/fade/duration_seconds", json!(21.)),
            ("/playback/warning_fading", json!(false)),
        ] {
            let mut window = fade_window("strap", true, "middle");
            *window.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_fade(&window, &png(), "strap", true, "middle").is_err(),
                "{pointer}"
            );
        }
    }
    #[test]
    fn native_legal_fade_gate_requires_initial_black_capture_and_visible_midpoint() {
        validate_fade(
            &fade_window("legal", false, "first"),
            &png(),
            "legal",
            false,
            "first",
        )
        .unwrap();
        let mut first = fade_window("legal", false, "first");
        first["capture_non_background_pixels"] = json!(1);
        assert!(validate_fade(&first, &png(), "legal", false, "first").is_err());
        let mut middle = fade_window("legal", false, "middle");
        validate_fade(&middle, &png(), "legal", false, "middle").unwrap();
        middle["capture_non_background_pixels"] = json!(0);
        assert!(validate_fade(&middle, &png(), "legal", false, "middle").is_err());
    }
    fn title_audit() -> Value {
        json!({"resource_hash":0x01080004u32,"root_script":0x04001bb6u32,"reference_compared":false,
            "morph_color_metric":"per_mesh_rgb_patterns_no_rgb_tracks",
            "camera_aspect":"wide","camera_fov_degrees":1.9972616,
            "sample_rate_hz":60,"source_frames_per_second":60.,"scheduled_seconds":40.,"samples":2401,
            "intro":{"start":0.,"end":29.},"outro":{"start":30.,"end":49.},
            "hold_samples":2371,"minimum_hold_logo_triangles":2,"maximum_color_frame_meshes":1,
            "hold_morph_color_changes":4,"hold_layer_changes":4,"outro_samples":20,
            "confirmed_seconds":40.,"completed_seconds":40.333333,"completed_frame":49.,
            "deterministic_replay_verified":true,"deterministic_replay_samples":2421,
            "windows":(0..4).map(|i|json!({"start_seconds":f64::from(i*10),"end_seconds":f64::from((i+1)*10),
                "hold_samples":1,"morph_color_changes":1,"distinct_morph_color_hashes":2})).collect::<Vec<_>>(),
            "scheduled_sequence_sha256":"ab".repeat(32),"outro_sequence_sha256":"cd".repeat(32)})
    }
    #[test]
    fn title_loop_gate_rejects_late_freeze_missing_logo_and_incomplete_outro_or_replay() {
        validate_title_audit(&title_audit()).unwrap();
        for (pointer, value) in [
            ("/minimum_hold_logo_triangles", json!(0)),
            ("/windows/3/morph_color_changes", json!(0)),
            ("/windows/2/distinct_morph_color_hashes", json!(1)),
            ("/completed_seconds", json!(40.)),
            ("/completed_frame", json!(30.)),
            ("/deterministic_replay_samples", json!(2401)),
            ("/deterministic_replay_verified", json!(false)),
            ("/reference_compared", json!(true)),
            ("/morph_color_metric", json!("ordered_rgba_mesh_population")),
        ] {
            let mut report = title_audit();
            *report.pointer_mut(pointer).unwrap() = value;
            assert!(validate_title_audit(&report).is_err(), "{pointer}");
        }
    }
    #[test]
    fn title_presentation_gate_requires_hold_then_completed_outro_before_menu() {
        let mut window = native(true);
        window["frames_presented"] = json!(120);
        window["textures"] = json!(20);
        window["title_confirmation_accepted"] = json!(false);
        window["title"] = json!({"scripts":13,"frame":29.,"phase":"hold","entities":1,"sorted_entities":1,
            "camera_aspect":"wide","camera_fov_degrees":1.9972616,"draw_triangles":2,"prompt_alpha":1.,"cue_requests":1});
        validate_title_visual(&window, &png(), false, "wide").unwrap();
        assert!(validate_title_visual(&window, &png(), false, "standard").is_err());
        let mut missing = window.clone();
        missing["title"]["draw_triangles"] = json!(0);
        assert!(validate_title_visual(&missing, &png(), false, "wide").is_err());
        window["title_confirmation_accepted"] = json!(true);
        window["playback"]["stage"] = json!("main_menu");
        assert!(validate_title_visual(&window, &png(), true, "wide").is_err());
        window["title"]["phase"] = json!("complete");
        window["title"]["frame"] = json!(49.);
        validate_title_visual(&window, &png(), true, "wide").unwrap();
    }
    fn action_window() -> Value {
        let mut window = native(true);
        window["frames_presented"] = json!(120);
        window["playback"]["stage"] = json!("legal");
        window["playback"]["fade"] = json!({"active":false,"color":[0,0,0,0]});
        // Authored counts deliberately differ from the supplied corpus.
        window["playback"]["startup_resources"] = json!({"state":"initialized","actions_ready":true,
            "resource_hash":0x01090001,"action_sheet_hash":0x14000017,"composite_sheet_hash":0x1400000b,
            "actions":3,"composites":2,"resolved_slots":5,"unresolved_slots":3,"error":null});
        window
    }
    #[test]
    fn action_gate_requires_actual_initialized_tables_and_expected_resource_identity() {
        validate_action_initialization(&action_window(), &png()).unwrap();
        for (field, value) in [
            ("state", json!("loading")),
            ("actions_ready", json!(false)),
            ("resource_hash", json!(0)),
            ("action_sheet_hash", json!(0)),
            ("composite_sheet_hash", json!(0)),
            ("actions", json!(0)),
            ("composites", json!(0)),
            ("error", json!("worker read failed")),
        ] {
            let mut window = action_window();
            window["playback"]["startup_resources"][field] = value;
            assert!(
                validate_action_initialization(&window, &png()).is_err(),
                "{field}"
            );
        }
        let mut window = action_window();
        window["playback"]["startup_resources"] = Value::Null;
        assert!(validate_action_initialization(&window, &png()).is_err());
    }
    #[test]
    fn action_gate_preserves_nullable_slot_totals_and_requires_postfade_legal_window() {
        let mut all_missing = action_window();
        all_missing["playback"]["startup_resources"]["resolved_slots"] = json!(0);
        all_missing["playback"]["startup_resources"]["unresolved_slots"] = json!(8);
        validate_action_initialization(&all_missing, &png()).unwrap();
        for (pointer, value) in [
            ("/playback/startup_resources/unresolved_slots", json!(0)),
            ("/playback/startup_resources/resolved_slots", json!(9)),
            ("/playback/startup_resources/composites", json!(u64::MAX)),
            ("/playback/fade/active", json!(true)),
            ("/playback/fade/color/3", json!(127)),
            ("/playback/stage", json!("title")),
            ("/playback/muted", json!(false)),
            ("/frames_presented", json!(60)),
        ] {
            let mut window = action_window();
            *window.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_action_initialization(&window, &png()).is_err(),
                "{pointer}"
            );
        }
    }
    fn png() -> Vec<u8> {
        let mut bytes = vec![0; 45];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&20u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&10u32.to_be_bytes());
        bytes[37..41].copy_from_slice(b"IEND");
        bytes
    }
    fn native(muted: bool) -> Value {
        let (stage, movie) = if muted {
            ("title", "frontend")
        } else {
            ("activision", "activision")
        };
        let mut clips = serde_json::Map::new();
        clips.insert(movie.into(), json!(2));
        json!({"hidden":true,"frames_presented":45,"scene_uploads":1,"textures":17,"adapter":"Authored adapter","backend":"Authored backend","surface_format":"Rgba8Unorm",
            "capture_rgba_sha256":"ab".repeat(32),"capture_non_background_pixels":0,"final_size":[20,10],
            "playback":{"stage":stage,"movie":movie,"video_frame":1,"decoded_frames":2,"transitions":0,"clips_decoded":clips,"muted":muted,
                "intro_cue_requests":1,"intro_cue_voices":0,"intro_cue":null,
                "audio":if muted {Value::Null}else{json!({"backend":"WASAPI","device":"Authored device","source_frames_consumed":1,"output_frames_submitted":2,"device_errors":0,"paused":false})}}})
    }
    fn music_info() -> Value {
        json!({"resource_hash":0x2d500017u32,"sound_hash":0x2d700109u32,"wave_hash":0x2da021bfu32,
        "channels":2,"sample_rate":31999,"sample_frames":2,"loop_start_frame":0})
    }
    fn music_audit() -> Value {
        json!({"info":music_info(),"reference_compared":false,"decoded_sample_frames":2,"interleaved_samples":4,
        "nonzero_samples":2,"peak_absolute_sample":100,"pcm_s16le_sha256":"ab".repeat(32)})
    }
    fn cue_audit() -> Value {
        json!({"info":{"resource_hash":0x2d500001u32,"sound_hash":0x2d7011adu32,"wave_hash":0x2da01cd0u32,
            "file_bytes":4096,"sample_rate":22050,"channels":2,"sample_frames":11256,"codec_raw":0x13,"loop_start_frame":null},
            "decoded_frames":11256,"decoded_chunks":1,"interleaved_samples":22512,"nonzero_samples":20000,"peak_absolute_sample":123,
            "pcm_s16le_sha256":"ab".repeat(32),"source_flags_raw":0x1fc,"stop_on_owner_release":false,"reference_compared":false,
            "source_origin":{"bank":{"index":"File_COM.bin","archive":"File_COM.000","resource_hash":0x2d500001u32,"offset":64,"bytes":4096},
                "sound_info":{"index":"File_USA.bin","archive":"File_USA.000","resource_hash":0x2d200000u32,"offset":128,"bytes":256}}})
    }
    fn title_audio() -> Value {
        let mut window = native(false);
        let playback = &mut window["playback"];
        playback["stage"] = json!("title");
        playback["movie"] = json!("frontend");
        playback["clips_decoded"] = json!({"frontend":2});
        playback["audio"]["source_rate"] = json!(31999);
        playback["audio"]["source_channels"] = json!(2);
        playback["music"] = json!({"info":music_info(),"decoded_frames":14});
        playback["intro_cue"] = cue_audit();
        playback["intro_cue_voices"] = json!(1);
        playback["intro_cue_gain"] = json!(1.0);
        playback["audio"]["source_frames_consumed"] = json!(32000);
        playback["audio"]["oneshot_started"] = json!(1);
        playback["audio"]["oneshot_completed"] = json!(1);
        playback["audio"]["oneshot_active"] = json!(0);
        playback["audio"]["oneshot_output_frames"] = json!(24504);
        playback["audio"]["mixed_clipped_frames"] = json!(0);
        window["title"] = json!({"cue_requests":1});
        window
    }
    fn media() -> Value {
        let clips:Vec<_>=["activision","eurocom","mgm","frontend"].into_iter().map(|movie| {
            let audio=if movie=="frontend" {Value::Null}else{json!({"frames_decoded":2,"channels":2,"sample_rate":32000,"samples_per_channel":1,"interleaved_samples":2,"pcm_s16le_sha256":"ab".repeat(32)})};
            json!({"movie":movie,"header":{"frame_count":2,"width":2,"height":1,"audio":if movie=="frontend" {Value::Null}else{json!({"channels":2,"sample_rate":32000,"total_samples":1})}},
                "frames_indexed":2,"video_frames_decoded":2,"rgba_bytes_decoded":16,"video_sequence_rgba_sha256":"ab".repeat(32),"audio":audio,
                "sampled_video":[{"frame":0,"video_payload_sha256":"ab".repeat(32),"rgba_sha256":"cd".repeat(32)},{"frame":1,"video_payload_sha256":"ab".repeat(32),"rgba_sha256":"cd".repeat(32)}]})
        }).collect();
        json!({"reference_compared":false,"disc":{"game_id":"SJBE52","revision":0},"clips":clips,"video_frames_decoded":8,"interleaved_audio_samples_decoded":6,"title_music":music_audit(),"intro_cue":cue_audit()})
    }
    #[test]
    fn native_audio_requires_consumption_submission_unpaused_device_and_moving_video() {
        validate_native(&native(false), &png(), "activision", "activision", false).unwrap();
        for (field, value) in [
            ("backend", json!("")),
            ("device", json!("")),
            ("source_frames_consumed", json!(0)),
            ("output_frames_submitted", json!(0)),
            ("device_errors", json!(1)),
            ("paused", json!(true)),
        ] {
            let mut window = native(false);
            window["playback"]["audio"][field] = value;
            assert!(
                validate_native(&window, &png(), "activision", "activision", false).is_err(),
                "{field}"
            );
        }
        for (field, value) in [
            ("video_frame", json!(0)),
            ("decoded_frames", json!(1)),
            ("stage", json!("strap")),
            ("movie", Value::Null),
        ] {
            let mut window = native(false);
            window["playback"][field] = value;
            assert!(
                validate_native(&window, &png(), "activision", "activision", false).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn title_requires_mute_and_capture_dimensions_without_faking_audio() {
        validate_native(&native(true), &png(), "title", "frontend", true).unwrap();
        validate_title_first(&native(true), &png()).unwrap();
        let mut window = native(true);
        window["playback"]["audio"] = json!({});
        assert!(validate_native(&window, &png(), "title", "frontend", true).is_err());
        assert!(validate_native(&native(true), &[], "title", "frontend", true).is_err());
        let mut bytes = png();
        bytes[16..20].copy_from_slice(&21u32.to_be_bytes());
        assert!(validate_native(&native(true), &bytes, "title", "frontend", true).is_err());
    }
    #[test]
    fn full_media_requires_distinct_complete_clips_samples_and_exact_audio_totals() {
        let result = validate_media(&media()).unwrap();
        assert_eq!(result["video_frames_decoded"], 8);
        let mut current = media();
        current["clips"][0]["video_frames_decoded"] = json!(1);
        assert!(validate_media(&current).is_err());
        let mut current = media();
        current["clips"][1]["movie"] = json!("activision");
        assert!(validate_media(&current).is_err());
        let mut current = media();
        current["clips"][0]["sampled_video"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(validate_media(&current).is_err());
        let mut current = media();
        current["clips"][0]["audio"]["samples_per_channel"] = json!(0);
        assert!(validate_media(&current).is_err());
        let mut current = media();
        current["interleaved_audio_samples_decoded"] = json!(7);
        assert!(validate_media(&current).is_err());
    }
    #[test]
    fn title_music_requires_actual_binding_decode_signal_and_native_source_consumption() {
        validate_native(&title_audio(), &png(), "title", "frontend", false).unwrap();
        let summary = validate_music_audit(&music_audit()).unwrap();
        assert_eq!(summary["sample_rate"], 31999);
        for (field, value) in [
            ("sound_hash", json!(0)),
            ("wave_hash", json!(0)),
            ("sample_rate", json!(32000)),
            ("channels", json!(1)),
        ] {
            let mut window = title_audio();
            window["playback"]["music"]["info"][field] = value;
            assert!(
                validate_native(&window, &png(), "title", "frontend", false).is_err(),
                "{field}"
            );
        }
        let mut window = title_audio();
        window["playback"]["music"]["decoded_frames"] = json!(0);
        assert!(validate_native(&window, &png(), "title", "frontend", false).is_err());
        for (field, value) in [
            ("decoded_sample_frames", json!(1)),
            ("nonzero_samples", json!(0)),
            ("pcm_s16le_sha256", json!("missing")),
        ] {
            let mut audit = music_audit();
            audit[field] = value;
            assert!(validate_music_audit(&audit).is_err(), "{field}");
        }
        assert!(validate_music_audit(&Value::Null).is_err());
    }
    #[test]
    fn intro_cue_requires_bound_wave_real_metadata_flags_and_complete_nonlooping_decode() {
        validate_intro_cue(&cue_audit()).unwrap();
        for (pointer, value) in [
            ("/info/resource_hash", json!(0x2d500017)),
            ("/info/sound_hash", json!(0)),
            ("/info/wave_hash", json!(0)),
            ("/info/sample_rate", json!(31999)),
            ("/info/channels", json!(1)),
            ("/info/loop_start_frame", json!(0)),
            ("/source_flags_raw", json!(0)),
            ("/stop_on_owner_release", json!(true)),
            ("/source_origin/sound_info", Value::Null),
            ("/source_origin/bank/index", json!("File_USA.bin")),
            ("/source_origin/bank/archive", json!("File_COM.bin")),
            ("/source_origin/bank/bytes", json!(1)),
            ("/source_origin/sound_info/resource_hash", json!(0)),
            ("/decoded_frames", json!(11255)),
            ("/interleaved_samples", json!(22511)),
            ("/nonzero_samples", json!(0)),
            ("/pcm_s16le_sha256", json!("missing")),
        ] {
            let mut cue = cue_audit();
            *cue.pointer_mut(pointer).unwrap() = value;
            assert!(validate_intro_cue(&cue).is_err(), "{pointer}");
        }
        assert!(validate_intro_cue(&Value::Null).is_err());
        let mut report = media();
        report["intro_cue"] = Value::Null;
        assert!(validate_media(&report).is_err());
    }
    #[test]
    fn intro_voice_requires_one_request_source_gain_completion_and_primary_continuity() {
        validate_title_cue_playback(&title_audio(), false).unwrap();
        for (pointer, value) in [
            ("/title/cue_requests", json!(0)),
            ("/playback/intro_cue_requests", json!(2)),
            ("/playback/intro_cue_voices", json!(0)),
            ("/playback/intro_cue_gain", json!(0.5)),
            ("/playback/audio/oneshot_started", json!(2)),
            ("/playback/audio/oneshot_completed", json!(0)),
            ("/playback/audio/oneshot_active", json!(1)),
            ("/playback/audio/oneshot_output_frames", json!(0)),
            ("/playback/audio/source_frames_consumed", json!(1)),
            ("/playback/audio/source_rate", json!(22050)),
            ("/playback/audio/source_channels", json!(1)),
        ] {
            let mut window = title_audio();
            *window.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_title_cue_playback(&window, false).is_err(),
                "{pointer}"
            );
        }
        let mut muted = native(true);
        muted["title"] = json!({"cue_requests":1});
        validate_title_cue_playback(&muted, true).unwrap();
        muted["playback"]["intro_cue_voices"] = json!(1);
        assert!(validate_title_cue_playback(&muted, true).is_err());
    }
    #[test]
    fn real_completion_probe_requires_both_clip_identities_and_new_stage() {
        let mut window = native(false);
        window["frames_presented"] = json!(300);
        window["playback"]["stage"] = json!("mgm");
        window["playback"]["movie"] = json!("mgm");
        window["playback"]["clips_decoded"] = json!({"eurocom":10,"mgm":2});
        validate_eos(&window).unwrap();
        window["playback"]["clips_decoded"]["eurocom"] = json!(0);
        assert!(validate_eos(&window).is_err());
        window["playback"]["clips_decoded"]["eurocom"] = json!(10);
        window["playback"]["stage"] = json!("eurocom");
        assert!(validate_eos(&window).is_err());
    }
}
