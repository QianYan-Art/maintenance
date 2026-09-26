use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub(crate) mod closeout;
pub(crate) mod config;
pub(crate) mod diff;
pub(crate) mod report;
pub(crate) mod tokens;
pub(crate) mod verify;
pub(crate) mod waivers;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) input_warnings: Vec<InputWarning>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) closeout: Option<closeout::CloseoutManifest>,
}

/// A problem with an explicit document input that would otherwise be silent,
/// e.g. a record doc that does not exist yet or a topic that filtered out
/// every record doc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct InputWarning {
    pub(crate) kind: InputWarningKind,
    pub(crate) path: String,
    pub(crate) message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputWarningKind {
    MissingPath,
    NoRecordDocs,
}

impl InputWarning {
    pub(crate) fn render(&self) -> String {
        format!("{}: `{}` — {}", self.kind.as_str(), self.path, self.message)
    }
}

impl InputWarningKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::MissingPath => "missing_path",
            Self::NoRecordDocs => "no_record_docs",
        }
    }
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
    #[serde(rename = "Record Docs")]
    RecordDocs,
    #[serde(rename = "Archived Records")]
    ArchivedRecords,
}

impl DocumentLane {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::CurrentDevDocs => "Current Dev Docs",
            Self::RecordDocs => "Record Docs",
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
        let mut input_warnings = Vec::new();
        collect_candidates(
            &project,
            &dev_inputs,
            DocumentLane::CurrentDevDocs,
            &[],
            &mut candidates,
            &mut input_warnings,
        )?;
        let topic_filtered = collect_candidates(
            &project,
            &record_inputs,
            DocumentLane::RecordDocs,
            &self.topic,
            &mut candidates,
            &mut input_warnings,
        )?;
        let has_record_candidate = candidates
            .iter()
            .any(|candidate| candidate.lane == DocumentLane::RecordDocs);
        if !record_inputs.is_empty() && !has_record_candidate {
            input_warnings.push(InputWarning {
                kind: InputWarningKind::NoRecordDocs,
                path: display_paths(&record_inputs).join(", "),
                message: no_record_docs_message(topic_filtered, &self.topic),
            });
        }

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
            input_warnings,
            closeout: None,
        })
    }
}

fn no_record_docs_message(topic_filtered: usize, topics: &[String]) -> String {
    if topic_filtered > 0 {
        format!(
            "record docs were given but none became a candidate; {topic_filtered} document(s) were filtered out by topic {}; loosen --topic or name the file directly",
            topics.join(" / ")
        )
    } else {
        "record docs were given but none became a candidate; check that the paths exist and point to .md/.mdx/.txt/.rst files".to_string()
    }
}

/// The newest run directory whose manifest is a closeout manifest.
pub(crate) fn latest_closeout_manifest(project: &Path) -> Result<Option<PathBuf>, String> {
    let runs = project.join(".doc-maintenance").join("runs");
    if !runs.is_dir() {
        return Ok(None);
    }
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
    Ok(manifests.pop())
}

pub(crate) fn latest_closeout_run_id(project: &Path) -> Result<Option<String>, String> {
    Ok(latest_closeout_manifest(project)?.and_then(|manifest| run_id_of(&manifest)))
}

pub(crate) fn run_id_of(manifest_path: &Path) -> Option<String> {
    manifest_path
        .parent()
        .and_then(|run| run.file_name())
        .and_then(|name| name.to_str())
        .map(str::to_string)
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

/// Create `<project>/.doc-maintenance` with a self-ignoring `.gitignore` so
/// run artifacts never enter Git regardless of the host repository's config.
pub(crate) fn ensure_artifact_dir(project: &Path) -> Result<PathBuf, String> {
    let dir = project.join(".doc-maintenance");
    fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "cannot create artifact directory {}: {error}",
            dir.display()
        )
    })?;
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        fs::write(&gitignore, "*\n")
            .map_err(|error| format!("cannot write {}: {error}", gitignore.display()))?;
    }
    Ok(dir)
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
    if !absolute.is_dir() {
        return Err(format!(
            "project directory does not exist: {}",
            absolute.display()
        ));
    }
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

/// Collects candidates for one lane and returns how many documents the topic
/// filter skipped. Explicitly named files are never topic-filtered; the topic
/// only narrows what a directory expands to.
fn collect_candidates(
    project: &Path,
    inputs: &[PathBuf],
    lane: DocumentLane,
    topics: &[String],
    candidates: &mut Vec<DocumentCandidate>,
    warnings: &mut Vec<InputWarning>,
) -> Result<usize, String> {
    let mut topic_filtered = 0;
    for input in inputs {
        if !input.exists() {
            warnings.push(InputWarning {
                kind: InputWarningKind::MissingPath,
                path: display_path(input),
                message: format!(
                    "explicit {} path does not exist; create the document first (or fix the path) and rerun",
                    lane.title()
                ),
            });
            continue;
        }
        collect_one(
            project,
            input,
            lane.clone(),
            topics,
            candidates,
            &mut topic_filtered,
        )?;
    }
    Ok(topic_filtered)
}

fn collect_one(
    project: &Path,
    path: &Path,
    lane: DocumentLane,
    topics: &[String],
    candidates: &mut Vec<DocumentCandidate>,
    topic_filtered: &mut usize,
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
        if looks_like_doc(path) {
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
                if lane == DocumentLane::RecordDocs && include_for_topic(&child, topics) {
                    push_candidate(
                        project,
                        &child,
                        lane.clone(),
                        "explicit record docs navigation directory",
                        false,
                        candidates,
                    );
                }
                collect_one(
                    project,
                    &child,
                    lane.clone(),
                    topics,
                    candidates,
                    topic_filtered,
                )?;
            } else if child.is_file() && looks_like_doc(&child) {
                if !include_for_topic(&child, topics) {
                    *topic_filtered += 1;
                    continue;
                }
                let reason = match lane {
                    DocumentLane::CurrentDevDocs => "default or explicit development doc",
                    DocumentLane::RecordDocs => {
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

/// A path passes the topic filter when its file name contains any topic term.
/// Each `--topic` value is split on whitespace and commas (ASCII or
/// full-width), so `"SafetyRAISE release"` matches a name containing either
/// word. Matching is case-insensitive for all scripts.
fn include_for_topic(path: &Path, topics: &[String]) -> bool {
    let terms = topic_terms(topics);
    if terms.is_empty() {
        return true;
    }
    let haystack = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_lowercase();
    terms.iter().any(|term| haystack.contains(term.as_str()))
}

fn topic_terms(topics: &[String]) -> Vec<String> {
    topics
        .iter()
        .flat_map(|topic| {
            topic
                .split(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | '、'))
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .filter(|term| !term.is_empty())
        .collect()
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
