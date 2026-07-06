use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::closeout::DocImpactSignal;
use crate::core::waivers::load_waivers;
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
    let waivers = load_waivers(&project)?;
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

    let stale_impacts = closeout
        .possible_doc_impact
        .iter()
        .filter(|impact| impact.signal == DocImpactSignal::Stale)
        .filter(|impact| !waivers.contains(&impact.token))
        .map(|impact| (impact.path.clone(), impact.token.clone()))
        .collect::<BTreeSet<_>>();

    let mut stale_remaining = BTreeSet::new();
    for (path, token) in stale_impacts {
        let doc = project.join(PathBuf::from(path));
        if path_contains(&doc, &token) {
            stale_remaining.insert(token);
        }
    }
    let stale_remaining = stale_remaining.into_iter().collect::<Vec<_>>();

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
        let manifest_json: Manifest = serde_json::from_str(&text)
            .map_err(|error| format!("invalid manifest {}: {error}", manifest.display()))?;
        if manifest_json.closeout.is_some() {
            manifests.push(manifest);
        }
    }
    manifests.sort();
    manifests
        .pop()
        .ok_or_else(|| "no closeout manifest found".to_string())
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
