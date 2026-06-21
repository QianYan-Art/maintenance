use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub(crate) enum TokenCategory {
    #[serde(rename = "env")]
    Env,
    #[serde(rename = "flag")]
    Flag,
    #[serde(rename = "config_key")]
    ConfigKey,
}

impl TokenCategory {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::Flag => "flag",
            Self::ConfigKey => "config_key",
        }
    }
}

pub(crate) trait TokenExtractor {
    fn extract(&self, lines: &[String]) -> BTreeMap<String, TokenCategory>;
}

pub(crate) struct RegexExtractor {
    env: Regex,
    flag: Regex,
    config_key: Regex,
    stopwords: BTreeSet<&'static str>,
}

impl RegexExtractor {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            env: Regex::new(r"\b[A-Z][A-Z0-9_]{2,}\b")
                .map_err(|error| format!("invalid env token regex: {error}"))?,
            flag: Regex::new(r"--[a-z][a-z0-9-]{2,}")
                .map_err(|error| format!("invalid flag token regex: {error}"))?,
            config_key: Regex::new(r"\b[a-z][a-z0-9_-]*(?:\.[a-z][a-z0-9_-]*)+\b")
                .map_err(|error| format!("invalid config token regex: {error}"))?,
            stopwords: BTreeSet::from([
                "AND",
                "FALSE",
                "FIXME",
                "HEAD",
                "NONE",
                "NULL",
                "PATH",
                "README",
                "SUCCESS",
                "TEST",
                "TODO",
                "TRUE",
                "UNIX_EPOCH",
            ]),
        })
    }

    fn insert(
        &self,
        output: &mut BTreeMap<String, TokenCategory>,
        value: &str,
        category: TokenCategory,
    ) {
        if value.len() < 3 || self.stopwords.contains(value) {
            return;
        }
        output.entry(value.to_string()).or_insert(category);
    }

    fn insert_config(&self, output: &mut BTreeMap<String, TokenCategory>, value: &str) {
        let mut parts = value.split('.');
        let first = parts.next().unwrap_or_default();
        let last = value.rsplit('.').next().unwrap_or_default();
        let semantic_roots = [
            "api", "auth", "bucket", "cache", "config", "database", "db", "endpoint", "feature",
            "host", "mode", "port", "queue", "region", "service", "setting", "timeout", "topic",
            "url",
        ];
        let blocked_first = [
            "args",
            "candidate",
            "change_set",
            "child",
            "closeout",
            "crates",
            "entry",
            "error",
            "file",
            "fs",
            "impact",
            "manifest",
            "out",
            "output",
            "path",
            "project",
            "report",
            "revision",
            "serde",
            "self",
            "source",
            "std",
            "token",
        ];
        let blocked_last = [
            "as_deref",
            "build_manifest",
            "changed_categories",
            "changed_files",
            "clone",
            "com",
            "detail",
            "dev_docs",
            "display",
            "is_empty",
            "is_none",
            "io-index",
            "json",
            "kind",
            "len",
            "line",
            "missing_remaining",
            "missing_tokens",
            "new_tokens",
            "packet_path",
            "path",
            "possible_doc_impact",
            "project",
            "push_str",
            "record_docs",
            "removed_tokens",
            "signal",
            "source",
            "stale_remaining",
            "status",
            "summary_source",
            "title",
            "token",
            "topic",
        ];
        if blocked_first.contains(&first) || blocked_last.contains(&last) {
            return;
        }
        if !value
            .split('.')
            .any(|part| semantic_roots.iter().any(|root| part.contains(root)))
        {
            return;
        }
        self.insert(output, value, TokenCategory::ConfigKey);
    }
}

impl TokenExtractor for RegexExtractor {
    fn extract(&self, lines: &[String]) -> BTreeMap<String, TokenCategory> {
        let mut output = BTreeMap::new();
        for line in lines {
            for matched in self.env.find_iter(line) {
                self.insert(&mut output, matched.as_str(), TokenCategory::Env);
            }
            for matched in self.flag.find_iter(line) {
                self.insert(&mut output, matched.as_str(), TokenCategory::Flag);
            }
            for matched in self.config_key.find_iter(line) {
                self.insert_config(&mut output, matched.as_str());
            }
        }
        output
    }
}
