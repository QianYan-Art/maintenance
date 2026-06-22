use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ChangeSet {
    pub(crate) source: ChangeSourceSummary,
    pub(crate) files: Vec<ChangedFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ChangeSourceSummary {
    pub(crate) kind: String,
    pub(crate) detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChangedFile {
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) added: Vec<String>,
    #[serde(default)]
    pub(crate) removed: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum ChangeSourceRequest {
    GitUncommitted,
    Since(String),
    ChangeManifest(PathBuf),
}

#[derive(Debug)]
pub(crate) enum ChangeSetError {
    NeedsInput,
    Other(String),
}

impl ChangeSet {
    pub(crate) fn changed_files(&self) -> Vec<String> {
        self.files.iter().map(|file| file.path.clone()).collect()
    }
}

pub(crate) fn load_change_set(
    project: &Path,
    request: ChangeSourceRequest,
) -> Result<ChangeSet, ChangeSetError> {
    match request {
        ChangeSourceRequest::GitUncommitted => load_git_uncommitted(project),
        ChangeSourceRequest::Since(revision) => load_git_since(project, &revision),
        ChangeSourceRequest::ChangeManifest(path) => load_change_manifest(project, &path),
    }
}

fn load_git_uncommitted(project: &Path) -> Result<ChangeSet, ChangeSetError> {
    ensure_git(project)?;
    let unstaged = run_git_diff(project, &["diff", "--unified=0"])?;
    let staged = run_git_diff(project, &["diff", "--cached", "--unified=0"])?;
    let mut files = parse_git_diff(&unstaged);
    files.extend(parse_git_diff(&staged));
    files.extend(load_untracked(project)?);
    Ok(ChangeSet {
        source: ChangeSourceSummary {
            kind: "git_uncommitted".to_string(),
            detail: "working tree and index".to_string(),
        },
        files: merge_files(filter_internal_files(files)),
    })
}

fn load_untracked(project: &Path) -> Result<Vec<ChangedFile>, ChangeSetError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()
        .map_err(|error| {
            ChangeSetError::Other(format!("failed to list untracked files: {error}"))
        })?;
    if !output.status.success() {
        return Err(ChangeSetError::Other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    let mut files = Vec::new();
    for path in String::from_utf8_lossy(&output.stdout).lines() {
        let path = path.replace('\\', "/");
        if is_internal_change_path(&path) {
            continue;
        }
        let full_path = project.join(&path);
        let Ok(text) = fs::read_to_string(&full_path) else {
            continue;
        };
        files.push(ChangedFile {
            path,
            added: text.lines().map(|line| line.to_string()).collect(),
            removed: Vec::new(),
        });
    }
    Ok(files)
}

fn load_git_since(project: &Path, revision: &str) -> Result<ChangeSet, ChangeSetError> {
    ensure_git(project)?;
    let diff = run_git_diff(project, &["diff", "--unified=0", revision])?;
    Ok(ChangeSet {
        source: ChangeSourceSummary {
            kind: "git_since".to_string(),
            detail: revision.to_string(),
        },
        files: merge_files(filter_internal_files(parse_git_diff(&diff))),
    })
}

fn load_change_manifest(project: &Path, path: &Path) -> Result<ChangeSet, ChangeSetError> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project.join(path)
    };
    let text = fs::read_to_string(&path).map_err(|error| {
        ChangeSetError::Other(format!(
            "cannot read change manifest {}: {error}",
            path.display()
        ))
    })?;
    let manifest: ChangeManifest = serde_json::from_str(&text).map_err(|error| {
        ChangeSetError::Other(format!(
            "invalid change manifest {}: {error}",
            path.display()
        ))
    })?;
    Ok(ChangeSet {
        source: ChangeSourceSummary {
            kind: "change_manifest".to_string(),
            detail: path.display().to_string(),
        },
        files: merge_files(manifest.files),
    })
}

#[derive(Debug, Deserialize)]
struct ChangeManifest {
    files: Vec<ChangedFile>,
}

fn ensure_git(project: &Path) -> Result<(), ChangeSetError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|error| ChangeSetError::Other(format!("failed to run git: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ChangeSetError::NeedsInput)
    }
}

fn run_git_diff(project: &Path, args: &[&str]) -> Result<String, ChangeSetError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|error| ChangeSetError::Other(format!("failed to run git diff: {error}")))?;
    if !output.status.success() {
        return Err(ChangeSetError::Other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_git_diff(diff: &str) -> Vec<ChangedFile> {
    let mut files = Vec::new();
    let mut current: Option<ChangedFile> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            current = Some(ChangedFile {
                path: parse_diff_path(rest),
                ..ChangedFile::default()
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ b/") {
            if let Some(file) = current.as_mut() {
                file.path = rest.to_string();
            }
            continue;
        }
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            if let Some(file) = current.as_mut() {
                file.added.push(added.to_string());
            }
        } else if let Some(removed) = line.strip_prefix('-') {
            if let Some(file) = current.as_mut() {
                file.removed.push(removed.to_string());
            }
        }
    }
    if let Some(file) = current {
        files.push(file);
    }

    files
}

fn parse_diff_path(rest: &str) -> String {
    rest.split_whitespace()
        .nth(1)
        .and_then(|path| path.strip_prefix("b/"))
        .unwrap_or(rest)
        .to_string()
}

fn merge_files(files: Vec<ChangedFile>) -> Vec<ChangedFile> {
    let mut merged: BTreeMap<String, ChangedFile> = BTreeMap::new();
    for file in files {
        let entry = merged
            .entry(file.path.clone())
            .or_insert_with(|| ChangedFile {
                path: file.path,
                added: Vec::new(),
                removed: Vec::new(),
            });
        entry.added.extend(file.added);
        entry.removed.extend(file.removed);
    }
    merged.into_values().collect()
}

fn filter_internal_files(files: Vec<ChangedFile>) -> Vec<ChangedFile> {
    files
        .into_iter()
        .filter(|file| !is_internal_change_path(&file.path))
        .collect()
}

fn is_internal_change_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    matches!(path.as_str(), ".doc-maintenance" | ".mission")
        || path.starts_with(".doc-maintenance/")
        || path.starts_with(".mission/")
}
