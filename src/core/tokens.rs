use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

    fn extract_for_path(&self, path: &str, lines: &[String]) -> BTreeMap<String, TokenCategory> {
        let _ = path;
        self.extract(lines)
    }
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
        self.insert(output, value, TokenCategory::ConfigKey);
    }

    fn extract_lines(
        &self,
        lines: &[String],
        include_config_keys: bool,
    ) -> BTreeMap<String, TokenCategory> {
        let mut output = BTreeMap::new();
        for line in lines {
            for matched in self.env.find_iter(line) {
                self.insert(&mut output, matched.as_str(), TokenCategory::Env);
            }
            for matched in self.flag.find_iter(line) {
                self.insert(&mut output, matched.as_str(), TokenCategory::Flag);
            }
            if include_config_keys {
                for matched in self.config_key.find_iter(line) {
                    self.insert_config(&mut output, matched.as_str());
                }
            }
        }
        output
    }
}

impl TokenExtractor for RegexExtractor {
    fn extract(&self, lines: &[String]) -> BTreeMap<String, TokenCategory> {
        self.extract_lines(lines, false)
    }

    fn extract_for_path(&self, path: &str, lines: &[String]) -> BTreeMap<String, TokenCategory> {
        self.extract_lines(lines, is_config_path(path))
    }
}

fn is_config_path(path: &str) -> bool {
    let path = Path::new(path);
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.eq_ignore_ascii_case(".env"))
        .unwrap_or(false)
    {
        return true;
    }

    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "toml" | "yaml" | "yml" | "json" | "ini" | "env" | "cfg" | "conf"
    )
}
