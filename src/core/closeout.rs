use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::diff::{load_change_set, ChangeSetError, ChangeSourceRequest};
use crate::core::tokens::{
    RegexExtractor, TokenCategory, TokenConfidence, TokenExtractor, TokenMatch,
};
use crate::core::waivers::{load_waivers, ActiveWaivers};
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
    #[serde(default)]
    pub(crate) new_token_details: Vec<TokenObservation>,
    #[serde(default)]
    pub(crate) removed_token_details: Vec<TokenObservation>,
    pub(crate) missing_tokens: Vec<String>,
    #[serde(default)]
    pub(crate) low_confidence_tokens: Vec<TokenObservation>,
    #[serde(default)]
    pub(crate) ignored_tokens: Vec<IgnoredToken>,
    #[serde(default)]
    pub(crate) missing_targets: Vec<MissingTarget>,
    pub(crate) possible_doc_impact: Vec<DocImpact>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct TokenObservation {
    pub(crate) token: String,
    pub(crate) category: TokenCategory,
    pub(crate) confidence: TokenConfidence,
    pub(crate) evidence: String,
    pub(crate) source_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct IgnoredToken {
    pub(crate) token: String,
    pub(crate) reason: String,
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
        let waivers = load_waivers(&project)
            .map_err(CloseoutError::Other)?
            .active(None);
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
        let ignored_tokens = ignored_tokens(&waivers, &added_tokens, &removed_tokens);
        let waived_tokens = waivers.tokens();
        filter_waived_tokens(&mut added_tokens, &waived_tokens);
        filter_waived_tokens(&mut removed_tokens, &waived_tokens);
        let changed_categories = added_tokens
            .values()
            .chain(removed_tokens.values())
            .map(|matched| matched.category.as_str().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let added_token_set = high_token_set(&added_tokens);
        let removed_token_set = high_token_set(&removed_tokens);
        let new_tokens = added_token_set
            .difference(&removed_token_set)
            .cloned()
            .collect::<Vec<_>>();
        let removed_tokens_list = removed_token_set
            .difference(&added_token_set)
            .cloned()
            .collect::<Vec<_>>();
        let new_token_details = token_observations(&new_tokens, &added_tokens);
        let removed_token_details = token_observations(&removed_tokens_list, &removed_tokens);
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
        let missing_targets = find_missing_targets(
            &project,
            &manifest,
            &missing_tokens,
            &added_tokens,
            &new_tokens,
        );
        let low_confidence_tokens = low_confidence_tokens(&added_tokens, &removed_tokens);

        let changed_files = change_set.changed_files();
        manifest.schema_version = 2;
        manifest.command = "closeout".to_string();
        manifest.closeout = Some(CloseoutManifest {
            source: change_set.source,
            changed_files,
            changed_categories,
            new_tokens,
            removed_tokens: removed_tokens_list,
            new_token_details,
            removed_token_details,
            missing_tokens,
            low_confidence_tokens,
            ignored_tokens,
            missing_targets,
            possible_doc_impact,
        });
        manifest.rules.push(
            "closeout requires a content-bearing change source; missing sources return needs_input"
                .to_string(),
        );
        manifest.rules.push(
            "verify checks stale tokens are absent from dev docs and missing tokens are present; stale tokens in record docs are advisory".to_string(),
        );
        Ok(manifest)
    }
}

fn find_missing_targets(
    project: &std::path::Path,
    manifest: &Manifest,
    missing_tokens: &[String],
    token_matches: &BTreeMap<String, TokenMatch>,
    changed_tokens: &[String],
) -> Vec<MissingTarget> {
    missing_tokens
        .iter()
        .filter_map(|token| {
            let target = best_missing_target(
                project,
                manifest,
                token,
                token_matches.get(token),
                changed_tokens,
            )?;
            Some(MissingTarget {
                token: token.clone(),
                path: display_path(&PathBuf::from(&target.path)),
                lane: target.lane.clone(),
            })
        })
        .collect()
}

fn best_missing_target<'a>(
    project: &std::path::Path,
    manifest: &'a Manifest,
    token: &str,
    matched: Option<&TokenMatch>,
    changed_tokens: &[String],
) -> Option<&'a DocumentCandidate> {
    matched
        .and_then(|matched| {
            affinity_missing_target(project, manifest, token, matched, changed_tokens)
        })
        .or_else(|| fallback_missing_target(project, manifest))
}

fn fallback_missing_target<'a>(
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

