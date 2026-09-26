use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::core::closeout::DocImpactSignal;
use crate::core::waivers::load_waivers;
use crate::core::{latest_closeout_manifest, normalize_project, run_id_of, DocumentLane, Manifest};

#[derive(Debug)]
pub(crate) struct VerifyReport {
    pub(crate) stale_remaining: Vec<String>,
    pub(crate) missing_remaining: Vec<String>,
    /// Removed tokens still mentioned in record docs. Record docs are history,
    /// so these are reported but never fail verify.
    pub(crate) stale_advisory: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct VerifyOutcome {
    pub(crate) verified_at: String,
    pub(crate) result: String,
    pub(crate) stale_remaining: Vec<String>,
    pub(crate) missing_remaining: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) stale_advisory: Vec<String>,
}

impl VerifyReport {
    pub(crate) fn is_ok(&self) -> bool {
        self.stale_remaining.is_empty() && self.missing_remaining.is_empty()
    }
}

pub(crate) fn verify_project(project: &Path) -> Result<VerifyReport, String> {
    let project = normalize_project(project)?;
    crate::core::ensure_artifact_dir(&project)?;
    let manifest_path = latest_closeout_manifest(&project)?
        .ok_or_else(|| "no closeout manifest found".to_string())?;
    let waivers = load_waivers(&project)?.active(run_id_of(&manifest_path).as_deref());
    let run_dir = manifest_path
        .parent()
        .ok_or_else(|| format!("manifest has no run directory: {}", manifest_path.display()))?
        .to_path_buf();
    let manifest_text = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("invalid manifest {}: {error}", manifest_path.display()))?;
    let closeout = manifest
        .closeout
        .as_ref()
        .ok_or_else(|| "latest manifest is not a closeout manifest".to_string())?;
    let docs = manifest
        .candidates
        .iter()
        .filter(|candidate| !candidate.archived)
        .filter(|candidate| project.join(PathBuf::from(&candidate.path)).is_file())
        .map(|candidate| project.join(PathBuf::from(&candidate.path)))
        .collect::<Vec<_>>();

    let stale_impacts = closeout
        .possible_doc_impact
        .iter()
        .filter(|impact| impact.signal == DocImpactSignal::Stale)
        .filter(|impact| !waivers.contains(&impact.token))
        .map(|impact| {
            (
                impact.path.clone(),
                impact.token.clone(),
                impact.lane == DocumentLane::RecordDocs,
            )
        })
        .collect::<BTreeSet<_>>();

    let mut stale_remaining = BTreeSet::new();
    let mut stale_advisory = BTreeSet::new();
    for (path, token, is_record) in stale_impacts {
        let doc = project.join(PathBuf::from(&path));
        if !path_contains(&doc, &token) {
            continue;
        }
        if is_record {
            stale_advisory.insert(format!("{token} ({path})"));
        } else {
            stale_remaining.insert(token);
        }
    }
    let stale_remaining = stale_remaining.into_iter().collect::<Vec<_>>();
    let stale_advisory = stale_advisory.into_iter().collect::<Vec<_>>();

    let mut missing_remaining = BTreeSet::new();
    let targeted_missing = closeout
        .missing_targets
        .iter()
        .filter(|target| !waivers.contains(&target.token))
        .map(|target| target.token.clone())
        .collect::<BTreeSet<_>>();
    for target in &closeout.missing_targets {
        if waivers.contains(&target.token) {
            continue;
        }
        let doc = project.join(PathBuf::from(&target.path));
        if !path_contains(&doc, &target.token) {
            missing_remaining.insert(target.token.clone());
        }
    }
    for token in &closeout.missing_tokens {
        if !waivers.contains(token)
            && !targeted_missing.contains(token)
            && !docs_contain(&docs, token)
        {
            missing_remaining.insert(token.clone());
        }
    }
    let missing_remaining = missing_remaining.into_iter().collect::<Vec<_>>();

    let report = VerifyReport {
        stale_remaining,
        missing_remaining,
        stale_advisory,
    };
    write_verify_outcome(&run_dir, &report)?;
    Ok(report)
}

fn docs_contain(docs: &[PathBuf], token: &str) -> bool {
    docs.iter().any(|path| path_contains(path, token))
}

fn path_contains(path: &Path, token: &str) -> bool {
    fs::read_to_string(path)
        .map(|text| contains_token(&text, token))
        .unwrap_or(false)
}

fn contains_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + token.len()..].chars().next();
        !before.map(is_token_char).unwrap_or(false) && !after.map(is_token_char).unwrap_or(false)
    })
}

fn is_token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn write_verify_outcome(run_dir: &Path, report: &VerifyReport) -> Result<(), String> {
    let outcome = VerifyOutcome {
        verified_at: current_unix_ms().to_string(),
        result: if report.is_ok() {
            "passed".to_string()
        } else {
            "failed".to_string()
        },
        stale_remaining: report.stale_remaining.clone(),
        missing_remaining: report.missing_remaining.clone(),
        stale_advisory: report.stale_advisory.clone(),
    };
    let text = serde_json::to_string_pretty(&outcome)
        .map(|json| format!("{json}\n"))
        .map_err(|error| format!("cannot render verify outcome: {error}"))?;
    let path = run_dir.join("outcome.json");
    fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
