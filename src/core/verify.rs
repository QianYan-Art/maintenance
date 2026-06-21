use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::closeout::DocImpactSignal;
use crate::core::{normalize_project, Manifest};

#[derive(Debug)]
pub(crate) struct VerifyReport {
    pub(crate) stale_remaining: Vec<String>,
    pub(crate) missing_remaining: Vec<String>,
}

impl VerifyReport {
    pub(crate) fn is_ok(&self) -> bool {
        self.stale_remaining.is_empty() && self.missing_remaining.is_empty()
    }
}

pub(crate) fn verify_project(project: &Path) -> Result<VerifyReport, String> {
    let project = normalize_project(project)?;
    let manifest_path = latest_closeout_manifest(&project)?;
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

    let stale_tokens = closeout
        .possible_doc_impact
        .iter()
        .filter(|impact| impact.signal == DocImpactSignal::Stale)
        .map(|impact| impact.token.clone())
        .collect::<BTreeSet<_>>();

    let mut stale_remaining = Vec::new();
    for token in stale_tokens {
        if docs_contain(&docs, &token) {
            stale_remaining.push(token);
        }
    }

    let mut missing_remaining = Vec::new();
    for token in &closeout.missing_tokens {
        if !docs_contain(&docs, token) {
            missing_remaining.push(token.clone());
        }
    }

    Ok(VerifyReport {
        stale_remaining,
        missing_remaining,
    })
}

fn latest_closeout_manifest(project: &Path) -> Result<PathBuf, String> {
    let runs = project.join(".doc-maintenance").join("runs");
    let mut manifests = Vec::new();
    let entries = fs::read_dir(&runs)
        .map_err(|error| format!("cannot read runs directory {}: {error}", runs.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read runs entry: {error}"))?;
        let manifest = entry.path().join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let text = fs::read_to_string(&manifest)
            .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
        if text.contains("\"command\": \"closeout\"") {
            manifests.push(manifest);
        }
    }
    manifests.sort();
    manifests
        .pop()
        .ok_or_else(|| "no closeout manifest found".to_string())
}

fn docs_contain(docs: &[PathBuf], token: &str) -> bool {
    docs.iter().any(|path| {
        fs::read_to_string(path)
            .map(|text| text.contains(token))
            .unwrap_or(false)
    })
}