fn affinity_missing_target<'a>(
    project: &std::path::Path,
    manifest: &'a Manifest,
    token: &str,
    matched: &TokenMatch,
    changed_tokens: &[String],
) -> Option<&'a DocumentCandidate> {
    let extractor = RegexExtractor::new().ok()?;
    let source_segments = path_segments(&matched.source_path);
    let peer_tokens = changed_tokens
        .iter()
        .filter(|other| other.as_str() != token)
        .collect::<Vec<_>>();
    let mut best = None;
    let mut best_score = 0;
    for candidate in manifest
        .candidates
        .iter()
        .filter(|candidate| is_targetable_doc(project, candidate))
    {
        let path = project.join(PathBuf::from(&candidate.path));
        let text = fs::read_to_string(&path).unwrap_or_default();
        let candidate_segments = path_segments(&candidate.path);
        let path_score = source_segments.intersection(&candidate_segments).count() * 3;
        let peer_score = peer_tokens
            .iter()
            .map(|peer| count_token_occurrences(&text, peer))
            .sum::<usize>()
            * 2;
        let lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        let category_score = extractor
            .extract_for_path(&candidate.path, &lines)
            .values()
            .filter(|found| found.category == matched.category)
            .count();
        let score = path_score + peer_score + category_score;
        if score > best_score {
            best_score = score;
            best = Some(candidate);
        }
    }
    if best_score == 0 {
        None
    } else {
        best
    }
}

fn path_segments(path: &str) -> BTreeSet<String> {
    path.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.to_ascii_lowercase())
        .collect()
}

fn count_token_occurrences(text: &str, token: &str) -> usize {
    text.match_indices(token)
        .filter(|(index, _)| {
            let before = text[..*index].chars().next_back();
            let after = text[*index + token.len()..].chars().next();
            !before.map(is_token_char).unwrap_or(false)
                && !after.map(is_token_char).unwrap_or(false)
        })
        .count()
}

fn is_token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn merge_tokens(output: &mut BTreeMap<String, TokenMatch>, tokens: BTreeMap<String, TokenMatch>) {
    for (token, matched) in tokens {
        match output.get(&token) {
            Some(existing) if existing.confidence >= matched.confidence => {}
            _ => {
                output.insert(token, matched);
            }
        }
    }
}

fn high_token_set(tokens: &BTreeMap<String, TokenMatch>) -> BTreeSet<String> {
    tokens
        .iter()
        .filter(|(_, matched)| matched.confidence == TokenConfidence::High)
        .map(|(token, _)| token.clone())
        .collect()
}

fn low_confidence_tokens(
    added_tokens: &BTreeMap<String, TokenMatch>,
    removed_tokens: &BTreeMap<String, TokenMatch>,
) -> Vec<TokenObservation> {
    let mut tokens = BTreeMap::new();
    for (token, matched) in added_tokens.iter().chain(removed_tokens.iter()) {
        if matched.confidence == TokenConfidence::Low {
            tokens
                .entry(token.clone())
                .or_insert_with(|| TokenObservation {
                    token: token.clone(),
                    category: matched.category.clone(),
                    confidence: matched.confidence.clone(),
                    evidence: matched.evidence.clone(),
                    source_path: matched.source_path.clone(),
                });
        }
    }
    tokens.into_values().collect()
}

fn token_observations(
    tokens: &[String],
    token_matches: &BTreeMap<String, TokenMatch>,
) -> Vec<TokenObservation> {
    tokens
        .iter()
        .filter_map(|token| {
            let matched = token_matches.get(token)?;
            Some(TokenObservation {
                token: token.clone(),
                category: matched.category.clone(),
                confidence: matched.confidence.clone(),
                evidence: matched.evidence.clone(),
                source_path: matched.source_path.clone(),
            })
        })
        .collect()
}

fn ignored_tokens(
    waivers: &ActiveWaivers,
    added_tokens: &BTreeMap<String, TokenMatch>,
    removed_tokens: &BTreeMap<String, TokenMatch>,
) -> Vec<IgnoredToken> {
    let mut ignored = BTreeMap::new();
    for token in added_tokens.keys().chain(removed_tokens.keys()) {
        if let Some(reason) = waivers.reason_for(token) {
            ignored
                .entry(token.clone())
                .or_insert_with(|| reason.to_string());
        }
    }
    ignored
        .into_iter()
        .map(|(token, reason)| IgnoredToken { token, reason })
        .collect()
}

fn filter_waived_tokens(
    tokens: &mut BTreeMap<String, TokenMatch>,
    waived_tokens: &BTreeSet<String>,
) {
    tokens.retain(|token, _| !waived_tokens.contains(token));
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
