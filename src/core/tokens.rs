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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub(crate) enum TokenConfidence {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "high")]
    High,
}

impl TokenConfidence {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct TokenMatch {
    pub(crate) category: TokenCategory,
    pub(crate) confidence: TokenConfidence,
    pub(crate) evidence: String,
}

pub(crate) trait TokenExtractor {
    fn extract(&self, lines: &[String]) -> BTreeMap<String, TokenMatch>;

    fn extract_for_path(&self, path: &str, lines: &[String]) -> BTreeMap<String, TokenMatch> {
        let _ = path;
        self.extract(lines)
    }
}

pub(crate) struct RegexExtractor {
    env: Regex,
    flag: Regex,
    config_key: Regex,
    heredoc: Regex,
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
            heredoc: Regex::new(
                r#"<<-?\s*(?:"([A-Z][A-Z0-9_]{2,})"|'([A-Z][A-Z0-9_]{2,})'|([A-Z][A-Z0-9_]{2,}))"#,
            )
            .map_err(|error| format!("invalid heredoc regex: {error}"))?,
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
        output: &mut BTreeMap<String, TokenMatch>,
        value: &str,
        category: TokenCategory,
        confidence: TokenConfidence,
        evidence: impl Into<String>,
    ) {
        if value.len() < 3 || self.stopwords.contains(value) {
            return;
        }
        let incoming = TokenMatch {
            category,
            confidence,
            evidence: evidence.into(),
        };
        match output.get(value) {
            Some(existing) if existing.confidence >= incoming.confidence => {}
            _ => {
                output.insert(value.to_string(), incoming);
            }
        }
    }

    fn insert_config(&self, output: &mut BTreeMap<String, TokenMatch>, path: &str, value: &str) {
        self.insert(
            output,
            value,
            TokenCategory::ConfigKey,
            TokenConfidence::High,
            evidence_path(path),
        );
    }

    fn extract_lines(
        &self,
        path: &str,
        lines: &[String],
        include_config_keys: bool,
    ) -> BTreeMap<String, TokenMatch> {
        let mut output = BTreeMap::new();
        let yaml_path = is_yaml_path(path);
        let mut yaml_environment_indent = None;
        let heredoc_delimiters = lines
            .iter()
            .flat_map(|line| self.heredoc_delimiters(line))
            .collect::<BTreeSet<_>>();
        for line in lines {
            let yaml_environment_key =
                update_yaml_environment_state(line, yaml_path, &mut yaml_environment_indent);
            for matched in self.env.find_iter(line) {
                let value = matched.as_str();
                if heredoc_delimiters.contains(value) {
                    continue;
                }
                let (confidence, evidence) =
                    env_confidence(path, line, value, yaml_environment_key);
                self.insert(&mut output, value, TokenCategory::Env, confidence, evidence);
            }
            for matched in self.flag.find_iter(line) {
                self.insert(
                    &mut output,
                    matched.as_str(),
                    TokenCategory::Flag,
                    TokenConfidence::High,
                    "pattern:flag",
                );
            }
            if include_config_keys {
                for matched in self.config_key.find_iter(line) {
                    self.insert_config(&mut output, path, matched.as_str());
                }
            }
        }
        output
    }

    fn heredoc_delimiters(&self, line: &str) -> BTreeSet<String> {
        self.heredoc
            .captures_iter(line)
            .filter_map(|captures| {
                (1..=3).find_map(|index| captures.get(index).map(|matched| matched.as_str()))
            })
            .map(str::to_string)
            .collect()
    }
}

impl TokenExtractor for RegexExtractor {
    fn extract(&self, lines: &[String]) -> BTreeMap<String, TokenMatch> {
        self.extract_lines("", lines, false)
    }

    fn extract_for_path(&self, path: &str, lines: &[String]) -> BTreeMap<String, TokenMatch> {
        self.extract_lines(path, lines, is_config_path(path))
    }
}

fn env_confidence(
    path: &str,
    line: &str,
    value: &str,
    yaml_environment_key: bool,
) -> (TokenConfidence, String) {
    if is_dotenv_path(path) && is_dotenv_assignment(line, value) {
        return (TokenConfidence::High, evidence_path(path));
    }
    if yaml_environment_key && is_yaml_environment_key(line, value) {
        return (
            TokenConfidence::High,
            "pattern:yaml_environment".to_string(),
        );
    }
    if line.contains(&format!("std::env::var(\"{value}\")")) {
        return (TokenConfidence::High, "pattern:std::env::var".to_string());
    }
    if line.contains(&format!("process.env.{value}")) {
        return (TokenConfidence::High, "pattern:process.env".to_string());
    }
    if line.contains(&format!("import.meta.env.{value}")) {
        return (TokenConfidence::High, "pattern:import.meta.env".to_string());
    }
    if line.contains(&format!("os.environ[\"{value}\"]")) {
        return (TokenConfidence::High, "pattern:os.environ".to_string());
    }
    if line.contains(&format!("os.getenv(\"{value}\")")) {
        return (TokenConfidence::High, "pattern:os.getenv".to_string());
    }
    if is_shell_path(path) && has_shell_env_reference(line, value) {
        return (TokenConfidence::High, "pattern:shell_env".to_string());
    }
    (TokenConfidence::Low, "pattern:uppercase_word".to_string())
}

fn evidence_path(path: &str) -> String {
    if path.is_empty() {
        "path:<unknown>".to_string()
    } else {
        format!("path:{path}")
    }
}

fn update_yaml_environment_state(
    line: &str,
    yaml_path: bool,
    yaml_environment_indent: &mut Option<usize>,
) -> bool {
    if !yaml_path {
        return false;
    }
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return yaml_environment_indent.is_some();
    }
    let indent = line.len() - trimmed.len();
    if let Some(environment_indent) = *yaml_environment_indent {
        if indent <= environment_indent && !trimmed.starts_with('-') {
            *yaml_environment_indent = None;
        }
    }
    if trimmed == "environment:" || trimmed.starts_with("environment: ") {
        *yaml_environment_indent = Some(indent);
        return false;
    }
    yaml_environment_indent
        .map(|environment_indent| indent > environment_indent)
        .unwrap_or(false)
}

fn is_dotenv_assignment(line: &str, value: &str) -> bool {
    let trimmed = line
        .trim_start()
        .strip_prefix("export ")
        .unwrap_or_else(|| line.trim_start())
        .trim_start();
    let Some(rest) = trimmed.strip_prefix(value) else {
        return false;
    };
    rest.trim_start().starts_with('=')
}

fn is_yaml_environment_key(line: &str, value: &str) -> bool {
    let trimmed = line.trim_start();
    let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim_start();
    let Some(rest) = trimmed.strip_prefix(value) else {
        return false;
    };
    rest.starts_with(':') || rest.starts_with('=')
}

fn has_shell_env_reference(line: &str, value: &str) -> bool {
    if line.contains(&format!("${{{value}}}")) {
        return true;
    }
    let needle = format!("${value}");
    line.match_indices(&needle).any(|(index, _)| {
        line[index + needle.len()..]
            .chars()
            .next()
            .map(|next| !is_token_char(next))
            .unwrap_or(true)
    })
}

fn is_token_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_dotenv_path(path: &str) -> bool {
    let Some(file_name) = Path::new(path).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name == ".env" || file_name.starts_with(".env.")
}

fn is_yaml_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "yaml" | "yml"))
        .unwrap_or(false)
}

fn is_shell_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "sh" | "bash" | "zsh"
            )
        })
        .unwrap_or(false)
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
