use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::diff::{load_change_set, ChangeSetError, ChangeSourceRequest};
use crate::core::tokens::{RegexExtractor, TokenCategory, TokenExtractor};
use crate::core::{
    display_path, normalize_project, DocumentCandidate, DocumentLane, Manifest, RouteArgs,
};

#[derive(Debug)]
pub(crate) struct CloseoutArgs {
    pub(crate) route: RouteArgs,
    pub(crate) source: Option<ChangeSourceRequest>,
    pub(crate) pack: bool,
    pub(crate) max_lines: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CloseoutManifest {
    pub(crate) source: crate::core::diff::ChangeSourceSummary,
    pub(crate) changed_files: Vec<String>,
    pub(crate) changed_categories: Vec<String>,
    pub(crate) new_tokens: Vec<String>,
    pub(crate) removed_tokens: Vec<String>,
    pub(crate) missing_tokens: Vec<String>,
    #[serde(default)]
    pub(crate) missing_targets: Vec<MissingTarget>,
    pub(crate) possible_doc_impact: Vec<DocImpact>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MissingTarget {
    pub(crate) token: String,
    pub(crate) path: String,
    pub(crate) lane: DocumentLane,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DocImpact {
    pub(crate) token: String,
    pub(crate) signal: DocImpactSignal,
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) lane: DocumentLane,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum DocImpactSignal {
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "update")]
    Update,
}

#[derive(Debug)]
pub(crate) enum CloseoutError {
    NeedsInput,
    Other(String),
}

impl CloseoutArgs {
    pub(crate) fn build_manifest(&self) -> Result<Manifest, CloseoutError> {
        let source = self.source.clone().ok_or(CloseoutError::NeedsInput)?;
        let mut manifest = self.route.build_manifest().map_err(CloseoutError::Other)?;
        let project = normalize_project(&self.route.project).map_err(CloseoutError::Other)?;
        let change_set = load_change_set(&project, source).map_err(|error| match error {
            ChangeSetError::NeedsInput => CloseoutError::NeedsInput,
            ChangeSetError::Other(message) => CloseoutError::Other(message),
        })?;
        let extractor = RegexExtractor::new().map_err(CloseoutError::Other)?;
        let mut added_tokens = BTreeMap::new();
        let mut removed_tokens = BTreeMap::new();
        for file in &change_set.files {
            merge_tokens(
                &mut added_tokens,
                extractor.extract_for_path(&file.path, &file.added),
            );
            merge_tokens(
                &mut removed_tokens,
                extractor.extract_for_path(&file.path, &file.removed),
            );
        }
        let changed_categories = added_tokens
            .values()
            .chain(removed_tokens.values())
            .map(|category| category.as_str().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let added_token_set = added_tokens.keys().cloned().collect::<BTreeSet<_>>();
        let removed_token_set = removed_tokens.keys().cloned().collect::<BTreeSet<_>>();
        let new_tokens = added_token_set
            .difference(&removed_token_set)
            .cloned()
            .collect::<Vec<_>>();
        let removed_tokens_list = removed_token_set
            .difference(&added_token_set)
            .cloned()
            .collect::<Vec<_>>();
        let possible_doc_impact =
            find_doc_impact(&project, &manifest, &new_tokens, &removed_tokens_list);
        let impacted_new = possible_doc_impact
            .iter()
            .filter(|impact| impact.signal == DocImpactSignal::Update)
            .map(|impact| impact.token.clone())
            .collect::<BTreeSet<_>>();
        let missing_tokens = new_tokens
            .iter()
            .filter(|token| !impacted_new.contains(*token))
            .cloned()
            .collect::<Vec<_>>();
        let missing_targets = find_missing_targets(&project, &manifest, &missing_tokens);

        let changed_files = change_set.changed_files();
        manifest.command = "closeout".to_string();
        manifest.closeout = Some(CloseoutManifest {
            source: change_set.source,
            changed_files,
            changed_categories,
            new_tokens,
            removed_tokens: removed_tokens_list,
            missing_tokens,
            missing_targets,
            possible_doc_impact,
        });
        manifest.rules.push(
            "closeout requires a content-bearing change source; missing sources return needs_input"
                .to_string(),
        );
        manifest.rules.push(
            "verify checks stale tokens are absent and missing tokens are present".to_string(),
        );
        Ok(manifest)
    }
}

fn find_missing_targets(
    project: &std::path::Path,
    manifest: &Manifest,
    missing_tokens: &[String],
) -> Vec<MissingTarget> {
    let Some(target) = best_missing_target(project, manifest) else {
        return Vec::new();
    };
    missing_tokens
        .iter()
        .map(|token| MissingTarget {
            token: token.clone(),
            path: display_path(&PathBuf::from(&target.path)),
            lane: target.lane.clone(),
        })
        .collect()
}

fn best_missing_target<'a>(
    project: &std::path::Path,
    manifest: &'a Manifest,
) -> Option<&'a DocumentCandidate> {
    manifest
        .candidates
        .iter()
        .filter(|candidate| is_targetable_doc(project, candidate))
        .find(|candidate| candidate.lane == DocumentLane::CurrentDevDocs)
        .or_else(|| {
            manifest
                .candidates
                .iter()
                .find(|candidate| is_targetable_doc(project, candidate))
        })
}

fn is_targetable_doc(project: &std::path::Path, candidate: &DocumentCandidate) -> bool {
    !candidate.archived && project.join(PathBuf::from(&candidate.path)).is_file()
}

fn merge_tokens(
    output: &mut BTreeMap<String, TokenCategory>,
    tokens: BTreeMap<String, TokenCategory>,
) {
    for (token, category) in tokens {
        output.entry(token).or_insert(category);
    }
}

fn find_doc_impact(
    project: &std::path::Path,
    manifest: &Manifest,
    new_tokens: &[String],
    removed_tokens: &[String],
) -> Vec<DocImpact> {
    let mut impacts = Vec::new();
    for candidate in &manifest.candidates {
        if candidate.archived {
            continue;
        }
        let path = project.join(PathBuf::from(&candidate.path));
        if !path.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            for token in removed_tokens {
                if line.contains(token) {
                    impacts.push(DocImpact {
                        token: token.clone(),
                        signal: DocImpactSignal::Stale,
                        path: display_path(&PathBuf::from(&candidate.path)),
                        line: index + 1,
                        lane: candidate.lane.clone(),
                    });
                }
            }
            for token in new_tokens {
                if line.contains(token) {
                    impacts.push(DocImpact {
                        token: token.clone(),
                        signal: DocImpactSignal::Update,
                        path: display_path(&PathBuf::from(&candidate.path)),
                        line: index + 1,
                        lane: candidate.lane.clone(),
                    });
                }
            }
        }
    }
    impacts
}
