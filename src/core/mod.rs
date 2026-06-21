use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(crate) mod closeout;
pub(crate) mod diff;
pub(crate) mod tokens;
pub(crate) mod verify;

#[derive(Debug)]
pub(crate) struct RouteArgs {
    pub(crate) project: PathBuf,
    pub(crate) dev_docs: Vec<PathBuf>,
    pub(crate) record_docs: Vec<PathBuf>,
    pub(crate) summary_source: Vec<PathBuf>,
    pub(crate) topic: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) schema_version: u32,
    pub(crate) command: String,
    pub(crate) project: String,
    pub(crate) inputs: ManifestInputs,
    pub(crate) candidates: Vec<DocumentCandidate>,
    pub(crate) rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) closeout: Option<closeout::CloseoutManifest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ManifestInputs {
    pub(crate) dev_docs: Vec<String>,
    pub(crate) record_docs: Vec<String>,
    pub(crate) summary_source: Vec<String>,
    pub(crate) topic: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DocumentCandidate {
    pub(crate) path: String,
    pub(crate) lane: DocumentLane,
    pub(crate) reason: String,
    pub(crate) archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum DocumentLane {
    #[serde(rename = "Current Dev Docs")]
    CurrentDevDocs,
    #[serde(rename = "KBase Records")]
    KBaseRecords,
    #[serde(rename = "Archived Records")]
    ArchivedRecords,
}

impl DocumentLane {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::CurrentDevDocs => "Current Dev Docs",
            Self::KBaseRecords => "KBase Records",
            Self::ArchivedRecords => "Archived Records",
        }
    }
}

impl RouteArgs {
    pub(crate) fn build_manifest(&self) -> Result<Manifest, String> {
        let project = normalize_project(&self.project)?;
        let dev_inputs = dev_doc_inputs(&project, &self.dev_docs);
        let record_inputs = resolve_inputs(&project, &self.record_docs);
        let summary_inputs = resolve_inputs(&project, &self.summary_source);

        let mut candidates = Vec::new();
        collect_candidates(
            &project,
            &dev_inputs,
            DocumentLane::CurrentDevDocs,
            &[],
            &mut candidates,
        )?;
        collect_candidates(
            &project,
            &record_inputs,
            DocumentLane::KBaseRecords,
            &self.topic,
            &mut candidates,
        )?;

        Ok(Manifest {
            schema_version: 1,
            command: "route".to_string(),
            project: display_path(&project),
            inputs: ManifestInputs {
                dev_docs: display_paths(&dev_inputs),
                record_docs: display_paths(&record_inputs),
                summary_source: display_paths(&summary_inputs),
                topic: self.topic.clone(),
            },
            candidates: dedupe_candidates(candidates),
            rules: vec![
                "packet lists paths and reasons only; it never inlines document bodies".to_string(),
                "subagent must be read-only and return path:line evidence".to_string(),
                "record docs are processed only when explicitly passed".to_string(),
                "any path segment named archived is listed as Archived Records only".to_string(),
            ],
            closeout: None,
        })
    }
}

pub(crate) fn run_dir(project: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    project
        .join(".doc-maintenance")
        .join("runs")
        .join(format!("{millis}"))
}

pub(crate) fn normalize_project(project: &Path) -> Result<PathBuf, String> {
    let path = if project.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        project.to_path_buf()
    };
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot read current directory: {error}"))?
            .join(path)
    };
    fs::create_dir_all(&absolute).map_err(|error| {
        format!(
            "cannot create or access project {}: {error}",
            absolute.display()
        )
    })?;
    Ok(clean_path(absolute))
}

fn clean_path(path: PathBuf) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}

fn dev_doc_inputs(project: &Path, explicit: &[PathBuf]) -> Vec<PathBuf> {
    if !explicit.is_empty() {
        return resolve_inputs(project, explicit);
    }

    ["README.md", "docs"]
        .into_iter()
        .map(|item| project.join(item))
        .filter(|path| path.exists())
        .collect()
}

pub(crate) fn resolve_inputs(project: &Path, inputs: &[PathBuf]) -> Vec<PathBuf> {
    inputs
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                project.join(path)
            }
        })
        .collect()
}

fn collect_candidates(
    project: &Path,
    inputs: &[PathBuf],
    lane: DocumentLane,
    topics: &[String],
    candidates: &mut Vec<DocumentCandidate>,
) -> Result<(), String> {
    for input in inputs {
        collect_one(project, input, lane.clone(), topics, candidates)?;
    }
    Ok(())
}

fn collect_one(
    project: &Path,
    path: &Path,
    lane: DocumentLane,
    topics: &[String],
    candidates: &mut Vec<DocumentCandidate>,
) -> Result<(), String> {
    if is_archived(path) {
        push_candidate(
            project,
            path,
            DocumentLane::ArchivedRecords,
            "archived path; list only",
            true,
            candidates,
        );
        return Ok(());
    }

    if path.is_file() {
        if include_for_topic(path, topics) && looks_like_doc(path) {
            push_candidate(
                project,
                path,
                lane,
                "explicit document path",
                false,
                candidates,
            );
        }
        return Ok(());
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|error| format!("cannot read directory {}: {error}", path.display()))?
        {
            let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
            let child = entry.path();
            if is_archived(&child) {
                push_candidate(
                    project,
                    &child,
                    DocumentLane::ArchivedRecords,
                    "archived path; list only",
                    true,
                    candidates,
                );
                continue;
            }
            if child.is_dir() {
                if lane == DocumentLane::KBaseRecords && include_for_topic(&child, topics) {
                    push_candidate(
                        project,
                        &child,
                        lane.clone(),
                        "explicit record docs navigation directory",
                        false,
                        candidates,
                    );
                }
                collect_one(project, &child, lane.clone(), topics, candidates)?;
            } else if child.is_file() && looks_like_doc(&child) && include_for_topic(&child, topics)
            {
                let reason = match lane {
                    DocumentLane::CurrentDevDocs => "default or explicit development doc",
                    DocumentLane::KBaseRecords => {
                        "explicit record docs path matched by file name or topic"
                    }
                    DocumentLane::ArchivedRecords => "archived path; list only",
                };
                push_candidate(project, &child, lane.clone(), reason, false, candidates);
            }
        }
    }

    Ok(())
}

fn push_candidate(
    project: &Path,
    path: &Path,
    lane: DocumentLane,
    reason: &str,
    archived: bool,
    candidates: &mut Vec<DocumentCandidate>,
) {
    candidates.push(DocumentCandidate {
        path: display_path(&relative_to_project(project, path)),
        lane,
        reason: reason.to_string(),
        archived,
    });
}

fn dedupe_candidates(candidates: Vec<DocumentCandidate>) -> Vec<DocumentCandidate> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = format!("{}|{}", candidate.lane.title(), candidate.path);
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn relative_to_project(project: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(project).unwrap_or(path).to_path_buf()
}

fn display_paths(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|path| display_path(path)).collect()
}

pub(crate) fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

pub(crate) fn looks_like_doc(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("md" | "mdx" | "txt" | "rst")
    )
}

fn include_for_topic(path: &Path, topics: &[String]) -> bool {
    if topics.is_empty() {
        return true;
    }
    let haystack = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    topics
        .iter()
        .map(|topic| topic.to_ascii_lowercase())
        .any(|topic| haystack.contains(&topic))
}

fn is_archived(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(value) => value
            .to_str()
            .map(|segment| segment.eq_ignore_ascii_case("archived"))
            .unwrap_or(false),
        _ => false,
    })
}
