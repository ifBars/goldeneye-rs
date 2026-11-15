//! Offline, finite frontend comparisons. No roadmap state or game assets ship here.
mod images;
mod manifest;
mod receipt;

use anyhow::{Result, ensure};
use manifest::{CaseSet, Pack};
use receipt::{Failure, Tracked};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const COMPARATOR: &str = "png_rgba8_image_set_v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub pixels: u64,
    pub mae: f64,
    pub max_channel_error: u8,
    pub bad_pixel_fraction: f64,
    pub reference_nonblack_fraction: f64,
    pub candidate_nonblack_fraction: f64,
    pub reference_spatial_range: u8,
    pub candidate_spatial_range: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleResult {
    pub id: String,
    pub passed: bool,
    pub reason: Option<String>,
    pub metrics: Option<Metrics>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub id: String,
    pub title: String,
    pub state: String,
    pub reason: Option<String>,
    pub passed_samples: usize,
    pub total_samples: usize,
    pub samples: Vec<SampleResult>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub schema_version: u32,
    pub comparator: String,
    pub state: String,
    pub reason: Option<String>,
    pub measured_state: Option<String>,
    pub verified: usize,
    pub total: usize,
    pub percent: Option<f64>,
    pub case_set_sha256: String,
    pub run_id: Option<String>,
    pub cases: Vec<CaseResult>,
}
impl Summary {
    fn empty(cases: &CaseSet, hash: &str, state: &str, reason: Option<&str>) -> Self {
        let total = cases.cases.len();
        Self {
            schema_version: 1,
            comparator: COMPARATOR.into(),
            state: state.into(),
            reason: reason.map(str::to_owned),
            measured_state: None,
            verified: 0,
            total,
            percent: (total > 0).then_some(0.0),
            case_set_sha256: hash.into(),
            run_id: None,
            cases: cases
                .cases
                .iter()
                .map(|c| CaseResult {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    state: state.into(),
                    reason: reason.map(str::to_owned),
                    passed_samples: 0,
                    total_samples: c.samples.len(),
                    samples: Vec::new(),
                })
                .collect(),
        }
    }
    fn invalidate(&mut self, state: &str, reason: &str) {
        self.measured_state = Some(self.state.clone());
        self.state = state.into();
        self.reason = Some(reason.into());
        self.verified = 0;
        self.percent = (self.total > 0).then_some(0.0);
        for case in &mut self.cases {
            case.state = state.into();
            case.reason = Some(reason.into());
        }
    }
}
type Captures = BTreeMap<(String, String), (PathBuf, String)>;
struct Run<'a> {
    root: &'a Path,
    cases: &'a CaseSet,
    case_hash: &'a str,
    reference_pack: &'a Path,
    candidate_pack: &'a Path,
    source_hash: &'a str,
    current_exe: &'a Path,
}
fn captures(root: &Path, p: &Pack, cases: &CaseSet) -> std::result::Result<Captures, Failure> {
    let mut result = BTreeMap::new();
    for capture in &p.captures {
        let key = (capture.case_id.clone(), capture.sample_id.clone());
        if !manifest::hash_valid(&capture.image.sha256) || result.contains_key(&key) {
            return Err(Failure::invalid("invalid_capture_identity"));
        }
        let path = receipt::resolve_local(root, &capture.image.path, true)?;
        result.insert(key, (path, capture.image.sha256.clone()));
    }
    let paths: BTreeSet<_> = result.values().map(|(p, _)| p).collect();
    if paths.len() != result.len() {
        return Err(Failure::invalid("aliased_required_samples"));
    }
    let expected = cases.keys();
    let actual: BTreeSet<_> = result
        .keys()
        .map(|(c, s)| (c.as_str(), s.as_str()))
        .collect();
    if !actual.is_subset(&expected) {
        return Err(Failure::invalid("unknown_capture_identity"));
    }
    Ok(result)
}
fn run(context: Run<'_>, tracked: &mut Vec<Tracked>) -> std::result::Result<Summary, Failure> {
    let Run {
        root,
        cases,
        case_hash,
        reference_pack,
        candidate_pack,
        source_hash,
        current_exe,
    } = context;
    let r = manifest::load_pack(root, reference_pack, "reference", case_hash, tracked)?;
    let c = manifest::load_pack(root, candidate_pack, "candidate", case_hash, tracked)?;
    if tracked[1].path == tracked[2].path {
        return Err(Failure::invalid("aliased_capture_packs"));
    }
    if r.source_hash.is_some() || c.source_hash.as_deref() != Some(source_hash) {
        return Err(Failure {
            state: "stale",
            reason: "candidate_source_mismatch",
        });
    }
    if r.disc.image_sha256 != c.disc.image_sha256 {
        return Err(Failure::invalid("disc_identity_mismatch"));
    }
    if r.input_contract.sha256 != c.input_contract.sha256 {
        return Err(Failure::invalid("input_contract_mismatch"));
    }
    let rexe = receipt::track(
        root,
        &r.producer.executable,
        false,
        receipt::MAX_EXE,
        "reference_executable",
        tracked,
    )?;
    let cexe = receipt::track(
        root,
        &c.producer.executable,
        false,
        receipt::MAX_EXE,
        "candidate_executable",
        tracked,
    )?;
    if rexe == cexe || r.producer.executable.sha256 == c.producer.executable.sha256 {
        return Err(Failure::invalid("aliased_producer_executables"));
    }
    if cexe != current_exe {
        return Err(Failure {
            state: "stale",
            reason: "candidate_executable_mismatch",
        });
    }
    for (role, p) in [("reference", &r), ("candidate", &c)] {
        receipt::track(
            root,
            &p.configuration,
            true,
            manifest::MAX_JSON,
            &format!("{role}_configuration"),
            tracked,
        )?;
        receipt::track(
            root,
            &p.input_contract,
            true,
            manifest::MAX_JSON,
            &format!("{role}_inputs"),
            tracked,
        )?;
    }
    let refs = captures(root, &r, cases)?;
    let probes = captures(root, &c, cases)?;
    let reference_paths: BTreeSet<_> = refs.values().map(|(p, _)| p.clone()).collect();
    if probes.values().any(|(p, _)| reference_paths.contains(p)) {
        return Err(Failure::invalid("aliased_reference_candidate_images"));
    }
    // An unpaired image cannot be compared, but it remains part of the pack's
    // evidence and must participate in hash validation and later freshness.
    for (captures, counterpart, kind) in [
        (&refs, &probes, "reference_capture"),
        (&probes, &refs, "candidate_capture"),
    ] {
        for (key, (path, sha256)) in captures {
            if !counterpart.contains_key(key) {
                receipt::track(
                    root,
                    &receipt::Artifact {
                        path: path.clone(),
                        sha256: sha256.clone(),
                    },
                    true,
                    receipt::MAX_PNG,
                    kind,
                    tracked,
                )?;
            }
        }
    }
    let mut summary = Summary::empty(cases, case_hash, "passed", None);
    for (index, case) in cases.cases.iter().enumerate() {
        let mut samples = Vec::new();
        for sample in &case.samples {
            let key = (case.id.clone(), sample.id.clone());
            let missing = if !refs.contains_key(&key) {
                Some("missing_reference_capture")
            } else if !probes.contains_key(&key) {
                Some("missing_candidate_capture")
            } else {
                None
            };
            if let Some(reason) = missing {
                samples.push(SampleResult {
                    id: sample.id.clone(),
                    passed: false,
                    reason: Some(reason.into()),
                    metrics: None,
                });
                continue;
            }
            let (rp, rh) = &refs[&key];
            let (cp, ch) = &probes[&key];
            let (reference, proof) =
                receipt::read_artifact(rp, Some(rh), receipt::MAX_PNG, "reference_capture")?;
            tracked.push(proof);
            let (candidate, proof) =
                receipt::read_artifact(cp, Some(ch), receipt::MAX_PNG, "candidate_capture")?;
            tracked.push(proof);
            let result = if case.kind != "image_set" {
                Err(Failure::unsupported("unsupported_case_kind"))
            } else {
                images::compare(&reference, &candidate, sample, case)
            };
            samples.push(match result {
                Ok((metrics, passed)) => SampleResult {
                    id: sample.id.clone(),
                    passed,
                    reason: (!passed).then(|| "image_or_signal_tolerance_failed".into()),
                    metrics: Some(metrics),
                },
                Err(f) if f.state == "unsupported" || f.state == "failed" => SampleResult {
                    id: sample.id.clone(),
                    passed: false,
                    reason: Some(f.reason.into()),
                    metrics: None,
                },
                Err(f) => return Err(f),
            });
        }
        let passed = samples.iter().filter(|s| s.passed).count();
        let result = &mut summary.cases[index];
        result.passed_samples = passed;
        result.state = if passed == case.samples.len() {
            "passed"
        } else if samples.iter().any(|s| {
            matches!(
                s.reason.as_deref(),
                Some("missing_reference_capture" | "missing_candidate_capture")
            )
        }) {
            "missing"
        } else if case.kind != "image_set"
            || samples
                .iter()
                .any(|s| s.reason.as_deref() == Some("unsupported_png_format"))
        {
            "unsupported"
        } else {
            "failed"
        }
        .into();
        result.reason = (result.state != "passed").then(|| "required_samples_did_not_pass".into());
        result.samples = samples;
    }
    summary.verified = summary.cases.iter().filter(|c| c.state == "passed").count();
    summary.percent = Some(100.0 * summary.verified as f64 / summary.total as f64);
    summary.state = if summary.verified == summary.total {
        "passed"
    } else if summary.cases.iter().all(|c| c.state == "missing") {
        "missing"
    } else if summary.cases.iter().all(|c| c.state == "unsupported") {
        "unsupported"
    } else {
        "failed"
    }
    .into();
    if summary.state != "passed" {
        summary.reason = Some("required_cases_did_not_pass".into());
    }
    receipt::unchanged(tracked)?;
    Ok(summary)
}
/// Compare every declared sample and retain an immutable local run before
/// replacing the latest pointer. Original payloads and paths stay private.
pub fn compare(
    workspace: &Path,
    case_set: &Path,
    reference_pack: &Path,
    candidate_pack: &Path,
    current_source_hash: &str,
    current_candidate_exe: &Path,
) -> Result<Summary> {
    ensure!(
        manifest::hash_valid(current_source_hash),
        "invalid current source hash"
    );
    let (cases, case_proof) = manifest::load_cases(workspace, case_set)?;
    let mut dependencies = vec![case_proof.clone()];
    let exe = receipt::resolve_external(workspace, current_candidate_exe);
    let result = match &exe {
        Ok(exe) => run(
            Run {
                root: workspace,
                cases: &cases,
                case_hash: &case_proof.sha256,
                reference_pack,
                candidate_pack,
                source_hash: current_source_hash,
                current_exe: exe,
            },
            &mut dependencies,
        ),
        Err(_) => Err(Failure {
            state: "missing",
            reason: "missing_candidate_executable",
        }),
    };
    let summary = match result {
        Ok(s) => s,
        Err(f) => Summary::empty(&cases, &case_proof.sha256, f.state, Some(f.reason)),
    };
    let exe = exe.unwrap_or_else(|_| {
        if current_candidate_exe.is_absolute() {
            current_candidate_exe.to_path_buf()
        } else {
            workspace.join(current_candidate_exe)
        }
    });
    receipt::save(
        workspace,
        current_source_hash,
        &exe,
        &case_proof.sha256,
        dependencies,
        summary,
    )
}
/// Revalidate real artifacts against the current case set, source and executable.
/// A missing registry still reports the authored nonzero denominator.
pub fn current(
    workspace: &Path,
    case_set: &Path,
    current_source_hash: &str,
    current_candidate_exe: &Path,
) -> Result<Summary> {
    ensure!(
        manifest::hash_valid(current_source_hash),
        "invalid current source hash"
    );
    let (cases, proof) = manifest::load_cases(workspace, case_set)?;
    receipt::current(
        workspace,
        &cases,
        &proof,
        current_source_hash,
        current_candidate_exe,
    )
}

#[cfg(test)]
mod tests;
