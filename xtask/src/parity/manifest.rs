use super::receipt::{Artifact, Failure, Tracked, read_artifact, resolve_local};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

pub(super) const MAX_CASES: usize = 128;
pub(super) const MAX_SAMPLES: usize = 512;
pub(super) const MAX_JSON: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseSet {
    pub schema_version: u32,
    pub cases: Vec<Case>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub samples: Vec<Sample>,
    pub tolerances: Tolerances,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Sample {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub roi: Option<Roi>,
    pub minimum_nonblack_fraction: f64,
    pub minimum_spatial_range: u8,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Roi {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tolerances {
    pub max_mae: f64,
    pub bad_pixel_delta: u8,
    pub max_bad_pixel_fraction: f64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pack {
    pub schema_version: u32,
    pub role: String,
    pub case_set_sha256: String,
    pub disc: Disc,
    pub producer: Producer,
    pub configuration: Artifact,
    pub input_contract: Artifact,
    pub source_hash: Option<String>,
    pub captures: Vec<Capture>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Disc {
    pub game_id: String,
    pub revision: u8,
    pub image_sha256: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Producer {
    pub kind: String,
    pub version: String,
    pub executable: Artifact,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Capture {
    pub case_id: String,
    pub sample_id: String,
    pub image: Artifact,
}
pub(super) fn hash_valid(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn id_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn fraction(n: f64) -> bool {
    n.is_finite() && (0.0..=1.0).contains(&n)
}
impl CaseSet {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && !self.cases.is_empty() && self.cases.len() <= MAX_CASES,
            "invalid case-set size/version"
        );
        let mut ids = BTreeSet::new();
        let mut total = 0usize;
        for case in &self.cases {
            ensure!(
                id_valid(&case.id) && ids.insert(&case.id),
                "invalid/duplicate case ID"
            );
            ensure!(
                !case.title.trim().is_empty()
                    && case.title.len() <= 256
                    && !case.kind.is_empty()
                    && case.kind.len() <= 64,
                "invalid case metadata"
            );
            ensure!(!case.samples.is_empty(), "empty required sample set");
            total = total
                .checked_add(case.samples.len())
                .ok_or_else(|| anyhow::anyhow!("sample count overflow"))?;
            ensure!(total <= MAX_SAMPLES, "sample budget exceeded");
            ensure!(
                case.tolerances.max_mae.is_finite()
                    && (0.0..=255.0).contains(&case.tolerances.max_mae)
                    && fraction(case.tolerances.max_bad_pixel_fraction),
                "invalid image tolerances"
            );
            let mut samples = BTreeSet::new();
            for sample in &case.samples {
                ensure!(
                    id_valid(&sample.id) && samples.insert(&sample.id),
                    "invalid/duplicate sample ID"
                );
                ensure!(
                    (1..=4096).contains(&sample.width)
                        && (1..=4096).contains(&sample.height)
                        && fraction(sample.minimum_nonblack_fraction),
                    "invalid sample dimensions/signal threshold"
                );
                let roi = sample.region();
                ensure!(
                    roi.width > 0
                        && roi.height > 0
                        && roi
                            .x
                            .checked_add(roi.width)
                            .is_some_and(|n| n <= sample.width)
                        && roi
                            .y
                            .checked_add(roi.height)
                            .is_some_and(|n| n <= sample.height),
                    "ROI outside expected image"
                );
            }
        }
        Ok(())
    }
    pub fn keys(&self) -> BTreeSet<(&str, &str)> {
        self.cases
            .iter()
            .flat_map(|c| {
                c.samples
                    .iter()
                    .map(move |s| (c.id.as_str(), s.id.as_str()))
            })
            .collect()
    }
}
impl Sample {
    pub fn region(&self) -> Roi {
        self.roi.unwrap_or(Roi {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        })
    }
}
pub(super) fn load_cases(root: &Path, path: &Path) -> Result<(CaseSet, Tracked)> {
    let path = resolve_local(root, path, false).map_err(anyhow::Error::new)?;
    let (bytes, tracked) =
        read_artifact(&path, None, MAX_JSON, "case_set").map_err(anyhow::Error::new)?;
    let cases: CaseSet = serde_json::from_slice(&bytes)?;
    cases.validate()?;
    Ok((cases, tracked))
}
pub(super) fn load_pack(
    root: &Path,
    path: &Path,
    role: &str,
    case_hash: &str,
    tracked: &mut Vec<Tracked>,
) -> Result<Pack, Failure> {
    let path = resolve_local(root, path, true)?;
    let (bytes, proof) = read_artifact(&path, None, MAX_JSON, &format!("{role}_pack"))?;
    tracked.push(proof);
    let p: Pack = serde_json::from_slice(&bytes).map_err(|_| Failure::invalid("invalid_pack"))?;
    if p.case_set_sha256 != case_hash {
        return Err(Failure {
            state: "stale",
            reason: "case_set_mismatch",
        });
    }
    if p.schema_version != 1
        || p.role != role
        || p.captures.len() > MAX_SAMPLES
        || p.producer.version.trim().is_empty()
        || p.producer.version.len() > 256
    {
        return Err(Failure::invalid("invalid_pack_identity"));
    }
    if p.disc.game_id != "SJBE52" || p.disc.revision != 0 || !hash_valid(&p.disc.image_sha256) {
        return Err(Failure::invalid("wrong_disc"));
    }
    if p.producer.kind
        != if role == "reference" {
            "dolphin"
        } else {
            "goldeneye_rust"
        }
    {
        return Err(Failure::invalid("wrong_producer"));
    }
    Ok(p)
}
