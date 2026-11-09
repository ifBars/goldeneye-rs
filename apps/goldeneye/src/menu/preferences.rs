//! Versioned Windows preferences, separate from the game's Wii save/profile data.
use anyhow::{Context, Result, ensure};
use ge_core::frontend::settings::Settings;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: u64 = 4096;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    settings: Settings,
}

pub struct Preferences {
    path: PathBuf,
    saved: Settings,
}
impl Preferences {
    pub fn open(explicit: Option<PathBuf>) -> Result<(Self, Settings)> {
        let path = match explicit {
            Some(path) => path,
            None => PathBuf::from(
                std::env::var_os("LOCALAPPDATA")
                    .context("LOCALAPPDATA is unavailable; pass --settings-file <path>")?,
            )
            .join("GoldenEyeRewrite")
            .join("frontend-settings.json"),
        };
        let settings = match fs::File::open(&path) {
            Ok(file) => {
                use std::io::Read;
                ensure!(
                    file.metadata()?.len() <= MAX_BYTES,
                    "settings file exceeds 4096 bytes: {}",
                    path.display()
                );
                let mut bytes = Vec::new();
                file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_BYTES,
                    "settings file exceeds budget"
                );
                let doc: Document = serde_json::from_slice(&bytes).with_context(|| {
                    format!(
                        "invalid settings file {}; preserve or move it before restarting",
                        path.display()
                    )
                })?;
                ensure!(
                    doc.schema_version == 1,
                    "unsupported settings schema in {}",
                    path.display()
                );
                doc.settings
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(error) => {
                return Err(error).with_context(|| format!("reading settings {}", path.display()));
            }
        };
        Ok((
            Self {
                path,
                saved: settings,
            },
            settings,
        ))
    }
    /// Write only changed settings. A same-directory rename replaces the prior
    /// document only after a complete, flushed write; failed writes preserve it.
    pub fn save(&mut self, settings: Settings) -> Result<bool> {
        if settings == self.saved {
            return Ok(false);
        }
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        fs::create_dir_all(parent)?;
        let name = self
            .path
            .file_name()
            .context("settings path has no filename")?
            .to_string_lossy();
        let temporary = parent.join(format!(
            ".{name}.{}-{}.tmp",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let bytes = serde_json::to_vec_pretty(&Document {
            schema_version: 1,
            settings,
        })?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.with_context(|| format!("saving settings {}", self.path.display()))?;
        self.saved = settings;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "tests/preferences.rs"]
mod tests;
