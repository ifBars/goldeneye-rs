use super::{COMPARATOR, Summary, manifest::hash_valid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const MAX_PNG: u64 = 64 * 1024 * 1024;
pub(super) const MAX_EXE: u64 = 512 * 1024 * 1024;
const MAX_RECEIPT: u64 = 4 * 1024 * 1024;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Artifact {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tracked {
    pub path: PathBuf,
    pub sha256: String,
    pub max_bytes: u64,
    pub kind: String,
}
#[derive(Debug)]
pub(super) struct Failure {
    pub state: &'static str,
    pub reason: &'static str,
}
impl Failure {
    pub fn invalid(reason: &'static str) -> Self {
        Self {
            state: "invalid",
            reason,
        }
    }
    pub fn failed(reason: &'static str) -> Self {
        Self {
            state: "failed",
            reason,
        }
    }
    pub fn unsupported(reason: &'static str) -> Self {
        Self {
            state: "unsupported",
            reason,
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}
impl std::error::Error for Failure {}
fn io_failure(error: std::io::Error) -> Failure {
    if error.kind() == std::io::ErrorKind::NotFound {
        Failure {
            state: "missing",
            reason: "missing_artifact",
        }
    } else {
        Failure::invalid("unreadable_artifact")
    }
}
pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn resolve_external(root: &Path, path: &Path) -> Result<PathBuf, Failure> {
    if path.as_os_str().is_empty()
        || path.as_os_str().len() > 4096
        || path.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err(Failure::invalid("invalid_artifact_path"));
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        if matches!(component, Component::Normal(_))
            && fs::symlink_metadata(&prefix)
                .map_err(io_failure)?
                .file_type()
                .is_symlink()
        {
            return Err(Failure::invalid("artifact_symlink_unsupported"));
        }
    }
    let path = path.canonicalize().map_err(io_failure)?;
    if !fs::metadata(&path).map_err(io_failure)?.is_file() {
        return Err(Failure::invalid("artifact_not_regular_file"));
    }
    Ok(path)
}
pub(super) fn resolve_local(root: &Path, path: &Path, private: bool) -> Result<PathBuf, Failure> {
    let resolved = resolve_external(root, path)?;
    let base = if private {
        root.join("local/parity")
    } else {
        root.to_path_buf()
    }
    .canonicalize()
    .map_err(io_failure)?;
    if !resolved.starts_with(base) {
        return Err(Failure::invalid("artifact_path_escape"));
    }
    Ok(resolved)
}
pub(super) fn hash_file(path: &Path, max: u64) -> Result<String, Failure> {
    let mut file = fs::File::open(path).map_err(io_failure)?;
    let metadata = file.metadata().map_err(io_failure)?;
    if !metadata.is_file() {
        return Err(Failure::invalid("artifact_not_regular_file"));
    }
    if metadata.len() > max {
        return Err(Failure::invalid("artifact_size_exceeded"));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(io_failure)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > max {
            return Err(Failure::invalid("artifact_size_exceeded"));
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub(super) fn read_artifact(
    path: &Path,
    expected: Option<&str>,
    max: u64,
    kind: &str,
) -> Result<(Vec<u8>, Tracked), Failure> {
    let file = fs::File::open(path).map_err(io_failure)?;
    let metadata = file.metadata().map_err(io_failure)?;
    if !metadata.is_file() {
        return Err(Failure::invalid("artifact_not_regular_file"));
    }
    if metadata.len() > max {
        return Err(Failure::invalid("artifact_size_exceeded"));
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(io_failure)?;
    if bytes.len() as u64 > max {
        return Err(Failure::invalid("artifact_size_exceeded"));
    }
    let sha256 = digest(&bytes);
    if expected.is_some_and(|hash| !hash_valid(hash) || hash != sha256) {
        return Err(Failure {
            state: "stale",
            reason: "artifact_hash_mismatch",
        });
    }
    Ok((
        bytes,
        Tracked {
            path: path.to_path_buf(),
            sha256,
            max_bytes: max,
            kind: kind.into(),
        },
    ))
}
pub(super) fn track(
    root: &Path,
    a: &Artifact,
    private: bool,
    max: u64,
    kind: &str,
    dependencies: &mut Vec<Tracked>,
) -> Result<PathBuf, Failure> {
    if !hash_valid(&a.sha256) {
        return Err(Failure::invalid("invalid_artifact_hash"));
    }
    let path = if private {
        resolve_local(root, &a.path, true)?
    } else {
        resolve_external(root, &a.path)?
    };
    let actual = hash_file(&path, max)?;
    if actual != a.sha256 {
        return Err(Failure {
            state: "stale",
            reason: "artifact_hash_mismatch",
        });
    }
    dependencies.push(Tracked {
        path: path.clone(),
        sha256: actual,
        max_bytes: max,
        kind: kind.into(),
    });
    Ok(path)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    comparator: String,
    source_hash: String,
    candidate_executable: PathBuf,
    os: String,
    arch: String,
    time_nanos: u64,
    case_set_sha256: String,
    dependencies: Vec<Tracked>,
    summary: Summary,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Latest {
    run_id: String,
    receipt_sha256: String,
}
pub(super) fn save(
    root: &Path,
    source_hash: &str,
    exe: &Path,
    case_hash: &str,
    dependencies: Vec<Tracked>,
    mut summary: Summary,
) -> anyhow::Result<Summary> {
    let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let time_nanos = u64::try_from(time)?;
    let receipt = Receipt {
        schema_version: 1,
        comparator: COMPARATOR.into(),
        source_hash: source_hash.into(),
        candidate_executable: exe.to_path_buf(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        time_nanos,
        case_set_sha256: case_hash.into(),
        dependencies,
        summary: summary.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    let run_id = digest(&bytes);
    summary.run_id = Some(run_id.clone());
    let directory = root.join("local/parity/runs");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{run_id}.json"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let pointer = serde_json::to_vec_pretty(&Latest {
        run_id,
        receipt_sha256: digest(&bytes),
    })?;
    // A partial pointer cannot select an old success: invalid latest is rejected.
    fs::write(root.join("local/parity/latest.json"), pointer)?;
    Ok(summary)
}
pub(super) fn current(
    root: &Path,
    cases: &super::manifest::CaseSet,
    case_proof: &Tracked,
    source_hash: &str,
    exe: &Path,
) -> anyhow::Result<Summary> {
    let path = root.join("local/parity/latest.json");
    if !path.exists() {
        return Ok(Summary::empty(cases, &case_proof.sha256, "not_run", None));
    }
    let invalid = || {
        Summary::empty(
            cases,
            &case_proof.sha256,
            "invalid",
            Some("invalid_latest_receipt"),
        )
    };
    let loaded = (|| -> Result<Receipt, Failure> {
        let path = resolve_local(root, &path, true)?;
        let (bytes, _) = read_artifact(&path, None, 4096, "index")?;
        let latest: Latest = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::invalid("invalid_latest_receipt"))?;
        if !hash_valid(&latest.run_id) || latest.receipt_sha256 != latest.run_id {
            return Err(Failure::invalid("invalid_latest_receipt"));
        }
        let path = resolve_local(
            root,
            &root
                .join("local/parity/runs")
                .join(format!("{}.json", latest.run_id)),
            true,
        )?;
        let (bytes, _) =
            read_artifact(&path, Some(&latest.receipt_sha256), MAX_RECEIPT, "receipt")?;
        let mut r: Receipt = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::invalid("invalid_latest_receipt"))?;
        if r.schema_version != 1
            || r.summary.run_id.is_some()
            || r.dependencies.len() > 1100
            || r.summary.total > super::manifest::MAX_CASES
            || u128::from(r.time_nanos)
                > SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|_| Failure::invalid("invalid_clock"))?
                    .as_nanos()
                    .saturating_add(300_000_000_000)
        {
            return Err(Failure::invalid("invalid_latest_receipt"));
        }
        // Hash the exact stored bytes, not a floating-point JSON round trip.
        // The public run ID comes from this content-addressed file name.
        r.summary.run_id = Some(latest.run_id);
        Ok(r)
    })();
    let r = match loaded {
        Ok(r) => r,
        Err(_) => return Ok(invalid()),
    };
    if r.case_set_sha256 != case_proof.sha256 {
        let mut summary =
            Summary::empty(cases, &case_proof.sha256, "stale", Some("stale_case_set"));
        summary.run_id = r.summary.run_id;
        summary.measured_state = Some(r.summary.state);
        return Ok(summary);
    }
    let mut summary = r.summary;
    let reason = if r.comparator != COMPARATOR {
        Some("stale_comparator")
    } else if r.source_hash != source_hash {
        Some("stale_source")
    } else if r.os != std::env::consts::OS || r.arch != std::env::consts::ARCH {
        Some("stale_host")
    } else {
        None
    };
    if let Some(reason) = reason {
        summary.invalidate("stale", reason);
        return Ok(summary);
    }
    let exe = match resolve_external(root, exe) {
        Ok(p) => p,
        Err(_) => {
            summary.invalidate("missing", "missing_candidate_executable");
            return Ok(summary);
        }
    };
    if exe != r.candidate_executable {
        summary.invalidate("stale", "stale_candidate_executable");
        return Ok(summary);
    }
    for a in &r.dependencies {
        if a.max_bytes > MAX_EXE || !hash_valid(&a.sha256) {
            return Ok(invalid());
        }
        if resolve_external(root, &a.path).is_err() {
            summary.invalidate("missing", "missing_or_changed_artifact_path");
            return Ok(summary);
        }
        match hash_file(&a.path, a.max_bytes) {
            Ok(hash) if hash == a.sha256 => {}
            Ok(_) => {
                summary.invalidate(
                    "stale",
                    if a.kind.starts_with("reference") {
                        "stale_reference"
                    } else if a.kind.starts_with("candidate") {
                        "stale_candidate"
                    } else {
                        "stale_case_set"
                    },
                );
                return Ok(summary);
            }
            Err(_) => {
                summary.invalidate("missing", "missing_or_unreadable_artifact");
                return Ok(summary);
            }
        }
    }
    Ok(summary)
}
pub(super) fn unchanged(dependencies: &[Tracked]) -> Result<(), Failure> {
    for a in dependencies {
        if hash_file(&a.path, a.max_bytes)? != a.sha256 {
            return Err(Failure {
                state: "stale",
                reason: "artifact_changed_during_comparison",
            });
        }
    }
    Ok(())
}
